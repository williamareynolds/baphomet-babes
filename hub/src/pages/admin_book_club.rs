//! Admin control panel for the Book Club: current book, the (admin-only)
//! queue, and the next meetup. Mirrors `gatherings.rs`'s in-place forms and
//! Leaflet pin picker for the meetup location.

use auth_client::AuthUser;
use crate::api;
use crate::components::admin_nav::AdminNav;
use crate::map;
use leptos::html::Div;
use leptos::prelude::*;
use shared::{BookClubBook, CreateBookClubBookRequest, MoveDirection};
use thaw::{Button, ButtonAppearance, ButtonType, Card, Field, Input, InputType, Textarea};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

const MAP_CENTER: (f64, f64) = (36.3729, -94.2088);
const MAP_ID: &str = "bb-book-club-map";

#[component]
pub fn AdminBookClubPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let is_superadmin = move || auth.get().map(|u| u.is_superadmin()).unwrap_or(false);
    let (refresh, set_refresh) = signal(0u32);

    let current: RwSignal<Option<Result<Option<BookClubBook>, String>>> = RwSignal::new(None);
    let queue: RwSignal<Option<Result<Vec<BookClubBook>, String>>> = RwSignal::new(None);
    let (note, set_note) = signal(String::new());
    let (err, set_err) = signal(String::new());

    Effect::new(move |_| {
        let _ = refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(t) = token else { return };
            match api::fetch_book_club_current(&t).await {
                Ok(s) => current.set(Some(Ok(s.current))),
                Err(e) => current.set(Some(Err(e))),
            }
        });
    });

    Effect::new(move |_| {
        let _ = refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(t) = token else { return };
            queue.set(Some(api::fetch_book_club_queue(&t).await));
        });
    });

    // ---- Current book edit ----
    let editing_current = RwSignal::new(false);
    let cur_title = RwSignal::new(String::new());
    let cur_author = RwSignal::new(String::new());
    let cur_published = RwSignal::new(String::new());
    let cur_why = RwSignal::new(String::new());
    let cur_cover = RwSignal::new(String::new());

    let start_edit_current = move |b: BookClubBook| {
        cur_title.set(b.title);
        cur_author.set(b.author);
        cur_published.set(b.published.unwrap_or_default());
        cur_why.set(b.why.unwrap_or_default());
        cur_cover.set(b.cover_url.unwrap_or_default());
        editing_current.set(true);
    };

    let save_current = move |_| {
        let Some(user) = auth.get() else { return };
        set_err.set(String::new());
        let req = shared::UpdateBookClubBookRequest {
            title: Some(cur_title.get()),
            author: Some(cur_author.get()),
            published: Some(cur_published.get()),
            why: Some(cur_why.get()),
            cover_url: Some(cur_cover.get()),
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::update_book_club_current(req, &user.token).await {
                Ok(_) => {
                    editing_current.set(false);
                    set_note.set("Current book updated.".into());
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_err.set(e),
            }
        });
    };

    let finish_book = move |_| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            match api::finish_book_club_book(&user.token).await {
                Ok(_) => {
                    set_note.set("Moved on to the next book.".into());
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_err.set(e),
            }
        });
    };

    // ---- Add-to-queue form ----
    let queue_open = RwSignal::new(false);
    let q_title = RwSignal::new(String::new());
    let q_author = RwSignal::new(String::new());
    let q_published = RwSignal::new(String::new());
    let q_why = RwSignal::new(String::new());
    let q_cover = RwSignal::new(String::new());
    let q_busy = RwSignal::new(false);

    let reset_queue_form = move || {
        q_title.set(String::new());
        q_author.set(String::new());
        q_published.set(String::new());
        q_why.set(String::new());
        q_cover.set(String::new());
    };

    let on_cover_change = move |ev: leptos::ev::Event| {
        let Some(user) = auth.get() else { return };
        let Some(input) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) else { return };
        let Some(file) = input.files().and_then(|f| f.get(0)) else { return };
        let content_type = file.type_();
        set_note.set("Uploading cover…".into());

        let reader = web_sys::FileReader::new().unwrap();
        let reader_c = reader.clone();
        let onload = Closure::<dyn FnMut()>::new(move || {
            let Ok(value) = reader_c.result() else { return };
            let Some(data_url) = value.as_string() else { return };
            let Some((_, b64)) = data_url.split_once(',') else { return };
            let b64 = b64.to_string();
            let ct = content_type.clone();
            let token = user.token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::upload_book_club_cover(&ct, &b64, &token).await {
                    Ok(r) => {
                        q_cover.set(r.url);
                        set_note.set("Cover uploaded.".into());
                    }
                    Err(e) => set_err.set(format!("Upload failed: {e}")),
                }
            });
        });
        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();
        let _ = reader.read_as_data_url(&file);
    };

    let submit_queue = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(user) = auth.get() else { return };
        set_err.set(String::new());

        if let Err(e) = shared::validate_book_club_book(&q_title.get(), &q_author.get()) {
            set_err.set(e);
            return;
        }

        q_busy.set(true);
        let req = CreateBookClubBookRequest {
            title: q_title.get(),
            author: q_author.get(),
            published: Some(q_published.get()).filter(|s| !s.is_empty()),
            why: Some(q_why.get()).filter(|s| !s.is_empty()),
            cover_url: Some(q_cover.get()).filter(|s| !s.is_empty()),
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::add_book_club_queue_item(req, &user.token).await {
                Ok(_) => {
                    reset_queue_form();
                    set_note.set("Added to the queue.".into());
                    queue_open.set(false);
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_err.set(e),
            }
            q_busy.set(false);
        });
    };

    let move_item = move |id: String, direction: MoveDirection| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if api::move_book_club_queue_item(&id, direction, &user.token).await.is_ok() {
                set_refresh.update(|n| *n += 1);
            }
        });
    };

    let remove_item = move |id: String| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if api::remove_book_club_queue_item(&id, &user.token).await.is_ok() {
                set_refresh.update(|n| *n += 1);
            }
        });
    };

    // ---- Next meetup form ----
    let meetup_starts_at = RwSignal::new(String::new());
    let meetup_host = RwSignal::new(String::new());
    let meetup_address = RwSignal::new(String::new());
    let meetup_lat: RwSignal<Option<f64>> = RwSignal::new(None);
    let meetup_lng: RwSignal<Option<f64>> = RwSignal::new(None);
    let meetup_loaded = StoredValue::new(false);

    Effect::new(move |_| {
        if meetup_loaded.get_value() {
            return;
        }
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(t) = token else { return };
            if let Ok(Some(m)) = api::fetch_book_club_meetup(&t).await {
                meetup_starts_at.set(m.starts_at);
                meetup_host.set(m.host);
                meetup_address.set(m.address.unwrap_or_default());
                meetup_lat.set(m.lat);
                meetup_lng.set(m.lng);
                meetup_loaded.set_value(true);
            }
        });
    });

    let map_ref: NodeRef<Div> = NodeRef::new();
    let map_inited = StoredValue::new(false);
    Effect::new(move |_| {
        if map_ref.get().is_none() || map_inited.get_value() {
            return;
        }
        map_inited.set_value(true);
        let on_pick = Closure::<dyn FnMut(f64, f64)>::new(move |la: f64, ln: f64| {
            meetup_lat.set(Some(la));
            meetup_lng.set(Some(ln));
        });
        match (meetup_lat.get_untracked(), meetup_lng.get_untracked()) {
            (Some(la), Some(ln)) => map::init(MAP_ID, la, ln, true, &on_pick),
            _ => map::init(MAP_ID, MAP_CENTER.0, MAP_CENTER.1, false, &on_pick),
        }
        on_pick.forget();
    });

    let clear_meetup_pin = move |_| {
        map::clear(MAP_ID);
        meetup_lat.set(None);
        meetup_lng.set(None);
    };

    let find_meetup_on_map = move |_| {
        let Some(user) = auth.get() else { return };
        let q = meetup_address.get();
        if q.trim().is_empty() {
            set_note.set("Type an address first.".into());
            return;
        }
        set_note.set("Looking up that address…".into());
        wasm_bindgen_futures::spawn_local(async move {
            match api::geocode_book_club_address(&q, &user.token).await {
                Ok(r) if r.found => {
                    if let (Some(la), Some(ln)) = (r.lat, r.lng) {
                        meetup_lat.set(Some(la));
                        meetup_lng.set(Some(ln));
                        map::init(MAP_ID, la, ln, true, &Closure::<dyn FnMut(f64, f64)>::new(move |a: f64, b: f64| {
                            meetup_lat.set(Some(a));
                            meetup_lng.set(Some(b));
                        }));
                        set_note.set("Pin placed — tap the map to adjust.".into());
                    }
                }
                Ok(_) => set_note.set("Couldn't find that address. Drop the pin on the map instead.".into()),
                Err(e) => set_note.set(format!("Lookup failed: {e}")),
            }
        });
    };

    let save_meetup = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(user) = auth.get() else { return };
        set_err.set(String::new());

        let addr = meetup_address.get();
        if let Err(e) = shared::validate_book_club_meetup(
            &meetup_starts_at.get(),
            &meetup_host.get(),
            Some(addr.as_str()).filter(|a| !a.trim().is_empty()),
            meetup_lat.get(),
            meetup_lng.get(),
        ) {
            set_err.set(e);
            return;
        }

        let req = shared::UpdateBookClubMeetupRequest {
            starts_at: meetup_starts_at.get(),
            host: meetup_host.get(),
            address: Some(addr).filter(|a| !a.trim().is_empty()),
            lat: meetup_lat.get(),
            lng: meetup_lng.get(),
            clear_pin: meetup_lat.get().is_none() || meetup_lng.get().is_none(),
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::update_book_club_meetup(req, &user.token).await {
                Ok(_) => set_note.set("Meetup saved.".into()),
                Err(e) => set_err.set(e),
            }
        });
    };

    view! {
        <main class="page">
            <AdminNav active="book-club" is_superadmin=is_superadmin() />
            <h1 class="page-title">"Book Club"</h1>

            <Show when=move || !note.get().is_empty()>
                <p class="success">{move || note.get()}</p>
            </Show>
            <Show when=move || !err.get().is_empty()>
                <p class="error">{move || err.get()}</p>
            </Show>

            // ---- Current book ----
            <Card>
                <p class="kicker">"Current Book"</p>
                {move || match current.get() {
                    None => view! { <p>"Loading…"</p> }.into_any(),
                    Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                    Some(Ok(None)) => view! {
                        <div>
                            <p class="mn-empty">"Between books — nothing current."</p>
                            <div class="admin-actions">
                                <Button appearance=ButtonAppearance::Primary on_click=finish_book>
                                    "Finish & start next"
                                </Button>
                            </div>
                            <p class="field-hint">
                                "Promotes the front of the queue to current. Members are notified."
                            </p>
                        </div>
                    }.into_any(),
                    Some(Ok(Some(b))) => {
                        let b2 = b.clone();
                        view! {
                            <div style:display=move || if editing_current.get() { "none" } else { "block" }>
                                <h3 class="mn-title">{b.title.clone()}</h3>
                                <p class="mn-date">{b.author.clone()}</p>
                                <div class="admin-actions">
                                    <Button on_click={
                                        let b = b2.clone();
                                        move |_| start_edit_current(b.clone())
                                    }>"Edit"</Button>
                                    <Button appearance=ButtonAppearance::Primary on_click=finish_book>
                                        "Finish & start next"
                                    </Button>
                                </div>
                                <p class="field-hint">
                                    "Archives this book and promotes the front of the queue. Members are notified."
                                </p>
                            </div>
                        }.into_any()
                    }
                }}

                <div style:display=move || if editing_current.get() { "block" } else { "none" }>
                    <Field label="Title"><Input value=cur_title placeholder="The Bloody Chamber" /></Field>
                    <Field label="Author"><Input value=cur_author placeholder="Angela Carter" /></Field>
                    <Field label="Published"><Input value=cur_published placeholder="1979" /></Field>
                    <Field label="Why we picked it"><Textarea value=cur_why /></Field>
                    <Field label="Cover URL"><Input value=cur_cover placeholder="https://…" /></Field>
                    <div class="admin-actions">
                        <Button appearance=ButtonAppearance::Primary on_click=save_current>"Save Changes"</Button>
                        <Button on_click=move |_| editing_current.set(false)>"Cancel"</Button>
                    </div>
                </div>
            </Card>

            // ---- Queue (admin-only) ----
            <Card>
                <div class="admin-row">
                    <p class="kicker">"Queue — admins only, members never see this"</p>
                    <Button on_click=move |_| {
                        if queue_open.get() { reset_queue_form(); }
                        queue_open.update(|o| *o = !*o);
                    }>
                        {move || if queue_open.get() { "Close" } else { "+ Add to queue" }}
                    </Button>
                </div>

                {move || match queue.get() {
                    None => view! { <p>"Loading…"</p> }.into_any(),
                    Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                    Some(Ok(list)) if list.is_empty() => view! { <p class="mn-empty">"Nothing queued."</p> }.into_any(),
                    Some(Ok(list)) => {
                        let count = list.len();
                        view! {
                            <div>
                                {list.into_iter().enumerate().map(|(i, b)| {
                                    let id_up = b.id.clone();
                                    let id_down = b.id.clone();
                                    let id_del = b.id.clone();
                                    view! {
                                        <div class="mn-row" style="align-items:center;padding:0.5rem 0;border-bottom:1px solid rgba(243,235,227,0.08);">
                                            <div class="mn-body">
                                                <span class="mn-title" style="font-size:1.1rem;">{b.title.clone()}</span>
                                                " "
                                                <span class="mn-date">{b.author.clone()}</span>
                                            </div>
                                            <div class="admin-actions">
                                                <Button
                                                    button_type=ButtonType::Button
                                                    disabled=Signal::derive(move || i == 0)
                                                    on_click=move |_| move_item(id_up.clone(), MoveDirection::Up)
                                                >"↑"</Button>
                                                <Button
                                                    button_type=ButtonType::Button
                                                    disabled=Signal::derive(move || i + 1 == count)
                                                    on_click=move |_| move_item(id_down.clone(), MoveDirection::Down)
                                                >"↓"</Button>
                                                <Button button_type=ButtonType::Button on_click=move |_| remove_item(id_del.clone())>"×"</Button>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_any()
                    }
                }}

                <div style:display=move || if queue_open.get() { "block" } else { "none" }>
                    <form on:submit=submit_queue>
                        <Field label="Title"><Input value=q_title placeholder="Wide Sargasso Sea" /></Field>
                        <Field label="Author"><Input value=q_author placeholder="Jean Rhys" /></Field>
                        <Field label="Published"><Input value=q_published placeholder="1966" /></Field>
                        <Field label="Why we picked it (optional)"><Textarea value=q_why /></Field>
                        <Field label="Cover image (optional)">
                            <input type="file" accept="image/*" on:change=on_cover_change />
                        </Field>
                        {move || (!q_cover.get().is_empty()).then(|| view! {
                            <img src={q_cover.get()} alt="cover preview" class="gathering-cover-preview" />
                        })}
                        <div class="admin-actions">
                            <Button
                                button_type=ButtonType::Submit
                                appearance=ButtonAppearance::Primary
                                disabled=Signal::derive(move || q_busy.get())
                            >
                                {move || if q_busy.get() { "Adding…" } else { "Add to queue" }}
                            </Button>
                        </div>
                    </form>
                </div>
            </Card>

            // ---- Next meetup ----
            <Card>
                <p class="kicker">"Next Meetup"</p>
                <form on:submit=save_meetup>
                    <Field label="Date & time"><Input value=meetup_starts_at input_type=InputType::DatetimeLocal /></Field>
                    <Field label="Host"><Input value=meetup_host placeholder="Marisol V." /></Field>
                    <Field label="Address"><Input value=meetup_address placeholder="1412 SE C St, Bentonville" /></Field>
                    <div class="admin-actions">
                        <Button button_type=ButtonType::Button on_click=find_meetup_on_map>"Find on map"</Button>
                        <Button button_type=ButtonType::Button on_click=clear_meetup_pin>"Clear pin"</Button>
                    </div>
                    <div node_ref=map_ref id=MAP_ID class="ride-map"></div>
                    {move || match (meetup_lat.get(), meetup_lng.get()) {
                        (Some(la), Some(ln)) => view! { <p class="field-hint">{format!("Pin: {la:.5}, {ln:.5}")}</p> }.into_any(),
                        _ => view! { <p class="field-hint">"No pin dropped."</p> }.into_any(),
                    }}
                    <div class="admin-actions">
                        <Button button_type=ButtonType::Submit appearance=ButtonAppearance::Primary>"Save meetup"</Button>
                    </div>
                </form>
            </Card>
        </main>
    }
}
