//! Gatherings: club get-togethers with a stated date, time and place.
//!
//! Members see the list and RSVP; the count is public, the names are not (the
//! backend refuses the names endpoint to anyone below admin). Admins post from
//! the same page rather than a separate admin screen, mirroring how rides work.

use crate::api;
use crate::map;
use auth_client::AuthUser;
use leptos::html::Div;
use leptos::prelude::*;
use shared::{CreateGatheringRequest, Gathering, Rsvp};
use thaw::{Button, ButtonAppearance, ButtonType, Card, Field, Input, InputType, Textarea};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

/// Bentonville — the same default the ride picker opens on.
const MAP_CENTER: (f64, f64) = (36.3729, -94.2088);
const MAP_ID: &str = "bb-gathering-map";

/// "2026-09-01T18:30" -> "September 1, 2026 · 6:30 PM". Falls back to the raw
/// value if the shape isn't what we expect — stored data is never trusted.
fn pretty_when(s: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    let Some((date, time)) = s.split_once('T') else { return s.to_string() };
    let d: Vec<&str> = date.split('-').collect();
    let t: Vec<&str> = time.split(':').collect();
    match (d.first(), d.get(1), d.get(2), t.first(), t.get(1)) {
        (Some(y), Some(m), Some(day), Some(h), Some(min)) => {
            let (Ok(mi), Ok(hh)) = (m.parse::<usize>(), h.parse::<u32>()) else {
                return s.to_string();
            };
            if !(1..=12).contains(&mi) || hh > 23 {
                return s.to_string();
            }
            let suffix = if hh < 12 { "AM" } else { "PM" };
            let h12 = match hh % 12 { 0 => 12, other => other };
            format!(
                "{} {}, {} · {}:{} {}",
                MONTHS[mi - 1],
                day.trim_start_matches('0'),
                y,
                h12,
                min,
                suffix
            )
        }
        _ => s.to_string(),
    }
}

/// Link out to whichever maps app the device prefers, rather than embedding a
/// map per card — an embed would leak every viewer's IP to a tile server.
fn maps_link(g: &Gathering) -> Option<String> {
    match (g.lat, g.lng) {
        (Some(lat), Some(lng)) => Some(format!("https://www.google.com/maps/search/?api=1&query={lat},{lng}")),
        _ => g.address.as_ref().map(|a| {
            format!("https://www.google.com/maps/search/?api=1&query={}", urlencode(a))
        }),
    }
}

/// Minimal percent-encoding for a query value. Only the characters an address
/// realistically contains need escaping.
fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            other => other
                .to_string()
                .bytes()
                .map(|b| format!("%{b:02X}"))
                .collect::<String>(),
        })
        .collect()
}

