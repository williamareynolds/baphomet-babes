//! Progressive-web-app glue: an offline indicator and a "new version available"
//! bar. The service worker (public/sw.js) handles offline caching; this module
//! handles the user-facing signals.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

/// Git SHA this bundle was built from (see build.rs). The deployed
/// /version.json holds the SHA of the currently published build; when they
/// differ, a newer version is live and we prompt to update.
const BUILD_SHA: &str = env!("BUILD_SHA");

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
    let check = move || {
        spawn_local(async move {
            // "dev" builds have no published version.json to compare against.
            if BUILD_SHA == "dev" {
                return;
            }
            if let Some(published) = published_version().await {
                if published != BUILD_SHA {
                    set_update_ready.set(true);
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

    view! {
        <Show when=move || !online.get()>
            <div class="pwa-bar pwa-offline">
                <span>"You're offline — changes won't save until you reconnect."</span>
            </div>
        </Show>
        <Show when=move || update_ready.get()>
            <div class="pwa-bar pwa-update">
                <span>"A new version is available."</span>
                <button on:click=move |_| reload()>"Update"</button>
            </div>
        </Show>
    }
}
