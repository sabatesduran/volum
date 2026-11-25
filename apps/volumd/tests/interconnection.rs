//! End-to-end checks for the server, Desktop, and mobile interconnection.
//!
//! Each test starts the real `volumd` binary, pairs clients through the same
//! endpoints the browser administrator, Desktop, and mobile companion use, and
//! verifies the optional Desktop local copy of project files.

use serde_json::{json, Value};
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use volum_server::{credentials::CredentialStore, remote};

const ADMIN_PASSWORD: &str = "correct horse battery staple";

#[derive(Default)]
struct MemoryCredentialStore(Mutex<HashMap<String, String>>);

impl CredentialStore for MemoryCredentialStore {
    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(account.into(), secret.into());
        Ok(())
    }

    fn get(&self, account: &str) -> Result<String, String> {
        self.0
            .lock()
            .unwrap()
            .get(account)
            .cloned()
            .ok_or_else(|| "missing".into())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(account);
        Ok(())
    }
}

struct Server {
    child: Child,
    base_url: String,
    port: u16,
    config: PathBuf,
    library: PathBuf,
}

impl Server {
    async fn start(root: &Path) -> Self {
        let port = free_port();
        Self::start_on(root.join("config"), root.join("library"), port).await
    }

    async fn start_on(config: PathBuf, library: PathBuf, port: u16) -> Self {
        let base_url = format!("http://127.0.0.1:{port}");
        let child = Command::new(env!("CARGO_BIN_EXE_volumd"))
            .env("VOLUM_CONFIG_DIR", &config)
            .env("VOLUM_LIBRARY_DIR", &library)
            .env("VOLUM_BIND", format!("127.0.0.1:{port}"))
            .env("VOLUM_PUBLIC_URL", &base_url)
            .env("VOLUM_ADMIN_PASSWORD", ADMIN_PASSWORD)
            .env("VOLUM_WEB_DIR", config.join("no-web-assets"))
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("volumd should start");
        let server = Self {
            child,
            base_url,
            port,
            config,
            library,
        };
        server.wait_until_ready().await;
        server
    }

