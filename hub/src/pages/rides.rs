//! Mountain bike rides: post that you're heading to a trail, see who's going,
//! join with one tap. Any member can post; the creator is automatically going.

use auth_client::AuthUser;
use crate::api;
use crate::map;
use leptos::html::Div;
use leptos::prelude::*;
use shared::{
    ContactKind, CreateRideRequest, RIDE_LOCATIONS, Ride, UpdateNotificationPrefs, UpdateRideRequest,
    classify_contact,
};
use thaw::{Button, ButtonAppearance, ButtonType, Card, Field, Input, InputType, Select, Switch, Textarea};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// Bentonville — where the picker opens before a pin is dropped.
const MAP_CENTER: (f64, f64) = (36.3729, -94.2088);
/// The one meeting-spot picker on the page.
const MAP_ID: &str = "bb-ride-map";

/// "Open in maps" links from a pin. We never embed a map on a card — these
/// hand off to the viewer's own map app, so browsing rides leaks no IPs to a
/// tile server.
fn map_links(lat: f64, lng: f64) -> impl IntoView {
    let apple = format!("https://maps.apple.com/?ll={lat},{lng}&q=Meeting%20spot");
    let google = format!("https://www.google.com/maps?q={lat},{lng}");
    let osm = format!("https://www.openstreetmap.org/?mlat={lat}&mlon={lng}#map=16/{lat}/{lng}");
    view! {
        <p class="ride-meet">
            <span class="ride-meet-label">"Meeting spot"</span>
            <a href=apple target="_blank" rel="noopener noreferrer">"Apple"</a>
            <a href=google target="_blank" rel="noopener noreferrer">"Google"</a>
            <a href=osm target="_blank" rel="noopener noreferrer">"OSM"</a>
        </p>
    }
}

