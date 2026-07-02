//! Mountain bike rides: post that you're heading to a trail, see who's going,
//! join with one tap. Any member can post; the creator is automatically going.

use auth_client::AuthUser;
use crate::api;
use leptos::prelude::*;
use shared::{CreateRideRequest, RIDE_LOCATIONS, Ride};
use thaw::{Button, ButtonAppearance, ButtonType, Card, Field, Input, InputType, Select};
use wasm_bindgen_futures::spawn_local;

/// Now as "YYYY-MM-DDTHH:MM" local, comparable to ride datetimes.
fn now_local() -> String {
    let d = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}",
        d.get_full_year() as i64,
        d.get_month() as i64 + 1, // JS months are 0-based
        d.get_date() as i64,
        d.get_hours() as i64,
        d.get_minutes() as i64,
    )
}

/// "2026-07-04T09:00" -> "July 4 · 9:00 AM". Falls back to the raw string.
fn pretty_dt(dt: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    let (date, time) = match dt.split_once('T') {
        Some(p) => p,
        None => return dt.to_string(),
    };
    let dparts: Vec<&str> = date.split('-').collect();
    let tparts: Vec<&str> = time.split(':').collect();
    match (dparts.get(1), dparts.get(2), tparts.first(), tparts.get(1)) {
        (Some(m), Some(day), Some(h), Some(min)) => {
            match (m.parse::<usize>(), h.parse::<i64>()) {
                (Ok(mi), Ok(hr)) if (1..=12).contains(&mi) => {
                    let (h12, ampm) = match hr {
                        0 => (12, "AM"),
                        1..=11 => (hr, "AM"),
                        12 => (12, "PM"),
                        _ => (hr - 12, "PM"),
                    };
                    format!("{} {} · {}:{} {}", MONTHS[mi - 1], day.trim_start_matches('0'), h12, min, ampm)
                }
                _ => dt.to_string(),
            }
        }
        _ => dt.to_string(),
    }
}

/// Time range label: same-day rides only repeat the clock time.
fn pretty_range(start: &str, end: &str) -> String {
    let same_day = start.split_once('T').map(|(d, _)| d) == end.split_once('T').map(|(d, _)| d);
    if same_day {
        let end_time = pretty_dt(end);
        let end_short = end_time.split_once("· ").map(|(_, t)| t.to_string()).unwrap_or(end_time);
        format!("{} – {}", pretty_dt(start), end_short)
    } else {
        format!("{} – {}", pretty_dt(start), pretty_dt(end))
    }
}

#[component]
fn RideCard(ride: Ride, auth: RwSignal<Option<AuthUser>>, on_change: Callback<()>) -> impl IntoView {
    let id = ride.id.clone();
    let going = RwSignal::new(ride.my_attending);
    let attendees = RwSignal::new(ride.attendees.clone());
    let busy = RwSignal::new(false);
    let err = RwSignal::new(String::new());

    let own = auth.get_untracked().map(|u| u.id == ride.created_by || u.is_admin()).unwrap_or(false);
    let over = ride.end_at.as_str() < now_local().as_str();

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
                match api::attend_ride(&id, want, &user.token).await {
                    Ok(r) => {
                        going.set(r.my_attending);
                        attendees.set(r.attendees);
                    }
                    Err(e) => err.set(e),
                }
                busy.set(false);
            });
        }
    };

    let remove = {
        let id = id.clone();
        move |_| {
            let Some(user) = auth.get() else { return };
            let id = id.clone();
            spawn_local(async move {
                if api::delete_ride(&id, &user.token).await.is_ok() {
                    on_change.run(());
                }
            });
        }
    };

    let count_label = move || {
        let n = attendees.get().len();
        if n == 1 { "1 rider".to_string() } else { format!("{n} riders") }
    };

    view! {
        <Card>
            <div class="mn-body">
                <h3 class="mn-title">{ride.location.clone()}</h3>
                <p class="mn-date">{pretty_range(&ride.start_at, &ride.end_at)}</p>
                <p class="mn-desc">{format!("Posted by {}", ride.created_by_name)}</p>
                <div class="rsvp">
                    <div class="rsvp-stats">
                        <span class="rsvp-count">{count_label}</span>
                        <span class="rsvp-deadline">
                            {move || attendees.get().join(", ")}
                        </span>
                    </div>
                    <Show when=move || !over>
                        <button
                            type="button"
                            class=move || if going.get() { "rsvp-btn going" } else { "rsvp-btn" }
                            disabled=move || busy.get()
                            on:click=toggle.clone()
                        >
                            {move || if going.get() { "Going ✓ · tap to bail" } else { "Join this ride" }}
                        </button>
                    </Show>
                    <Show when=move || !err.get().is_empty()>
                        <p class="error">{move || err.get()}</p>
                    </Show>
                </div>
                {own.then(|| view! {
                    <div style="margin-top:0.6rem;">
                        <Button appearance=ButtonAppearance::Secondary on_click=remove.clone()>
                            "Delete"
                        </Button>
                    </div>
                })}
            </div>
        </Card>
    }
}

