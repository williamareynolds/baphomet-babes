//! RSVP control for an event. Members see the going-count and a toggle button;
//! the deadline closes the toggle (server enforces it too). Admins get the
//! who's-going list elsewhere — this component never reveals names.

use crate::api;
use auth_client::AuthUser;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// Today's local date as "YYYY-MM-DD" so the deadline check matches the member's
/// wall clock (the server's UTC check is the backstop).
fn today_local() -> String {
    let d = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        d.get_full_year() as i64,
        d.get_month() as i64 + 1, // JS months are 0-based
        d.get_date() as i64,
    )
}

/// "2030-10-31" -> "October 31". Falls back to the raw string if malformed.
fn pretty_md(d: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    let parts: Vec<&str> = d.split('-').collect();
    match (parts.get(1), parts.get(2)) {
        (Some(m), Some(day)) => match m.parse::<usize>() {
            Ok(mi) if (1..=12).contains(&mi) => {
                format!("{} {}", MONTHS[mi - 1], day.trim_start_matches('0'))
            }
            _ => d.to_string(),
        },
        _ => d.to_string(),
    }
}

#[component]
pub fn EventRsvp(event: shared::Event, auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let id = event.id.clone();
    let deadline = event.rsvp_deadline.clone();
    let count = RwSignal::new(event.rsvp_count);
    let going = RwSignal::new(event.my_rsvp);
    let busy = RwSignal::new(false);
    let err = RwSignal::new(String::new());

    let closed = {
        let deadline = deadline.clone();
        move || deadline.as_deref().is_some_and(|d| today_local().as_str() > d)
    };

    let toggle = {
        let id = id.clone();
        move |_| {
            if busy.get_untracked() {
                return;
            }
            let Some(user) = auth.get() else { return };
            let want = !going.get_untracked();
            let id = id.clone();
            busy.set(true);
            err.set(String::new());
            spawn_local(async move {
                match api::rsvp_event(&id, want, &user.token).await {
                    Ok(ev) => {
                        count.set(ev.rsvp_count);
                        going.set(ev.my_rsvp);
                    }
                    Err(e) => err.set(e),
                }
                busy.set(false);
            });
        }
    };

    let count_label = move || {
        let n = count.get();
        if n == 1 { "1 person going".to_string() } else { format!("{n} going") }
    };

    let deadline_hint = deadline.clone().map(|d| {
        let closed = closed.clone();
        move || {
            if closed() {
                "RSVPs closed".to_string()
            } else {
                format!("RSVP by {}", pretty_md(&d))
            }
        }
    });

    view! {
        <div class="rsvp">
            <div class="rsvp-stats">
                <span class="rsvp-count">{count_label}</span>
                {deadline_hint.map(|f| view! { <span class="rsvp-deadline">{f}</span> })}
            </div>
            <Show
                when=move || !closed()
                fallback=move || view! {
                    <Show when=move || going.get()>
                        <span class="rsvp-mine">"You're going"</span>
                    </Show>
                }
            >
                <button
                    type="button"
                    class=move || if going.get() { "rsvp-btn going" } else { "rsvp-btn" }
                    disabled=move || busy.get()
                    on:click=toggle.clone()
                >
                    {move || if going.get() { "Going ✓ · tap to cancel" } else { "RSVP — I'm going" }}
                </button>
            </Show>
            <Show when=move || !err.get().is_empty()>
                <p class="error">{move || err.get()}</p>
            </Show>
        </div>
    }
}
