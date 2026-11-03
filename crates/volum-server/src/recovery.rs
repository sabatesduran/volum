use serde::Deserialize;
use std::path::Path;
use uuid::Uuid;
use walkdir::WalkDir;

type RecoveryResult<T> = Result<T, String>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingRestore {
    token: String,
    #[allow(dead_code)]
    prepared_at: String,
}

fn validate_token(token: &str) -> RecoveryResult<()> {
    Uuid::parse_str(token)
        .map(|_| ())
        .map_err(|_| "Invalid restore token".into())
}

pub fn apply_pending_restore(data_dir: &Path) -> RecoveryResult<()> {
    let marker_path = data_dir.join("restore-pending.json");
    if !marker_path.is_file() {
        return Ok(());
    }
    let marker: PendingRestore =
        serde_json::from_slice(&std::fs::read(&marker_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    validate_token(&marker.token)?;
    let staging = data_dir.join("restore-staging").join(&marker.token);
    let staged_database = staging.join("database/volum.sqlite3");
    if !staged_database.is_file() {
        return Err("The pending restore database is missing".into());
    }
    let database = data_dir.join("volum.sqlite3");
    let previous = data_dir.join("volum.sqlite3.pre-restore.bak");
    let database_new = data_dir.join("volum.sqlite3.restore-new");
    let media = data_dir.join("media");
    let old_media = data_dir.join("media.pre-restore");
    let media_new = data_dir.join("media.restore-new");
    let staged_media = staging.join("media");
    let staged_managed = staging.join("managed-library");
    let managed = data_dir.join("Managed Library");
    let old_managed = data_dir.join("Managed Library.pre-restore");
    let managed_new = data_dir.join("Managed Library.restore-new");

    let _ = std::fs::remove_file(&database_new);
    std::fs::copy(&staged_database, &database_new)
        .map_err(|error| format!("Could not prepare the restored database: {error}"))?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&database_new)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("Could not flush the restored database: {error}"))?;
    let _ = std::fs::remove_dir_all(&media_new);
    copy_directory(&staged_media, &media_new)
        .map_err(|error| format!("Could not prepare restored Volum media: {error}"))?;
    if staged_managed.is_dir() {
        let _ = std::fs::remove_dir_all(&managed_new);
        copy_directory(&staged_managed, &managed_new)
            .map_err(|error| format!("Could not prepare the restored managed library: {error}"))?;
    }

    let _ = std::fs::remove_file(data_dir.join("volum.sqlite3-wal"));
    let _ = std::fs::remove_file(data_dir.join("volum.sqlite3-shm"));
    let _ = std::fs::remove_file(&previous);
    if database.exists() {
        std::fs::rename(&database, &previous)
            .map_err(|error| format!("Could not preserve the current database: {error}"))?;
    }
    if let Err(error) = std::fs::rename(&database_new, &database) {
        if previous.exists() {
            let _ = std::fs::rename(&previous, &database);
        }
        return Err(format!("Could not activate the restored database: {error}"));
    }
    let mut media_preserved = false;
    let mut managed_preserved = false;
    let _ = std::fs::remove_dir_all(&old_media);
    if media.exists() {
        if let Err(error) = std::fs::rename(&media, &old_media) {
            let rollback = rollback_activated_restore(
                &database,
                &previous,
                &media,
                &old_media,
                false,
                false,
                &managed,
                &old_managed,
                false,
                false,
            );
            let _ = std::fs::remove_dir_all(&media_new);
            return Err(restore_error(
                &format!("Could not preserve current Volum media: {error}"),
                rollback,
            ));
        }
        media_preserved = true;
    }
    if let Err(error) = std::fs::rename(&media_new, &media) {
        let rollback = rollback_activated_restore(
            &database,
            &previous,
            &media,
            &old_media,
            media_preserved,
            false,
            &managed,
            &old_managed,
            false,
            false,
        );
        return Err(restore_error(
            &format!("Could not restore Volum media: {error}"),
            rollback,
        ));
    }
    let media_activated = true;
    if staged_managed.is_dir() {
        let _ = std::fs::remove_dir_all(&old_managed);
        if managed.exists() {
            if let Err(error) = std::fs::rename(&managed, &old_managed) {
                let rollback = rollback_activated_restore(
                    &database,
                    &previous,
                    &media,
                    &old_media,
                    media_preserved,
                    media_activated,
                    &managed,
                    &old_managed,
                    false,
                    false,
                );
                return Err(restore_error(
                    &format!("Could not preserve the current restored managed library: {error}"),
                    rollback,
                ));
            }
            managed_preserved = true;
        }
        if let Err(error) = std::fs::rename(&managed_new, &managed) {
            let rollback = rollback_activated_restore(
                &database,
                &previous,
                &media,
                &old_media,
                media_preserved,
                media_activated,
                &managed,
                &old_managed,
                managed_preserved,
                false,
            );
            return Err(restore_error(
                &format!("Could not restore managed project files: {error}"),
                rollback,
            ));
        }
        let managed_activated = true;
        if let Err(error) = std::fs::write(
            data_dir.join("managed-restore-pending"),
            managed.to_string_lossy().as_bytes(),
        ) {
            let _ = std::fs::remove_file(data_dir.join("managed-restore-pending"));
            let rollback = rollback_activated_restore(
                &database,
                &previous,
                &media,
                &old_media,
                media_preserved,
                media_activated,
                &managed,
                &old_managed,
                managed_preserved,
                managed_activated,
            );
            return Err(restore_error(
                &format!("Could not finish the managed-library restore: {error}"),
                rollback,
            ));
        }
    }
    std::fs::remove_file(marker_path).map_err(|error| error.to_string())?;
    let _ = std::fs::remove_dir_all(staging);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn rollback_activated_restore(
    database: &Path,
    previous: &Path,
    media: &Path,
    old_media: &Path,
    media_preserved: bool,
    media_activated: bool,
    managed: &Path,
    old_managed: &Path,
    managed_preserved: bool,
    managed_activated: bool,
) -> Vec<String> {
    let mut errors = Vec::new();
    if managed_activated && managed.exists() {
        if let Err(error) = std::fs::remove_dir_all(managed) {
            errors.push(format!(
                "could not remove restored managed library: {error}"
            ));
        }
    }
    if managed_preserved && old_managed.exists() && !managed.exists() {
        if let Err(error) = std::fs::rename(old_managed, managed) {
            errors.push(format!(
                "could not restore previous managed library: {error}"
            ));
        }
    }
    if media_activated && media.exists() {
        if let Err(error) = std::fs::remove_dir_all(media) {
            errors.push(format!("could not remove restored media: {error}"));
        }
    }
    if media_preserved && old_media.exists() && !media.exists() {
        if let Err(error) = std::fs::rename(old_media, media) {
            errors.push(format!("could not restore previous media: {error}"));
        }
    }
    if database.exists() {
        if let Err(error) = std::fs::remove_file(database) {
            errors.push(format!("could not remove restored database: {error}"));
        }
    }
    if previous.exists() && !database.exists() {
        if let Err(error) = std::fs::rename(previous, database) {
            errors.push(format!("could not restore previous database: {error}"));
        }
    }
    errors
}

fn restore_error(message: &str, rollback_errors: Vec<String>) -> String {
    if rollback_errors.is_empty() {
        message.to_string()
    } else {
        format!(
            "{message}. Restore rollback needs attention: {}",
            rollback_errors.join("; ")
        )
    }
}

fn copy_directory(source: &Path, destination: &Path) -> RecoveryResult<()> {
    std::fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|error| error.to_string())?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| error.to_string())?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let output = destination.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&output).map_err(|error| error.to_string())?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            std::fs::copy(entry.path(), &output).map_err(|error| error.to_string())?;
            std::fs::OpenOptions::new()
                .write(true)
                .open(&output)
                .and_then(|file| file.sync_all())
                .map_err(|error| error.to_string())?;
        } else {
            return Err("Restored media contains an unsupported filesystem entry".into());
        }
    }
    Ok(())
}
