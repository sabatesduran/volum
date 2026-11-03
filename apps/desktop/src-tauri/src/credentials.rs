use volum_server::credentials::CredentialStore;

pub struct KeyringCredentialStore {
    service: &'static str,
}

impl KeyringCredentialStore {
    pub fn new(service: &'static str) -> Self {
        Self { service }
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        keyring::Entry::new(self.service, account)
            .map_err(|error| error.to_string())?
            .set_password(secret)
            .map_err(|error| error.to_string())
    }

    fn get(&self, account: &str) -> Result<String, String> {
        keyring::Entry::new(self.service, account)
            .map_err(|error| error.to_string())?
            .get_password()
            .map_err(|error| error.to_string())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        let entry =
            keyring::Entry::new(self.service, account).map_err(|error| error.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}
