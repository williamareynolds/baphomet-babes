use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::wasm_bindgen;
use shared::{AuthUser, CookieIdentity};

#[wasm_bindgen]
extern "C" {
    // Defined by the App Check bootstrap in index.html. Returns a Promise<string>
    // (a fresh attestation token) in production, and resolves to null in local
    // dev where Firebase isn't initialized. `catch` turns a missing bridge into
    // an Err rather than a hard panic.
    #[wasm_bindgen(js_namespace = window, js_name = __appCheckToken, catch)]
    async fn js_app_check_token() -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;

    // Defined by the FCM bootstrap in index.html. Requests notification
    // permission and returns a Promise<string|null> FCM registration token.
    // Resolves to null when unsupported, denied, or in dev.
    #[wasm_bindgen(js_namespace = window, js_name = __enablePush, catch)]
    async fn js_enable_push() -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;

    // Silent token refresh: like __enablePush but never prompts. Resolves to
    // the current FCM token when permission is already granted, else null.
    #[wasm_bindgen(js_namespace = window, js_name = __refreshPush, catch)]
    async fn js_refresh_push() -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;

    // Current Notification permission: "granted" | "denied" | "default" |
    // "unsupported". Synchronous.
    #[wasm_bindgen(js_namespace = window, js_name = __notifPermission, catch)]
    fn js_notif_permission() -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;
}

/// Fresh Firebase App Check token for this app instance, or `None` when the
/// bridge is absent (local dev) or minting fails. Callers attach it as the
/// `X-Firebase-AppCheck` header; `None` means no header, which is correct for
/// environments where the backend isn't enforcing App Check.
pub async fn app_check_token() -> Option<String> {
    js_app_check_token().await.ok().and_then(|v| v.as_string())
}

/// Ask for notification permission and mint an FCM device token. Returns the
/// token on success, or an error message explaining why it failed (unsupported
/// browser, permission denied, getToken error, dev build, …) — surfaced in the
/// UI so failures (especially on iOS) are diagnosable.
pub async fn enable_push() -> Result<String, String> {
    match js_enable_push().await {
        Ok(v) => {
            let s = v
                .as_string()
                .unwrap_or_else(|| "push bridge returned a non-string".to_string());
            match s.strip_prefix("ERR:") {
                Some(msg) => Err(msg.to_string()),
                None => Ok(s),
            }
        }
        Err(_) => Err("push bridge unavailable".to_string()),
    }
}

/// Refresh this device's FCM token without prompting. `None` when permission
/// isn't granted, push is unconfigured (dev), or minting fails. Run on app
/// load so device subscriptions self-heal (token rotation, iOS dropping the
/// subscription, service-worker changes).
pub async fn refresh_push() -> Option<String> {
    js_refresh_push().await.ok().and_then(|v| v.as_string())
}

/// Current notification permission state: "granted" | "denied" | "default" |
/// "unsupported".
pub fn notif_permission() -> String {
    js_notif_permission()
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "unsupported".to_string())
}

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
