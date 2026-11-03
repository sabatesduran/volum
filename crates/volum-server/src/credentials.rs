use std::sync::Arc;

pub trait CredentialStore: Send + Sync {
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    fn get(&self, account: &str) -> Result<String, String>;
    fn delete(&self, account: &str) -> Result<(), String>;
}

pub type SharedCredentialStore = Arc<dyn CredentialStore>;

#[derive(Default)]
pub struct UnavailableCredentialStore;

impl CredentialStore for UnavailableCredentialStore {
    fn set(&self, _account: &str, _secret: &str) -> Result<(), String> {
        Err("A native credential store is unavailable".into())
    }

    fn get(&self, _account: &str) -> Result<String, String> {
        Err("A native credential store is unavailable".into())
    }

    fn delete(&self, _account: &str) -> Result<(), String> {
        Ok(())
    }
}
