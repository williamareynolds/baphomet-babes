//! Progressive-web-app glue: an offline indicator and a "new version available"
//! bar. The service worker (public/sw.js) handles offline caching; this module
//! handles the user-facing signals.

use leptos::prelude::*;
use thaw::{Button, ButtonAppearance, ButtonSize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

/// Git SHA this bundle was built from (see build.rs). The deployed
/// /version.json holds the SHA of the currently published build; when they
/// differ, a newer version is live and we prompt to update.
const BUILD_SHA: &str = env!("BUILD_SHA");

/// True when the app is running as an installed PWA (its own window, no browser
/// chrome) rather than in a normal browser tab. Used to hide install prompts and
/// the install-help link once there's nothing left to install.
pub fn is_standalone() -> bool {
    web_sys::window()
        .and_then(|w| w.match_media("(display-mode: standalone)").ok().flatten())
        .map(|m| m.matches())
        .unwrap_or(false)
}

fn online_now() -> bool {
    web_sys::window()
        .map(|w| w.navigator().on_line())
        .unwrap_or(true)
}

async fn published_version() -> Option<String> {
    // Cache-bust so we never read a stale copy.
    let url = format!("/version.json?t={}", js_sys::Date::now() as u64);
    let resp = gloo_net::http::Request::get(&url).send().await.ok()?;
    if !resp.ok() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("version")?.as_str().map(|s| s.to_string())
}

fn reload() {
    if let Some(w) = web_sys::window() {
        let _ = w.location().reload();
    }
}

/// True when an immediate reload would lose nothing — i.e. no text field holds
/// unsaved input (a chat draft, a half-filled form). Auto-update waits for such a
/// moment instead of nuking what someone is mid-way through typing.
fn safe_to_reload() -> bool {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return true;
    };
    let Ok(nodes) = doc.query_selector_all("input, textarea") else {
        return true;
    };
    for i in 0..nodes.length() {
        let Some(node) = nodes.item(i) else { continue };
        if let Some(input) = node.dyn_ref::<web_sys::HtmlInputElement>() {
            if !input.value().is_empty() {
                return false;
            }
        } else if let Some(area) = node.dyn_ref::<web_sys::HtmlTextAreaElement>() {
            if !area.value().is_empty() {
                return false;
            }
        }
    }
    true
}

/// Renders the offline + update bars. Mount once at the app root.
#[component]
pub fn PwaBars() -> impl IntoView {
    let win = web_sys::window().expect("window");

    // ----- offline indicator -----
    let (online, set_online) = signal(online_now());
    let on_online = Closure::<dyn FnMut()>::new(move || set_online.set(true));
    let on_offline = Closure::<dyn FnMut()>::new(move || set_online.set(false));
    let _ = win.add_event_listener_with_callback("online", on_online.as_ref().unchecked_ref());
    let _ = win.add_event_listener_with_callback("offline", on_offline.as_ref().unchecked_ref());
    on_online.forget();
    on_offline.forget();

    // ----- update available -----
    let (update_ready, set_update_ready) = signal(false);

    // Apply a known-pending update, but only at a safe moment (nothing typed).
    // The manual bar below is the force-now escape hatch when this holds off.
    let try_apply = move || {
        if update_ready.get_untracked() && safe_to_reload() {
            reload();
        }
    };

    let check = move || {
        spawn_local(async move {
            // "dev" builds have no published version.json to compare against.
            if BUILD_SHA == "dev" {
                return;
            }
            if let Some(published) = published_version().await {
                if published != BUILD_SHA {
                    set_update_ready.set(true);
                    try_apply();
                }
            }
        });
    };
    check(); // on load
    let interval_cb = Closure::<dyn FnMut()>::new(move || check());
    let _ = win.set_interval_with_callback_and_timeout_and_arguments_0(
        interval_cb.as_ref().unchecked_ref(),
        5 * 60 * 1000, // re-check every 5 minutes
    );
    interval_cb.forget();

    // Re-attempt on app foreground — the natural "reopen = fresh" moment for an
    // installed PWA, and a point where a half-typed draft is usually long gone.
    if let Some(doc) = win.document() {
        let vis_cb = Closure::<dyn FnMut()>::new(move || try_apply());
        let _ = doc
            .add_event_listener_with_callback("visibilitychange", vis_cb.as_ref().unchecked_ref());
        vis_cb.forget();
    }

    view! {
        <Show when=move || !online.get()>
            <div class="pwa-bar pwa-offline">
                <span>"You're offline — changes won't save until you reconnect."</span>
            </div>
        </Show>
        <Show when=move || update_ready.get()>
            <div class="pwa-bar pwa-update">
                <span>"A new version is available."</span>
                <Button
                    appearance=ButtonAppearance::Secondary
                    size=ButtonSize::Small
                    on_click=move |_| reload()
                >"Update"</Button>
            </div>
        </Show>
    }
}