    async fn wait_until_ready(&self) {
        let client = reqwest::Client::new();
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if let Ok(response) = client.get(format!("{}/readyz", self.base_url)).send().await {
                if response.status().is_success() {
                    return;
                }
            }
            assert!(Instant::now() < deadline, "volumd did not become ready");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn restart(mut self) -> Self {
        self.stop();
        Self::start_on(self.config.clone(), self.library.clone(), self.port).await
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Admin {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

impl Admin {
    async fn login(server: &Server) -> Self {
        let http = reqwest::Client::new();
        let login: Value = http
            .post(format!("{}/api/v1/web/login", server.base_url))
            .json(&json!({ "password": ADMIN_PASSWORD, "deviceName": "Integration browser" }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .expect("the administrator password from the environment should work")
            .json()
            .await
            .unwrap();
        Self {
            http,
            base_url: server.base_url.clone(),
            token: login["token"].as_str().unwrap().to_string(),
        }
    }

    async fn pairing_payload(&self) -> String {
        let pairing: Value = self
            .http
            .post(format!("{}/api/v1/admin/pairings", self.base_url))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        pairing["qrPayload"].as_str().unwrap().to_string()
    }

    async fn client_devices(&self) -> Vec<Value> {
        let devices: Vec<Value> = self
            .http
            .get(format!("{}/api/v1/admin/devices", self.base_url))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        devices
            .into_iter()
            .filter(|device| device["role"] == "client")
            .collect()
    }

    async fn revoke(&self, device_id: &str) {
        self.http
            .delete(format!(
                "{}/api/v1/admin/devices/{device_id}",
                self.base_url
            ))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }
}

/// Mirrors `pairWithDesktop` in `apps/mobile/src/lib/transport.ts`.
struct Mobile {
    http: reqwest::Client,
    base_url: String,
    device_id: String,
    token: String,
}

impl Mobile {
    async fn pair(payload_json: &str) -> Self {
        let http = reqwest::Client::new();
        let payload: Value = serde_json::from_str(payload_json).unwrap();
        let base_url = payload["baseUrl"].as_str().unwrap().to_string();
        let identity: Value = http
            .get(format!("{base_url}/identity"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(identity["serverId"], payload["serverId"]);
        assert!(identity["protocolMin"].as_i64().unwrap() <= 1);
        assert!(identity["protocolMax"].as_i64().unwrap() >= 1);
        let paired: Value = http
            .post(format!("{base_url}/pair"))
            .json(&json!({
                "secret": payload["secret"],
                "deviceName": "Workshop phone",
                "platform": "Android"
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(paired["serverId"], payload["serverId"]);
        assert_eq!(paired["protocolVersion"], 1);
        Self {
            http,
            base_url,
            device_id: paired["deviceId"].as_str().unwrap().to_string(),
            token: paired["token"].as_str().unwrap().to_string(),
        }
    }

    async fn get(&self, path: &str) -> reqwest::Response {
        self.http
            .get(format!("{}{path}", self.base_url))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap()
    }

    async fn project_names(&self) -> Vec<String> {
        let page: Value = self
            .get("/api/v1/projects")
            .await
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        let mut names: Vec<String> = page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|project| project["displayName"].as_str().unwrap().to_string())
            .collect();
        names.sort();
        names
    }

    async fn upload(&self, project_name: &str, filename: &str, bytes: Vec<u8>) -> Value {
        self.http
            .post(format!("{}/api/v1/uploads", self.base_url))
            .query(&[("filename", filename), ("projectName", project_name)])
            .bearer_auth(&self.token)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(bytes)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .expect("mobile uploads should be accepted")
            .json()
            .await
            .unwrap()
    }
}

fn free_port() -> u16 {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.local_addr().unwrap().port()
}

fn test_stl(offset: &str) -> Vec<u8> {
    format!(
        "solid volum\n  facet normal 0 0 1\n    outer loop\n      vertex {offset} 0 0\n      vertex 1 0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid volum\n"
    )
    .into_bytes()
}

fn status_json(status: remote::BackendConnectionStatus) -> Value {
    serde_json::to_value(status).unwrap()
}

fn desktop(client_data: &Path) -> remote::RemoteController {
    desktop_with(client_data, Arc::new(MemoryCredentialStore::default()))
}

fn desktop_with(client_data: &Path, store: Arc<MemoryCredentialStore>) -> remote::RemoteController {
    std::fs::create_dir_all(client_data).unwrap();
    remote::RemoteController::new(client_data, store).unwrap()
}

async fn desktop_project_names(controller: &remote::RemoteController) -> Vec<String> {
    let request = serde_json::from_value(json!({ "path": "/api/v1/projects" })).unwrap();
    let page = remote::remote_json_request(request, controller)
        .await
        .unwrap();
    let mut names: Vec<String> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|project| project["displayName"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

async fn desktop_upload(
    controller: &remote::RemoteController,
    source: &Path,
    project_name: &str,
) -> Value {
    let request = serde_json::from_value(json!({
        "filePath": source,
        "filename": source.file_name().unwrap().to_str().unwrap(),
        "projectName": project_name,
    }))
    .unwrap();
    remote::remote_upload_file(request, controller)
        .await
        .expect("Desktop uploads should be accepted")
}

fn mirrored_files(mirror: &Path) -> Vec<Vec<u8>> {
    let mut files: Vec<Vec<u8>> = walk(mirror)
        .into_iter()
        .filter(|path| !path.starts_with(mirror.join(".volum")))
        .map(|path| std::fs::read(path).unwrap())
        .collect();
    files.sort();
    files
}

fn walk(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files
}

#[tokio::test]
async fn desktop_and_mobile_share_one_server_library_with_a_local_desktop_copy() {
    let directory = tempfile::tempdir().unwrap();
    let server = Server::start(directory.path()).await;
    let admin = Admin::login(&server).await;

    // Desktop: Settings → Advanced → Connect to a Volum server… → paste payload.
    let desktop_payload = admin.pairing_payload().await;
    let desktop = desktop(&directory.path().join("desktop-data"));
    let connected = status_json(
        remote::connect_remote_server(desktop_payload.clone(), &desktop)
            .await
            .unwrap(),
    );
    assert_eq!(connected["mode"], "remote");
    assert_eq!(connected["online"], true);
    assert_eq!(connected["baseUrl"], server.base_url);
    assert_eq!(connected["mirrorEnabled"], false);
    let reused = remote::connect_remote_server(desktop_payload, &desktop)
        .await
        .expect_err("a pairing payload must work only once");
    assert!(!reused.is_empty());
    let status = status_json(remote::get_backend_connection(&desktop).await.unwrap());
    assert_eq!(status["online"], true, "{status}");

    let desktop_source = directory.path().join("desk-lamp.stl");
    std::fs::write(&desktop_source, test_stl("0")).unwrap();
    desktop_upload(&desktop, &desktop_source, "Desk lamp").await;

    // Mobile: scan the QR code from the administrator.
    let mobile = Mobile::pair(&admin.pairing_payload().await).await;
    assert_eq!(mobile.project_names().await, ["Desk lamp"]);
    mobile
        .upload("Phone stand", "phone-stand.stl", test_stl("5"))
        .await;
    assert_eq!(
        desktop_project_names(&desktop).await,
        ["Desk lamp", "Phone stand"]
    );

    let devices = admin.client_devices().await;
    assert_eq!(devices.len(), 2, "{devices:?}");
    assert!(devices.iter().any(|device| device["platform"] == "Android"));

    // Desktop: Local project files → Entire library → Choose local folder.
    let mirror = directory.path().join("Volum local copy");
    let configured = status_json(
        remote::configure_remote_mirror(
            Some(mirror.to_string_lossy().to_string()),
            true,
            Some("all".into()),
            &desktop,
        )
        .await
        .unwrap(),
    );
    assert_eq!(configured["mirrorEnabled"], true);
    assert_eq!(configured["mirrorMode"], "all");
    let report = serde_json::to_value(remote::sync_remote_mirror(&desktop).await.unwrap()).unwrap();
    assert_eq!(report["downloaded"], 2, "{report}");
    assert_eq!(report["totalAssets"], 2, "{report}");
    assert_eq!(mirrored_files(&mirror), [test_stl("0"), test_stl("5")]);

    // New projects from the phone reach the Desktop copy on the next sync.
    mobile
        .upload("Cable clip", "cable-clip.stl", test_stl("7"))
        .await;
    let report = serde_json::to_value(remote::sync_remote_mirror(&desktop).await.unwrap()).unwrap();
    assert_eq!(report["downloaded"], 1, "{report}");
    assert_eq!(report["unchanged"], 2, "{report}");
    assert_eq!(mirrored_files(&mirror).len(), 3);

    // Offline: originals stay available from the local copy.
    let mut server = server;
    server.stop();
    let offline = status_json(remote::get_backend_connection(&desktop).await.unwrap());
    assert_eq!(offline["mode"], "remote");
    assert_eq!(offline["online"], false);
    assert!(offline["lastError"].is_string(), "{offline}");
    let available = remote::list_mirrored_assets(&desktop).await.unwrap();
    assert_eq!(available.len(), 3);

    // Back online: both clients reconnect with their stored credentials.
    let server = server.restart().await;
    let online = status_json(remote::get_backend_connection(&desktop).await.unwrap());
    assert_eq!(online["online"], true, "{online}");
    assert_eq!(mobile.project_names().await.len(), 3);

    // Revoking the phone does not affect the Desktop.
    let admin = Admin::login(&server).await;
    admin.revoke(&mobile.device_id).await;
    assert_eq!(
        mobile.get("/api/v1/status").await.status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    assert_eq!(desktop_project_names(&desktop).await.len(), 3);
}

#[tokio::test]
async fn desktop_can_switch_between_local_and_server_libraries() {
    let directory = tempfile::tempdir().unwrap();
    let server = Server::start(directory.path()).await;
    let admin = Admin::login(&server).await;
    let desktop_data = directory.path().join("desktop-data");
    let store = Arc::new(MemoryCredentialStore::default());
    let desktop = desktop_with(&desktop_data, store.clone());

    remote::connect_remote_server(admin.pairing_payload().await, &desktop)
        .await
        .unwrap();
    let mirror = directory.path().join("mirror");
    remote::configure_remote_mirror(
        Some(mirror.to_string_lossy().to_string()),
        true,
        Some("selected".into()),
        &desktop,
    )
    .await
    .unwrap();

    let local = status_json(remote::use_local_backend(&desktop).await.unwrap());
    assert_eq!(local["mode"], "local");
    assert_eq!(local["configured"], true);

    let remote_again = status_json(remote::use_remote_backend(&desktop).await.unwrap());
    assert_eq!(remote_again["mode"], "remote");
    assert_eq!(remote_again["online"], true, "{remote_again}");
    assert_eq!(remote_again["mirrorEnabled"], true);
    assert_eq!(remote_again["mirrorMode"], "selected");

    // Re-opening Desktop keeps the pairing and local copy settings.
    drop(desktop);
    let reopened = desktop_with(&desktop_data, store.clone());
    let persisted = status_json(remote::get_backend_connection(&reopened).await.unwrap());
    assert_eq!(persisted["mode"], "remote");
    assert_eq!(persisted["online"], true, "{persisted}");
    assert_eq!(persisted["mirrorEnabled"], true);

    let forgotten = status_json(remote::forget_remote_server(&reopened).await.unwrap());
    assert_eq!(forgotten["mode"], "local");
    assert_eq!(forgotten["configured"], false);
    assert_eq!(forgotten["mirrorEnabled"], false);
    assert!(
        mirror.is_dir(),
        "forgetting a server must not delete local files"
    );
    assert!(
        store.0.lock().unwrap().is_empty(),
        "forgetting a server removes its token"
    );
}

#[tokio::test]
async fn pairing_rejects_payloads_for_a_different_server() {
    let directory = tempfile::tempdir().unwrap();
    let first = Server::start(&directory.path().join("first")).await;
    let second = Server::start(&directory.path().join("second")).await;
    let payload = Admin::login(&first).await.pairing_payload().await;
    let mut forged: Value = serde_json::from_str(&payload).unwrap();
    forged["baseUrl"] = json!(second.base_url);

    let desktop = desktop(&directory.path().join("desktop-data"));
    let error = remote::connect_remote_server(forged.to_string(), &desktop)
        .await
        .expect_err("Desktop must verify the server identity before pairing");
    assert!(
        error.contains("different") || error.contains("identity"),
        "{error}"
    );
    let status = status_json(remote::get_backend_connection(&desktop).await.unwrap());
    assert_eq!(status["configured"], false);
    assert!(Admin::login(&second)
        .await
        .client_devices()
        .await
        .is_empty());
}