#[component]
pub fn GatheringsPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let is_admin = move || auth.get().map(|u| u.is_admin()).unwrap_or(false);
    let (refresh, set_refresh) = signal(0u32);
    let gatherings: RwSignal<Option<Result<Vec<Gathering>, String>>> = RwSignal::new(None);

    Effect::new(move |_| {
        let _ = refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(t) = token else { return };
            gatherings.set(Some(api::fetch_gatherings(&t).await));
        });
    });

    // ---- Create form (admins only) ----
    let title = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let starts_at = RwSignal::new(String::new());
    let ends_at = RwSignal::new(String::new());
    let address = RwSignal::new(String::new());
    let lat: RwSignal<Option<f64>> = RwSignal::new(None);
    let lng: RwSignal<Option<f64>> = RwSignal::new(None);
    let cover_url = RwSignal::new(String::new());
    let (form_error, set_form_error) = signal(String::new());
    let (form_note, set_form_note) = signal(String::new());
    let busy = RwSignal::new(false);
    let form_open = RwSignal::new(false);

    // Leaflet picker. `forget()` hands the closure to JS for the map's life;
    // guarded so it initialises exactly once.
    let map_ref: NodeRef<Div> = NodeRef::new();
    let map_inited = StoredValue::new(false);
    Effect::new(move |_| {
        if map_ref.get().is_none() || map_inited.get_value() {
            return;
        }
        map_inited.set_value(true);
        let on_pick = Closure::<dyn FnMut(f64, f64)>::new(move |la: f64, ln: f64| {
            lat.set(Some(la));
            lng.set(Some(ln));
        });
        map::init(MAP_ID, MAP_CENTER.0, MAP_CENTER.1, false, &on_pick);
        on_pick.forget();
    });

    // The picker mounts hidden; nudge Leaflet to remeasure when it's revealed
    // so tiles lay out against the real size.
    Effect::new(move |_| {
        if form_open.get() {
            map::refresh(MAP_ID);
        }
    });

    let clear_pin = move |_| {
        map::clear(MAP_ID);
        lat.set(None);
        lng.set(None);
    };

    // Geocode the typed address into a pin. A miss is a note, not an error —
    // the pin stays hand-droppable.
    let find_on_map = move |_| {
        let Some(user) = auth.get() else { return };
        let q = address.get();
        if q.trim().is_empty() {
            set_form_note.set("Type an address first.".into());
            return;
        }
        set_form_note.set("Looking up that address…".into());
        wasm_bindgen_futures::spawn_local(async move {
            match api::geocode_address(&q, &user.token).await {
                Ok(r) if r.found => {
                    if let (Some(la), Some(ln)) = (r.lat, r.lng) {
                        lat.set(Some(la));
                        lng.set(Some(ln));
                        map::init(MAP_ID, la, ln, true, &Closure::<dyn FnMut(f64, f64)>::new(
                            move |a: f64, b: f64| {
                                lat.set(Some(a));
                                lng.set(Some(b));
                            },
                        ));
                        set_form_note.set(match r.display_name {
                            Some(n) if !n.is_empty() => format!("Found: {n} — drag-tap the map to adjust."),
                            _ => "Pin placed — tap the map to adjust.".into(),
                        });
                    }
                }
                Ok(_) => set_form_note.set(
                    "Couldn't find that address. Drop the pin on the map instead.".into(),
                ),
                Err(e) => set_form_note.set(format!("Lookup failed: {e}")),
            }
        });
    };

    // Cover upload: read the file as a data URL, strip the prefix, and post the
    // base64 to the API, which validates type and size before storing.
    let on_cover_change = move |ev: leptos::ev::Event| {
        let Some(user) = auth.get() else { return };
        let Some(input) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(file) = input.files().and_then(|f| f.get(0)) else { return };
        let content_type = file.type_();
        set_form_note.set("Uploading cover…".into());

        let reader = web_sys::FileReader::new().unwrap();
        let reader_c = reader.clone();
        let onload = Closure::<dyn FnMut()>::new(move || {
            let Ok(value) = reader_c.result() else { return };
            let Some(data_url) = value.as_string() else { return };
            // "data:image/png;base64,AAAA…" — everything after the comma.
            let Some((_, b64)) = data_url.split_once(',') else { return };
            let b64 = b64.to_string();
            let ct = content_type.clone();
            let token = user.token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::upload_gathering_cover(&ct, &b64, &token).await {
                    Ok(r) => {
                        cover_url.set(r.url);
                        set_form_note.set("Cover uploaded.".into());
                    }
                    Err(e) => set_form_note.set(format!("Upload failed: {e}")),
                }
            });
        });
        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();
        let _ = reader.read_as_data_url(&file);
    };

    let handle_create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(user) = auth.get() else { return };
        set_form_error.set(String::new());

        // Validate with the same rules the API enforces, so the form fails fast
        // and with identical wording.
        let addr = address.get();
        if let Err(e) = shared::validate_gathering(
            &title.get(),
            &starts_at.get(),
            Some(ends_at.get()).as_deref().filter(|s| !s.is_empty()),
            shared::GatheringPlace {
                address: Some(addr.as_str()).filter(|a| !a.trim().is_empty()),
                lat: lat.get(),
                lng: lng.get(),
            },
        ) {
            set_form_error.set(e);
            return;
        }

        let req = CreateGatheringRequest {
            title: title.get(),
            description: Some(description.get()).filter(|d| !d.is_empty()),
            starts_at: starts_at.get(),
            ends_at: Some(ends_at.get()).filter(|d| !d.is_empty()),
            address: Some(addr).filter(|a| !a.trim().is_empty()),
            lat: lat.get(),
            lng: lng.get(),
            cover_url: Some(cover_url.get()).filter(|c| !c.is_empty()),
        };
        busy.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            match api::create_gathering(req, &user.token).await {
                Ok(_) => {
                    title.set(String::new());
                    description.set(String::new());
                    starts_at.set(String::new());
                    ends_at.set(String::new());
                    address.set(String::new());
                    cover_url.set(String::new());
                    lat.set(None);
                    lng.set(None);
                    map::clear(MAP_ID);
                    set_form_note.set("Gathering posted.".into());
                    form_open.set(false);
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_form_error.set(e),
            }
            busy.set(false);
        });
    };

    let handle_delete = move |id: String| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if api::delete_gathering(&id, &user.token).await.is_ok() {
                set_refresh.update(|n| *n += 1);
            }
        });
    };

    // Admin-only attendee list, opened per gathering.
    let open_rsvps: RwSignal<Option<String>> = RwSignal::new(None);
    let rsvp_list: RwSignal<Option<Result<Vec<Rsvp>, String>>> = RwSignal::new(None);
    let view_rsvps = move |id: String| {
        let Some(user) = auth.get() else { return };
        if open_rsvps.get() == Some(id.clone()) {
            open_rsvps.set(None);
            return;
        }
        open_rsvps.set(Some(id.clone()));
        rsvp_list.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            rsvp_list.set(Some(api::fetch_gathering_rsvps(&id, &user.token).await));
        });
    };

    let toggle_rsvp = move |id: String, going: bool| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if api::rsvp_gathering(&id, going, &user.token).await.is_ok() {
                set_refresh.update(|n| *n += 1);
            }
        });
    };

    view! {
        <main class="page">
            <h1 class="page-title">"Gatherings"</h1>
            <p class="page-sub">"Club get-togethers. Date, time and place are set — no voting."</p>

            <Show when=is_admin>
                <div style="margin:1.5rem 0;">
                    <Button appearance=ButtonAppearance::Primary
                        on_click=move |_| form_open.update(|o| *o = !*o)>
                        {move || if form_open.get() { "Close" } else { "Post a Gathering" }}
                    </Button>
                </div>
            </Show>

            // The form stays mounted while hidden so the Leaflet picker keeps
            // its state; `display:none` rather than an unmount.
            <div style:display=move || if form_open.get() && is_admin() { "block" } else { "none" }>
                <Card>
                    <form on:submit=handle_create>
                        <Field label="What is it?">
                            <Input value=title placeholder="Bonfire at the quarry" />
                        </Field>
                        <Field label="Details (optional)">
                            <Textarea value=description />
                        </Field>
                        <Field label="Starts">
                            <Input value=starts_at input_type=InputType::DatetimeLocal />
                        </Field>
                        <Field label="Ends (optional)">
                            <Input value=ends_at input_type=InputType::DatetimeLocal />
                        </Field>
                        <Field label="Address">
                            <Input value=address placeholder="905 NW 10th St, Bentonville" />
                        </Field>
                        <div style="margin:0.5rem 0 1rem;display:flex;gap:0.5rem;flex-wrap:wrap;">
                            <Button on_click=find_on_map>"Find on map"</Button>
                            <Button on_click=clear_pin>"Clear pin"</Button>
                        </div>
                        <p class="field-hint">
                            "An address or a pin is required — both is best, so people can navigate either way."
                        </p>
                        <div node_ref=map_ref id=MAP_ID class="ride-map"></div>
                        {move || match (lat.get(), lng.get()) {
                            (Some(la), Some(ln)) => view! {
                                <p class="field-hint">{format!("Pin: {la:.5}, {ln:.5}")}</p>
                            }.into_any(),
                            _ => view! { <p class="field-hint">"No pin dropped."</p> }.into_any(),
                        }}

                        <Field label="Cover image (optional)">
                            <input type="file" accept="image/*" on:change=on_cover_change />
                        </Field>
                        {move || (!cover_url.get().is_empty()).then(|| view! {
                            <img src={cover_url.get()} alt="cover preview" class="gathering-cover-preview" />
                        })}

                        <Show when=move || !form_note.get().is_empty()>
                            <p class="success">{move || form_note.get()}</p>
                        </Show>
                        <Show when=move || !form_error.get().is_empty()>
                            <p class="error">{move || form_error.get()}</p>
                        </Show>

                        <div style="margin-top:1rem;">
                            <Button
                                button_type=ButtonType::Submit
                                appearance=ButtonAppearance::Primary
                                disabled=Signal::derive(move || busy.get())
                            >
                                {move || if busy.get() { "Posting…" } else { "Post Gathering" }}
                            </Button>
                        </div>
                    </form>
                </Card>
            </div>

            {move || match gatherings.get() {
                None => view! { <p>"Loading…"</p> }.into_any(),
                Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="mn-empty">"Nothing on the calendar yet."</p> }.into_any()
                }
                Some(Ok(list)) => view! {
                    <div>
                        {list.into_iter().map(|g| {
                            let id = g.id.clone();
                            let id_rsvp = g.id.clone();
                            let id_view = g.id.clone();
                            let id_del = g.id.clone();
                            let going = g.my_rsvp;
                            let link = maps_link(&g);
                            view! {
                                <Card>
                                    {g.cover_url.clone().map(|url| view! {
                                        <img src={url} alt="cover" class="gathering-cover" />
                                    })}
                                    <h3 class="mn-title">{g.title.clone()}</h3>
                                    <p class="mn-date">{pretty_when(&g.starts_at)}</p>
                                    {g.ends_at.as_deref().map(|e| view! {
                                        <p class="mn-date">{format!("until {}", pretty_when(e))}</p>
                                    })}
                                    {g.address.clone().map(|a| view! { <p class="mn-desc">{a}</p> })}
                                    {link.map(|href| view! {
                                        <p><a href={href} target="_blank" rel="noopener">"Open in maps →"</a></p>
                                    })}
                                    {g.description.clone().map(|d| view! { <p class="mn-desc">{d}</p> })}

                                    <p class="rsvp-count">
                                        {match g.rsvp_count {
                                            0 => "Nobody going yet".to_string(),
                                            1 => "1 person going".to_string(),
                                            n => format!("{n} people going"),
                                        }}
                                    </p>
                                    <Button
                                        class="rsvp-btn"
                                        appearance=if going { ButtonAppearance::Secondary } else { ButtonAppearance::Primary }
                                        on_click=move |_| toggle_rsvp(id_rsvp.clone(), !going)
                                    >
                                        {if going { "Going ✓" } else { "RSVP" }}
                                    </Button>

                                    <Show when=is_admin>
                                        <div style="margin-top:0.75rem;display:flex;gap:0.5rem;flex-wrap:wrap;">
                                            <Button on_click={
                                                let id = id_view.clone();
                                                move |_| view_rsvps(id.clone())
                                            }>"View RSVPs"</Button>
                                            <Button on_click={
                                                let id = id_del.clone();
                                                move |_| handle_delete(id.clone())
                                            }>"Delete"</Button>
                                        </div>
                                        // StoredValue so the reactive closure can
                                        // read the id repeatedly without moving it.
                                        {let id = StoredValue::new(id.clone());
                                         move || (open_rsvps.get() == Some(id.get_value())).then(|| {
                                            match rsvp_list.get() {
                                                None => view! { <p>"Loading…"</p> }.into_any(),
                                                Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                                                Some(Ok(names)) if names.is_empty() => {
                                                    view! { <p class="mn-desc">"Nobody yet."</p> }.into_any()
                                                }
                                                Some(Ok(names)) => view! {
                                                    <ul class="rsvp-names">
                                                        {names.into_iter().map(|r| view! {
                                                            <li>{r.author}</li>
                                                        }).collect::<Vec<_>>()}
                                                    </ul>
                                                }.into_any(),
                                            }
                                        })}
                                    </Show>
                                </Card>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any(),
            }}
        </main>
    }
}
