//! Local bookkeeping for this device's FCM token. We remember the token we
//! registered so we can unregister it on logout (important on shared devices —
//! otherwise the previous user keeps receiving this device's pushes).

const KEY: &str = "fcm_token";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

pub fn save(token: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(KEY, token);
    }
}

pub fn load() -> Option<String> {
    storage().and_then(|s| s.get_item(KEY).ok().flatten())
}

pub fn clear() {
    if let Some(s) = storage() {
        let _ = s.remove_item(KEY);
    }
}
