//! Tiny read-through stash for low-sensitivity GET data (events, announcements)
//! so the app still shows the last-seen schedule when a fetch fails offline.
//!
//! Backed by localStorage. The global offline bar (see [`crate::pwa`]) is the
//! staleness signal, so callers serve a recalled copy without extra UI. Authed
//! sensitive feeds (e.g. group chat) deliberately do NOT use this — we don't want
//! their contents sitting at rest in localStorage.

use serde::Serialize;
use serde::de::DeserializeOwned;

fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

/// Namespaced key so stashed payloads don't collide with auth/identity keys.
fn key(name: &str) -> String {
    format!("bb-cache:{name}")
}

/// Persist the latest successful payload (best effort; silently no-ops).
pub fn stash<T: Serialize>(name: &str, value: &T) {
    if let (Some(s), Ok(json)) = (storage(), serde_json::to_string(value)) {
        let _ = s.set_item(&key(name), &json);
    }
}

/// Read back the last stashed payload, if any and still parseable.
pub fn recall<T: DeserializeOwned>(name: &str) -> Option<T> {
    let raw = storage()?.get_item(&key(name)).ok().flatten()?;
    serde_json::from_str(&raw).ok()
}
