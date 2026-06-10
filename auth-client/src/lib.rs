pub use shared::{AuthUser, CookieIdentity};

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::{clear_auth, load_auth, load_identity, save_auth};
