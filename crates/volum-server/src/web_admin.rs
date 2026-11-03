use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

const ADMIN_PASSWORD_ENV: &str = "VOLUM_ADMIN_PASSWORD";
const MINIMUM_PASSWORD_CHARACTERS: usize = 12;
const ATTEMPT_WINDOW: Duration = Duration::from_secs(15 * 60);
const BLOCK_DURATION: Duration = Duration::from_secs(60);
const MAXIMUM_FAILURES: u8 = 5;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebAdminConfig {
    password_hash: String,
}

#[derive(Debug, Clone)]
pub(crate) struct WebAdminState {
    password_hash: Option<Arc<str>>,
    attempts: Arc<Mutex<HashMap<IpAddr, LoginAttempt>>>,
}

pub(crate) struct LoadedWebAdmin {
    pub state: WebAdminState,
    pub password_changed: bool,
}

#[derive(Debug)]
struct LoginAttempt {
    failures: u8,
    window_started: Instant,
    blocked_until: Option<Instant>,
}

impl WebAdminState {
    pub(crate) fn load(data_dir: &Path) -> Result<LoadedWebAdmin, String> {
        let path = data_dir.join("web-admin.json");
        let existing = load_config(&path)?;
        let supplied = std::env::var(ADMIN_PASSWORD_ENV)
            .ok()
            .filter(|password| !password.is_empty());
        let mut password_changed = false;

        let password_hash = match (existing, supplied) {
            (Some(config), Some(password)) => {
                validate_password(&password)?;
                if verify_hash(&config.password_hash, &password) {
                    Some(config.password_hash)
                } else {
                    let password_hash = hash_password(&password)?;
                    persist_config(
                        &path,
                        &WebAdminConfig {
                            password_hash: password_hash.clone(),
                        },
                    )?;
                    password_changed = true;
                    Some(password_hash)
                }
            }
            (Some(config), None) => Some(config.password_hash),
            (None, Some(password)) => {
                validate_password(&password)?;
                let password_hash = hash_password(&password)?;
                persist_config(
                    &path,
                    &WebAdminConfig {
                        password_hash: password_hash.clone(),
                    },
                )?;
                password_changed = true;
                Some(password_hash)
            }
            (None, None) => None,
        };

        Ok(LoadedWebAdmin {
            state: WebAdminState {
                password_hash: password_hash.map(Arc::from),
                attempts: Arc::new(Mutex::new(HashMap::new())),
            },
            password_changed,
        })
    }

    pub(crate) fn configured(&self) -> bool {
        self.password_hash.is_some()
    }

    pub(crate) fn verify(&self, password: &str) -> bool {
        self.password_hash
            .as_deref()
            .is_some_and(|hash| verify_hash(hash, password))
    }

    pub(crate) async fn login_permitted(&self, address: IpAddr) -> Result<(), u64> {
        let mut attempts = self.attempts.lock().await;
        let now = Instant::now();
        attempts.retain(|_, attempt| {
            attempt.blocked_until.is_some_and(|until| until > now)
                || now.duration_since(attempt.window_started) < ATTEMPT_WINDOW
        });
        let Some(attempt) = attempts.get(&address) else {
            return Ok(());
        };
        match attempt.blocked_until.filter(|until| *until > now) {
            Some(until) => Err(until.duration_since(now).as_secs().max(1)),
            None => Ok(()),
        }
    }

    pub(crate) async fn record_failure(&self, address: IpAddr) {
        let mut attempts = self.attempts.lock().await;
        let now = Instant::now();
        let attempt = attempts.entry(address).or_insert(LoginAttempt {
            failures: 0,
            window_started: now,
            blocked_until: None,
        });
        if now.duration_since(attempt.window_started) >= ATTEMPT_WINDOW {
            attempt.failures = 0;
            attempt.window_started = now;
            attempt.blocked_until = None;
        }
        attempt.failures = attempt.failures.saturating_add(1);
        if attempt.failures >= MAXIMUM_FAILURES {
            attempt.blocked_until = Some(now + BLOCK_DURATION);
        }
    }

    pub(crate) async fn record_success(&self, address: IpAddr) {
        self.attempts.lock().await.remove(&address);
    }

    #[cfg(test)]
    pub(crate) fn for_test(password: &str) -> Self {
        Self {
            password_hash: Some(Arc::from(hash_password(password).unwrap())),
            attempts: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

fn config_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn load_config(path: &Path) -> Result<Option<WebAdminConfig>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("Could not read web administrator settings: {error}"))?;
    let config = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Web administrator settings are damaged: {error}"))?;
    Ok(Some(config))
}

fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < MINIMUM_PASSWORD_CHARACTERS {
        return Err(format!(
            "{ADMIN_PASSWORD_ENV} must contain at least {MINIMUM_PASSWORD_CHARACTERS} characters"
        ));
    }
    if password.chars().any(char::is_control) {
        return Err(format!(
            "{ADMIN_PASSWORD_ENV} must not contain control characters"
        ));
    }
    Ok(())
}

fn hash_password(password: &str) -> Result<String, String> {
    let mut salt_bytes = [0_u8; 16];
    rand::rng().fill_bytes(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|error| error.to_string())?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| format!("Could not protect the web administrator password: {error}"))
}

fn verify_hash(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash).ok().is_some_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

fn persist_config(path: &Path, config: &WebAdminConfig) -> Result<(), String> {
    let path = config_path(path);
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)
        .map_err(|error| format!("Could not save web administrator settings: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Could not protect web administrator settings: {error}"))?;
    }
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Could not replace web administrator settings: {error}"))?;
    }
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Could not activate web administrator settings: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hashes_verify_without_storing_the_password() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(!hash.contains("correct horse"));
        assert!(verify_hash(&hash, "correct horse battery staple"));
        assert!(!verify_hash(&hash, "wrong password"));
    }

    #[test]
    fn short_passwords_are_rejected() {
        assert!(validate_password("too-short").is_err());
        assert!(validate_password("long-enough-password").is_ok());
    }
}
