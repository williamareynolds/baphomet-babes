use crate::api;
use auth_client::AuthUser;
use leptos::prelude::*;
use thaw::{Button, ButtonAppearance, Card};

/// Percent-encode for use inside a URL query parameter (delegates to the
/// browser's encodeURIComponent).
fn enc(s: &str) -> String {
    js_sys::encode_uri_component(s).as_string().unwrap_or_default()
}

/// Subscribe-to-calendar card: shows the member's personal .ics feed URL plus
/// one-tap subscribe links for Apple/iCloud (webcal), Google, and Outlook, a
/// copy button, and a regenerate (revoke) control.
#[component]
pub fn CalendarSubscribe(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let feed_token: RwSignal<Option<String>> = RwSignal::new(None);
    let copied = RwSignal::new(false);
    let busy = RwSignal::new(false);

    Effect::new(move |_| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(t) = api::get_calendar_token(&user.token).await {
                feed_token.set(Some(t.token));
            }
        });
    });

    // Derived URLs. webcal:// is what Apple/iCloud and most desktop calendar
    // apps register from; Google/Outlook take the https URL as a parameter.
    let https_url = move || feed_token.get().map(|t| api::calendar_feed_url(&t));
    let webcal_url = move || {
        https_url().map(|u| {
            u.replacen("https://", "webcal://", 1)
                .replacen("http://", "webcal://", 1)
        })
    };
    let google_url = move || webcal_url().map(|w| format!("https://calendar.google.com/calendar/r?cid={}", enc(&w)));
    let outlook_url = move || {
        https_url().map(|u| {
            format!(
                "https://outlook.live.com/calendar/0/addfromweb?url={}&name={}",
                enc(&u),
                enc("Baphomet Babes")
            )
        })
    };

    let copy = move |_| {
        let Some(url) = https_url() else { return };
        if let Some(win) = web_sys::window() {
            let _ = win.navigator().clipboard().write_text(&url);
            copied.set(true);
        }
    };

    let regenerate = move |_| {
        let Some(user) = auth.get() else { return };
        busy.set(true);
        copied.set(false);
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(t) = api::regenerate_calendar_token(&user.token).await {
                feed_token.set(Some(t.token));
            }
            busy.set(false);
        });
    };

    view! {
        <Card>
            <h2 style="margin-bottom:0.35rem;">"Subscribe to the calendar"</h2>
            <p style="color:#bdafb2;margin-bottom:1rem;">
                "Add every movie night and event to your own calendar. It stays up to "
                "date automatically as new screenings are posted."
            </p>

            <Show
                when=move || feed_token.get().is_some()
                fallback=|| view! {
                    <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;color:#95868f;">
                        "Loading your subscription link…"
                    </p>
                }
            >
                <div class="cal-actions">
                    {move || webcal_url().map(|w| view! {
                        <a href={w} class="cal-btn cal-btn-primary">"Apple / iCloud"</a>
                    })}
                    {move || google_url().map(|g| view! {
                        <a href={g} target="_blank" rel="noopener" class="cal-btn">"Google"</a>
                    })}
                    {move || outlook_url().map(|o| view! {
                        <a href={o} target="_blank" rel="noopener" class="cal-btn">"Outlook"</a>
                    })}
                </div>

                <p class="cal-label">"Or paste this link into any calendar app"</p>
                <div class="cal-url-row">
                    <code class="cal-url">{move || https_url().unwrap_or_default()}</code>
                    <Button appearance=ButtonAppearance::Secondary on_click=copy>
                        {move || if copied.get() { "Copied" } else { "Copy" }}
                    </Button>
                </div>

                <p class="cal-note">
                    "This link is private to you. If it leaks, regenerate it — the old "
                    "link stops working immediately."
                </p>
                <Button appearance=ButtonAppearance::Secondary loading=busy disabled=busy on_click=regenerate>
                    "Regenerate link"
                </Button>
            </Show>
        </Card>
    }
}
