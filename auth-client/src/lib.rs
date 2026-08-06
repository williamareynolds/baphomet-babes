pub use shared::{AuthUser, CookieIdentity};

mod jwt;

pub use jwt::{exp_seconds, is_expired};

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::{
    app_check_token, clear_auth, enable_push, load_auth, load_identity, notif_permission,
    refresh_push, save_auth,
};
