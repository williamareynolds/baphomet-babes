//! A slim onboarding nudge prompting logged-in members to install the PWA and
//! enable push notifications. Dismissible, and never shown once notifications
//! are granted (or hard-denied — we don't nag).

use auth_client::{AuthUser, enable_push, notif_permission};
use crate::api;
use leptos::prelude::*;
use thaw::{Button, ButtonAppearance, ButtonSize};
use wasm_bindgen_futures::spawn_local;

const DISMISS_KEY: &str = "notif_onboard_dismissed";

fn read_dismissed() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(DISMISS_KEY).ok().flatten())
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn write_dismissed() {
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.set_item(DISMISS_KEY, "1");
    }
}

#[component]
pub fn NotifyOnboard(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let permission = RwSignal::new(notif_permission());
    let dismissed = RwSignal::new(read_dismissed());

    // Show only to logged-in members who can still act on it: permission is
    // "default" (can ask) or "unsupported" (needs Home-Screen install on iOS).
    let can_ask = move || permission.get() == "default";
    let unsupported = move || permission.get() == "unsupported";
    let visible = move || {
        auth.get().is_some() && !dismissed.get() && (can_ask() || unsupported())
    };

    let on_enable = move |_| {
        let Some(user) = auth.get() else { return };
        spawn_local(async move {
            if let Some(tok) = enable_push().await {
                let _ = api::register_push_token(&tok, &user.token).await;
                crate::push::save(&tok);
            }
            // Reflect the new permission state; if granted, the bar disappears.
            permission.set(notif_permission());
        });
    };

    let on_dismiss = move |_| {
        write_dismissed();
        dismissed.set(true);
    };

    view! {
        <Show when=visible>
            <div class="pwa-bar notif-onboard">
                <Show
                    when=can_ask
                    fallback=move || view! {
                        <span>
                            "Add Baphomet Babes to your Home Screen to turn on notifications."
                        </span>
                    }
                >
                    <span>"Get notified about announcements, broadcasts, and movie nights."</span>
                    <Button
                        appearance=ButtonAppearance::Primary
                        size=ButtonSize::Small
                        on_click=on_enable
                    >"Enable"</Button>
                </Show>
                <Button
                    appearance=ButtonAppearance::Secondary
                    size=ButtonSize::Small
                    on_click=on_dismiss
                >"Dismiss"</Button>
            </div>
        </Show>
    }
}
