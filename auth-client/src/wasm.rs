use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use wasm_bindgen::JsCast;
use shared::{AuthUser, CookieIdentity};

/// Save auth: JWT → localStorage (this domain only), identity → cross-domain cookie (no JWT).
pub fn save_auth(user: &AuthUser) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item("auth_user", &serde_json::to_string(user).unwrap_or_default());
    }
    let identity = CookieIdentity::from(user);
    if let Ok(json) = serde_json::to_string(&identity) {
        set_identity_cookie(&json);
    }
}

/// Load auth from localStorage only (has JWT). Cookie has no JWT.
pub fn load_auth() -> Option<AuthUser> {
    local_storage()
        .and_then(|s| s.get_item("auth_user").ok().flatten())
        .and_then(|s| serde_json::from_str::<AuthUser>(&s).ok())
        .filter(|u| !u.token.is_empty())
}

/// Load identity from cross-domain cookie for display (no JWT).
pub fn load_identity() -> Option<CookieIdentity> {
    let cookies = html_document()?.cookie().ok()?;
    for cookie in cookies.split(';') {
        let cookie = cookie.trim();
        if let Some(val) = cookie.strip_prefix("bb_identity=") {
            let bytes = URL_SAFE_NO_PAD.decode(val).ok()?;
            let json = String::from_utf8(bytes).ok()?;
            return serde_json::from_str(&json).ok();
        }
    }
    None
}

pub fn clear_auth() {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item("auth_user");
    }
    clear_identity_cookie();
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

fn html_document() -> Option<web_sys::HtmlDocument> {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.dyn_into::<web_sys::HtmlDocument>().ok())
}

fn cookie_attrs() -> String {
    let hostname = web_sys::window()
        .and_then(|w| w.location().hostname().ok())
        .unwrap_or_default();
    if hostname.ends_with("baphometbabes.com") {
        "; domain=.baphometbabes.com; secure".to_string()
    } else {
        String::new()
    }
}

fn set_identity_cookie(json: &str) {
    if let Some(doc) = html_document() {
        let attrs = cookie_attrs();
        let val = URL_SAFE_NO_PAD.encode(json.as_bytes());
        let _ = doc.set_cookie(&format!("bb_identity={val}; path=/; max-age=604800; samesite=lax{attrs}"));
    }
}

fn clear_identity_cookie() {
    if let Some(doc) = html_document() {
        let attrs = cookie_attrs();
        let _ = doc.set_cookie(&format!("bb_identity=; path=/; max-age=0; samesite=lax{attrs}"));
    }
}