#[component]
pub fn RidesPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let rides: RwSignal<Option<Result<Vec<Ride>, String>>> = RwSignal::new(None);
    let (refresh, set_refresh) = signal(0u32);

    Effect::new(move |_| {
        let _ = refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(t) = token else { return };
            let result = match api::fetch_rides(&t).await {
                Ok(list) => {
                    crate::cache::stash("rides", &list);
                    Ok(list)
                }
                Err(e) => crate::cache::recall::<Vec<Ride>>("rides").map(Ok).unwrap_or(Err(e)),
            };
            rides.set(Some(result));
        });
    });
    let on_change = Callback::new(move |_| set_refresh.update(|n| *n += 1));

    // ---- Post-a-ride form ----
    let location = RwSignal::new(RIDE_LOCATIONS[0].to_string());
    let start_at = RwSignal::new(String::new());
    let end_at = RwSignal::new(String::new());
    let (form_error, set_form_error) = signal(String::new());
    let (form_success, set_form_success) = signal(String::new());

    let handle_create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_form_error.set(String::new());
        set_form_success.set(String::new());
        let Some(user) = auth.get() else { return };
        if start_at.get().is_empty() || end_at.get().is_empty() {
            set_form_error.set("Pick a start and end time.".into());
            return;
        }
        let req = CreateRideRequest {
            location: location.get(),
            start_at: start_at.get(),
            end_at: end_at.get(),
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::create_ride(req, &user.token).await {
                Ok(_) => {
                    set_form_success.set("Ride posted — you're on the list!".into());
                    start_at.set(String::new());
                    end_at.set(String::new());
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_form_error.set(e),
            }
        });
    };

    let ride_list = move || match rides.get() {
        None => view! { <p>"Loading…"</p> }.into_any(),
        Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
        Some(Ok(list)) => {
            let now = now_local();
            let (upcoming, past): (Vec<Ride>, Vec<Ride>) =
                list.into_iter().partition(|r| r.end_at.as_str() >= now.as_str());
            let mut past = past;
            past.reverse(); // most recent first
            let past: Vec<Ride> = past.into_iter().take(10).collect();

            view! {
                <h2 class="section-heading">"Upcoming Rides"</h2>
                {if upcoming.is_empty() {
                    view! { <p class="mn-empty">"Nobody's headed out yet — post a ride!"</p> }.into_any()
                } else {
                    view! {
                        <div>
                            {upcoming.into_iter().map(|r| view! {
                                <RideCard ride=r auth=auth on_change=on_change />
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any()
                }}

                {(!past.is_empty()).then(|| view! {
                    <h2 class="section-heading">"Past Rides"</h2>
                    <div>
                        {past.into_iter().map(|r| view! {
                            <RideCard ride=r auth=auth on_change=on_change />
                        }).collect::<Vec<_>>()}
                    </div>
                })}
            }.into_any()
        }
    };

    view! {
        <main>
            <h1>"Mountain Bike Rides"</h1>
            <Show when=move || auth.get().is_some()>
                <Card>
                    <h2>"Post a Ride"</h2>
                    <form on:submit=handle_create>
                        <Field label="Where">
                            <Select value=location>
                                {RIDE_LOCATIONS.iter().map(|l| view! {
                                    <option value={*l}>{*l}</option>
                                }).collect::<Vec<_>>()}
                            </Select>
                        </Field>
                        <Field label="Rolling out">
                            <Input value=start_at input_type=InputType::DatetimeLocal />
                        </Field>
                        <Field label="Wrapping up">
                            <Input value=end_at input_type=InputType::DatetimeLocal />
                        </Field>
                        <Show when=move || !form_error.get().is_empty()>
                            <p class="error">{move || form_error.get()}</p>
                        </Show>
                        <Show when=move || !form_success.get().is_empty()>
                            <p class="success">{move || form_success.get()}</p>
                        </Show>
                        <Button button_type=ButtonType::Submit appearance=ButtonAppearance::Primary>
                            "Post Ride"
                        </Button>
                    </form>
                </Card>

                {ride_list}
            </Show>
        </main>
    }
}
