use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub username: String,
    pub role: String,
    pub token: String,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

pub fn save_auth(user: &AuthUser) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item("auth_user", &serde_json::to_string(user).unwrap_or_default());
    }
}

pub fn load_auth() -> Option<AuthUser> {
    local_storage()
        .and_then(|s| s.get_item("auth_user").ok().flatten())
        .and_then(|s| serde_json::from_str(&s).ok())
}

pub fn clear_auth() {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item("auth_user");
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
}
