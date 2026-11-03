use serde::Serialize;
use std::sync::Arc;

type EmitCallback = dyn Fn(&str, serde_json::Value) + Send + Sync;

/// Transport-neutral backend event publisher.
///
/// Headless services use the no-op default. Desktop injects a Tauri-backed
/// callback at the adapter boundary.
#[derive(Clone, Default)]
pub struct EventEmitter {
    callback: Option<Arc<EmitCallback>>,
}

impl EventEmitter {
    pub fn new(callback: impl Fn(&str, serde_json::Value) + Send + Sync + 'static) -> Self {
        Self {
            callback: Some(Arc::new(callback)),
        }
    }

    pub fn emit<T: Serialize>(&self, event: &str, payload: T) {
        let Some(callback) = &self.callback else {
            return;
        };
        match serde_json::to_value(payload) {
            Ok(value) => callback(event, value),
            Err(error) => log::warn!("Could not serialize backend event {event}: {error}"),
        }
    }
}