/// Render free-text contact info as a tappable link when `shared` recognises it
/// — a Signal invite, any web link, an email, or a phone number — and as plain
/// (escaped) text otherwise. Classification (and the safe-scheme rule) lives in
/// `shared::classify_contact`, unit-tested there; this only maps to markup.
fn contact_view(raw: &str) -> AnyView {
    let s = raw.trim();
    match classify_contact(s) {
        ContactKind::Signal => view! {
            <a class="ride-contact-btn" href=s.to_string() target="_blank" rel="noopener noreferrer">
                "Join Signal group"
            </a>
        }
        .into_any(),
        ContactKind::Web => view! {
            <a class="ride-contact-link" href=s.to_string() target="_blank" rel="noopener noreferrer">
                {s.to_string()}
            </a>
        }
        .into_any(),
        ContactKind::Email => {
            view! { <a class="ride-contact-link" href=format!("mailto:{s}")>{s.to_string()}</a> }
                .into_any()
        }
        ContactKind::Phone(tel) => {
            view! { <a class="ride-contact-link" href=format!("tel:{tel}")>{s.to_string()}</a> }
                .into_any()
        }
        ContactKind::Plain if s.is_empty() => ().into_any(),
        ContactKind::Plain => view! { <span>{s.to_string()}</span> }.into_any(),
    }
}

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
    // The id in a StoredValue (Copy) so the handler closures below capture only
    // Copy state — that keeps them Copy, which the reactive card body (which
    // re-runs on the edit toggle) needs to reuse them across renders.
    let ride_id = StoredValue::new(ride.id.clone());
    let going = RwSignal::new(ride.my_attending);
    let attendees = RwSignal::new(ride.attendees.clone());
    let busy = RwSignal::new(false);
    let err = RwSignal::new(String::new());

    let own = auth.get_untracked().map(|u| u.id == ride.created_by || u.is_admin()).unwrap_or(false);
    let over = ride.end_at.as_str() < now_local().as_str();

    // The full ride, kept for prefilling the edit form on demand.
    let ride_data = StoredValue::new(ride.clone());

    let toggle = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some(user) = auth.get() else { return };
        let want = !going.get_untracked();
        let id = ride_id.get_value();
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
    };

    let remove = move |_| {
        let Some(user) = auth.get() else { return };
        let id = ride_id.get_value();
        spawn_local(async move {
            if api::delete_ride(&id, &user.token).await.is_ok() {
                on_change.run(());
            }
        });
    };

    let count_label = move || {
        let n = attendees.get().len();
        if n == 1 { "1 rider".to_string() } else { format!("{n} riders") }
    };

    // ---- Inline edit (creator or admin) ----
    let editing = RwSignal::new(false);
    let edit_location = RwSignal::new(String::new());
    let edit_start = RwSignal::new(String::new());
    let edit_end = RwSignal::new(String::new());
    let edit_contact = RwSignal::new(String::new());
    let edit_notes = RwSignal::new(String::new());
    let edit_lat: RwSignal<Option<f64>> = RwSignal::new(None);
    let edit_lng: RwSignal<Option<f64>> = RwSignal::new(None);
    let (edit_error, set_edit_error) = signal(String::new());
    let edit_busy = RwSignal::new(false);

    // A Leaflet picker per editing card (unique id → no clash with the create
    // map or other cards). Built when edit opens, torn down when it closes. The
    // id lives in a StoredValue so every closure below can share it by copy.
    let edit_map_id = StoredValue::new(format!("bb-ride-map-edit-{id}"));
    let edit_map_ref: NodeRef<Div> = NodeRef::new();
    let edit_map_inited = StoredValue::new(false);
    Effect::new(move |_| {
        if editing.get() {
            if edit_map_ref.get().is_none() || edit_map_inited.get_value() {
                return;
            }
            edit_map_inited.set_value(true);
            let seed = edit_lat.get_untracked().is_some();
            let (clat, clng) = match (edit_lat.get_untracked(), edit_lng.get_untracked()) {
                (Some(la), Some(ln)) => (la, ln),
                _ => MAP_CENTER,
            };
            let on_pick = Closure::<dyn FnMut(f64, f64)>::new(move |la: f64, ln: f64| {
                edit_lat.set(Some(la));
                edit_lng.set(Some(ln));
            });
            map::init(&edit_map_id.get_value(), clat, clng, seed, &on_pick);
            on_pick.forget();
        } else if edit_map_inited.get_value() {
            // Closed the form — drop the map so re-opening rebuilds it fresh.
            map::destroy(&edit_map_id.get_value());
            edit_map_inited.set_value(false);
        }
    });

    let clear_edit_pin = move |_| {
        map::clear(&edit_map_id.get_value());
        edit_lat.set(None);
        edit_lng.set(None);
    };

    let start_edit = move |_| {
        let r = ride_data.get_value();
        edit_location.set(r.location);
        edit_start.set(r.start_at);
        edit_end.set(r.end_at);
        edit_contact.set(r.contact_info.unwrap_or_default());
        edit_notes.set(r.notes.unwrap_or_default());
        edit_lat.set(r.meeting_lat);
        edit_lng.set(r.meeting_lng);
        set_edit_error.set(String::new());
        editing.set(true);
    };

    let submit_edit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if edit_busy.get_untracked() {
            return;
        }
        let Some(user) = auth.get() else { return };
        if edit_start.get().is_empty() || edit_end.get().is_empty() {
            set_edit_error.set("Pick a start and end time.".into());
            return;
        }
        // A pin set → send its coords; none → clear it (None means "keep").
        let (meeting_lat, meeting_lng, clear_meeting) = match (edit_lat.get(), edit_lng.get()) {
            (Some(la), Some(ln)) => (Some(la), Some(ln), false),
            _ => (None, None, true),
        };
        // contact/notes always sent as Some so an emptied field clears it.
        let req = UpdateRideRequest {
            location: Some(edit_location.get()),
            start_at: Some(edit_start.get()),
            end_at: Some(edit_end.get()),
            meeting_lat,
            meeting_lng,
            clear_meeting,
            contact_info: Some(edit_contact.get()),
            notes: Some(edit_notes.get()),
        };
        let id = ride_id.get_value();
        edit_busy.set(true);
        set_edit_error.set(String::new());
        spawn_local(async move {
            match api::update_ride(&id, req, &user.token).await {
                Ok(_) => {
                    editing.set(false);
                    on_change.run(());
                }
                Err(e) => set_edit_error.set(e),
            }
            edit_busy.set(false);
        });
    };

    view! {
        <Card>
            {move || {
                if editing.get() {
                    view! {
                        <form class="mn-body" on:submit=submit_edit.clone()>
                            <Field label="Where">
                                <Select value=edit_location>
                                    {RIDE_LOCATIONS.iter().map(|l| view! {
                                        <option value={*l}>{*l}</option>
                                    }).collect::<Vec<_>>()}
                                </Select>
                            </Field>
                            <Field label="Rolling out">
                                <Input value=edit_start input_type=InputType::DatetimeLocal />
                            </Field>
                            <Field label="Wrapping up">
                                <Input value=edit_end input_type=InputType::DatetimeLocal />
                            </Field>
                            <Field label="Meeting spot (optional)">
                                <div node_ref=edit_map_ref id=edit_map_id.get_value() class="ride-map"></div>
                                <div class="ride-map-status">
                                    <Show
                                        when=move || edit_lat.get().is_some()
                                        fallback=|| view! {
                                            <span class="ride-map-hint">"Tap the map to drop a pin"</span>
                                        }
                                    >
                                        <span class="ride-map-hint">"Pin dropped ✓"</span>
                                        <button type="button" class="ride-map-clear" on:click=clear_edit_pin.clone()>
                                            "Clear"
                                        </button>
                                    </Show>
                                </div>
                            </Field>
                            <Field label="Contact (optional)">
                                <Input value=edit_contact placeholder="phone, email, or a group-chat link" />
                            </Field>
                            <Field label="Additional info (optional)">
                                <Textarea value=edit_notes placeholder="Weather call, landmarks to find the group, pace…" />
                            </Field>
                            <Show when=move || !edit_error.get().is_empty()>
                                <p class="error">{move || edit_error.get()}</p>
                            </Show>
                            <div class="admin-actions">
                                <Button button_type=ButtonType::Submit appearance=ButtonAppearance::Primary>"Save"</Button>
                                <Button
                                    button_type=ButtonType::Button
                                    appearance=ButtonAppearance::Secondary
                                    on_click=move |_| editing.set(false)
                                >"Cancel"</Button>
                            </div>
                        </form>
                    }.into_any()
                } else {
                    let r = ride_data.get_value();
                    view! {
                        <div class="mn-body">
                            <h3 class="mn-title">{r.location.clone()}</h3>
                            <p class="mn-date">{pretty_range(&r.start_at, &r.end_at)}</p>
                            <p class="mn-desc">{format!("Posted by {}", r.created_by_name)}</p>
                            {r.meeting_lat.zip(r.meeting_lng).map(|(la, ln)| map_links(la, ln))}
                            {r.contact_info.clone().filter(|c| !c.trim().is_empty()).map(|c| view! {
                                <p class="ride-contact">
                                    <span class="ride-contact-cap">"Contact"</span>
                                    {contact_view(&c)}
                                </p>
                            })}
                            {r.notes.clone().filter(|n| !n.trim().is_empty()).map(|n| view! {
                                <p class="ride-notes">{n}</p>
                            })}
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
                                <div class="admin-actions" style="margin-top:0.6rem;">
                                    <Button appearance=ButtonAppearance::Secondary on_click=start_edit>
                                        "Edit"
                                    </Button>
                                    <Button appearance=ButtonAppearance::Secondary on_click=remove.clone()>
                                        "Delete"
                                    </Button>
                                </div>
                            })}
                        </div>
                    }.into_any()
                }
            }}
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

    // ---- Mountain-bike notification channel toggle ----
    // The channel is opt-in (off by default), so members who only visit this
    // page would never discover it buried in profile settings. Saves on flip.
    let notif_on = RwSignal::new(false);
    let notif_loaded = RwSignal::new(false);
    // Last value persisted to the server; guards the save effect against
    // firing for the initial load (and against redundant writes).
    let notif_saved: RwSignal<Option<bool>> = RwSignal::new(None);

    Effect::new(move |_| {
        let Some(user) = auth.get() else { return };
        spawn_local(async move {
            if let Ok(p) = api::fetch_notif_prefs(&user.token).await {
                notif_saved.set(Some(p.mountain_bike));
                notif_on.set(p.mountain_bike);
                notif_loaded.set(true);
            }
        });
    });

    Effect::new(move |_| {
        let want = notif_on.get();
        if !notif_loaded.get_untracked() || notif_saved.get_untracked() == Some(want) {
            return;
        }
        let Some(user) = auth.get_untracked() else { return };
        notif_saved.set(Some(want));
        spawn_local(async move {
            let req = UpdateNotificationPrefs { mountain_bike: Some(want), ..Default::default() };
            let _ = api::update_notif_prefs(req, &user.token).await;
        });
    });

    // ---- Post-a-ride form ----
    let location = RwSignal::new(RIDE_LOCATIONS[0].to_string());
    let start_at = RwSignal::new(String::new());
    let end_at = RwSignal::new(String::new());
    let contact = RwSignal::new(String::new());
    let notes = RwSignal::new(String::new());
    // The meeting-spot pin. Both set together (on tap) or both None (no spot).
    let meet_lat: RwSignal<Option<f64>> = RwSignal::new(None);
    let meet_lng: RwSignal<Option<f64>> = RwSignal::new(None);
    let (form_error, set_form_error) = signal(String::new());
    let (form_success, set_form_success) = signal(String::new());

    // Spin up the Leaflet picker once its div mounts. The click closure records
    // the pin into the signals above; `forget()` hands it to JS for the map's
    // lifetime. Guarded so it initialises exactly once.
    let map_ref: NodeRef<Div> = NodeRef::new();
    let map_inited = StoredValue::new(false);
    Effect::new(move |_| {
        if map_ref.get().is_none() || map_inited.get_value() {
            return;
        }
        map_inited.set_value(true);
        let on_pick = Closure::<dyn FnMut(f64, f64)>::new(move |la: f64, ln: f64| {
            meet_lat.set(Some(la));
            meet_lng.set(Some(ln));
        });
        map::init(MAP_ID, MAP_CENTER.0, MAP_CENTER.1, false, &on_pick);
        on_pick.forget();
    });

    let clear_pin = move |_| {
        map::clear(MAP_ID);
        meet_lat.set(None);
        meet_lng.set(None);
    };

    let handle_create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_form_error.set(String::new());
        set_form_success.set(String::new());
        let Some(user) = auth.get() else { return };
        if start_at.get().is_empty() || end_at.get().is_empty() {
            set_form_error.set("Pick a start and end time.".into());
            return;
        }
        let contact_info = {
            let t = contact.get().trim().to_string();
            (!t.is_empty()).then_some(t)
        };
        let notes_val = {
            let t = notes.get().trim().to_string();
            (!t.is_empty()).then_some(t)
        };
        let req = CreateRideRequest {
            location: location.get(),
            start_at: start_at.get(),
            end_at: end_at.get(),
            meeting_lat: meet_lat.get(),
            meeting_lng: meet_lng.get(),
            contact_info,
            notes: notes_val,
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::create_ride(req, &user.token).await {
                Ok(_) => {
                    set_form_success.set("Ride posted — you're on the list!".into());
                    start_at.set(String::new());
                    end_at.set(String::new());
                    contact.set(String::new());
                    notes.set(String::new());
                    meet_lat.set(None);
                    meet_lng.set(None);
                    map::clear(MAP_ID);
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
                <Show when=move || notif_loaded.get()>
                    <div class="ride-notify">
                        <Switch checked=notif_on label="Notify me when someone posts a ride" />
                    </div>
                </Show>

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
                        <Field label="Meeting spot (optional)">
                            <div node_ref=map_ref id=MAP_ID class="ride-map"></div>
                            <div class="ride-map-status">
                                <Show
                                    when=move || meet_lat.get().is_some()
                                    fallback=|| view! {
                                        <span class="ride-map-hint">"Tap the map to drop a pin"</span>
                                    }
                                >
                                    <span class="ride-map-hint">"Pin dropped ✓"</span>
                                    <button type="button" class="ride-map-clear" on:click=clear_pin>
                                        "Clear"
                                    </button>
                                </Show>
                            </div>
                        </Field>
                        <Field label="Contact (optional)">
                            <Input value=contact placeholder="phone, email, or a group-chat link" />
                        </Field>
                        <Field label="Additional info (optional)">
                            <Textarea value=notes placeholder="Weather call, landmarks to find the group, pace…" />
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
