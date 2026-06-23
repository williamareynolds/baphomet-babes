use auth_client::AuthUser;
use crate::api;
use crate::components::admin_nav::AdminNav;
use leptos::prelude::*;
use shared::{CreateEventRequest, Rsvp, UpdateEventRequest};
use thaw::{
    Button, ButtonAppearance, ButtonType, Card, Field, Input, InputType, Select, Textarea,
};

#[component]
pub fn AdminEventsPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let is_admin = move || auth.get().map(|u| u.is_admin()).unwrap_or(false);
    let is_superadmin = move || auth.get().map(|u| u.is_superadmin()).unwrap_or(false);

    // --- Events ---
    let (event_refresh, set_event_refresh) = signal(0u32);
    let events: RwSignal<Option<Result<Vec<shared::Event>, String>>> = RwSignal::new(None);

    Effect::new(move |_| {
        let _ = event_refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let result = match token {
                None => return,
                Some(t) => api::fetch_events(&t).await,
            };
            events.set(Some(result));
        });
    });

    let title = RwSignal::new(String::new());
    let date = RwSignal::new(String::new());
    let rsvp_deadline = RwSignal::new(String::new());
    let event_type = RwSignal::new("main".to_string());
    let description = RwSignal::new(String::new());
    let poll_url = RwSignal::new(String::new());
    let poster_url = RwSignal::new(String::new());

    // RSVP viewer: which event's attendee list is open, and the loaded names.
    let open_rsvps: RwSignal<Option<String>> = RwSignal::new(None);
    let rsvp_list: RwSignal<Option<Result<Vec<Rsvp>, String>>> = RwSignal::new(None);
    let view_rsvps = move |id: String| {
        let Some(user) = auth.get() else { return };
        // Toggle closed if it's already the open one.
        if open_rsvps.get() == Some(id.clone()) {
            open_rsvps.set(None);
            return;
        }
        open_rsvps.set(Some(id.clone()));
        rsvp_list.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            rsvp_list.set(Some(api::fetch_rsvps(&id, &user.token).await));
        });
    };
    let (form_error, set_form_error) = signal(String::new());
    let (form_success, set_form_success) = signal(String::new());

    let handle_create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_form_error.set(String::new());
        set_form_success.set(String::new());
        let Some(user) = auth.get() else { return };
        let req = CreateEventRequest {
            event_type: event_type.get(),
            title: title.get(),
            date: if date.get().is_empty() { None } else { Some(date.get()) },
            description: if description.get().is_empty() { None } else { Some(description.get()) },
            poll_embed_url: if poll_url.get().is_empty() { None } else { Some(poll_url.get()) },
            poster_url: if poster_url.get().is_empty() { None } else { Some(poster_url.get()) },
            rsvp_deadline: if rsvp_deadline.get().is_empty() { None } else { Some(rsvp_deadline.get()) },
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::create_event(req, &user.token).await {
                Ok(_) => {
                    set_form_success.set("Event created!".into());
                    rsvp_deadline.set(String::new());
                    set_event_refresh.update(|n| *n += 1);
                }
                Err(e) => set_form_error.set(e),
            }
        });
    };

    let handle_delete_event = move |id: String| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if api::delete_event(&id, &user.token).await.is_ok() {
                set_event_refresh.update(|n| *n += 1);
            }
        });
    };

    let editing_id: RwSignal<Option<String>> = RwSignal::new(None);
    let edit_title = RwSignal::new(String::new());
    let edit_date = RwSignal::new(String::new());
    let edit_rsvp_deadline = RwSignal::new(String::new());
    let edit_type = RwSignal::new(String::new());
    let edit_description = RwSignal::new(String::new());
    let edit_poll_url = RwSignal::new(String::new());
    let edit_poster_url = RwSignal::new(String::new());
    let (edit_error, set_edit_error) = signal(String::new());

    let handle_edit_start = move |e: shared::Event| {
        edit_title.set(e.title.clone());
        edit_date.set(e.date.clone().unwrap_or_default());
        edit_rsvp_deadline.set(e.rsvp_deadline.clone().unwrap_or_default());
        edit_type.set(e.event_type.clone());
        edit_description.set(e.description.clone().unwrap_or_default());
        edit_poll_url.set(e.poll_embed_url.clone().unwrap_or_default());
        edit_poster_url.set(e.poster_url.clone().unwrap_or_default());
        set_edit_error.set(String::new());
        editing_id.set(Some(e.id));
    };

    let handle_edit_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(id) = editing_id.get() else { return };
        let Some(user) = auth.get() else { return };
        set_edit_error.set(String::new());
        let req = UpdateEventRequest {
            event_type: Some(edit_type.get()),
            title: Some(edit_title.get()),
            date: Some(edit_date.get()),
            rsvp_deadline: Some(edit_rsvp_deadline.get()),
            description: if edit_description.get().is_empty() { None } else { Some(edit_description.get()) },
            poll_embed_url: if edit_poll_url.get().is_empty() { None } else { Some(edit_poll_url.get()) },
            poster_url: if edit_poster_url.get().is_empty() { None } else { Some(edit_poster_url.get()) },
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::update_event(&id, req, &user.token).await {
                Ok(_) => {
                    editing_id.set(None);
                    set_event_refresh.update(|n| *n += 1);
                }
                Err(e) => set_edit_error.set(e),
            }
        });
    };

    view! {
        <main>
            <Show
                when=is_admin
                fallback=|| view! { <p class="error">"Access denied."</p> }
            >
                <h1>"Admin"</h1>
                <AdminNav active="events" is_superadmin=is_superadmin() />

                <Card>
                    <h2>"Create Event"</h2>
                    <form on:submit=handle_create>
                        <Field label="Type">
                            <Select value=event_type>
                                <option value="main">"Featured Film"</option>
                                <option value="special">"Special Feature"</option>
                            </Select>
                        </Field>
                        <Field label="Title">
                            <Input value=title placeholder="Movie title" />
                        </Field>
                        <Field label="Date">
                            <Input value=date input_type=InputType::Date />
                        </Field>
                        <Field label="RSVP deadline (optional)">
                            <Input value=rsvp_deadline input_type=InputType::Date />
                        </Field>
                        <Field label="Description (optional)">
                            <Textarea value=description placeholder="A few words about the pick…" />
                        </Field>
                        <Field label="rcv123 poll embed URL (optional — src from their iframe code)">
                            <Input value=poll_url input_type=InputType::Url placeholder="https://rcv123.org/poll/..." />
                        </Field>
                        <Field label="Poster image URL (optional)">
                            <Input value=poster_url input_type=InputType::Url placeholder="https://..." />
                        </Field>
                        <Show when=move || !form_error.get().is_empty()>
                            <p class="error">{move || form_error.get()}</p>
                        </Show>
                        <Show when=move || !form_success.get().is_empty()>
                            <p class="success">{move || form_success.get()}</p>
                        </Show>
                        <Button button_type=ButtonType::Submit appearance=ButtonAppearance::Primary>
                            "Create Event"
                        </Button>
                    </form>
                </Card>

                <h2 class="section-heading">"All Events"</h2>
                {move || match events.get() {
                    None => view! { <p>"Loading..."</p> }.into_any(),
                    Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                    Some(Ok(list)) => view! {
                        <div>
                            {list.into_iter().map(|e| {
                                let id = e.id.clone();
                                // The card body is a reactive closure so toggling
                                // `editing_id` swaps display↔form in place. (Without
                                // this it sits inside the events-only closure and
                                // never re-renders — the Edit button looked dead.)
                                view! {
                                    <Card>
                                        {move || {
                                            let editing = editing_id.get().as_deref() == Some(id.as_str());
                                            if editing {
                                                view! {
                                                    <form on:submit=handle_edit_submit>
                                                        <Field label="Type">
                                                            <Select value=edit_type>
                                                                <option value="main">"Featured Film"</option>
                                                                <option value="special">"Special Feature"</option>
                                                            </Select>
                                                        </Field>
                                                        <Field label="Title">
                                                            <Input id="edit-title" value=edit_title />
                                                        </Field>
                                                        <Field label="Date">
                                                            <Input value=edit_date input_type=InputType::Date />
                                                        </Field>
                                                        <Field label="RSVP deadline (optional)">
                                                            <Input value=edit_rsvp_deadline input_type=InputType::Date />
                                                        </Field>
                                                        <Field label="Description (optional)">
                                                            <Textarea value=edit_description />
                                                        </Field>
                                                        <Field label="Poll embed URL (optional)">
                                                            <Input value=edit_poll_url input_type=InputType::Url placeholder="https://rcv123.org/poll/..." />
                                                        </Field>
                                                        <Field label="Poster image URL (optional)">
                                                            <Input value=edit_poster_url input_type=InputType::Url placeholder="https://..." />
                                                        </Field>
                                                        <Show when=move || !edit_error.get().is_empty()>
                                                            <p class="error">{move || edit_error.get()}</p>
                                                        </Show>
                                                        <div class="admin-actions">
                                                            <Button button_type=ButtonType::Submit appearance=ButtonAppearance::Primary>"Save"</Button>
                                                            <Button
                                                                button_type=ButtonType::Button
                                                                appearance=ButtonAppearance::Secondary
                                                                on_click=move |_| editing_id.set(None)
                                                            >"Cancel"</Button>
                                                        </div>
                                                    </form>
                                                }.into_any()
                                            } else {
                                                let e2 = e.clone();
                                                let id2 = id.clone();
                                                let rsvp_id = id.clone();
                                                let this_id = id.clone();
                                                let count = e.rsvp_count;
                                                let deadline = e.rsvp_deadline.clone();
                                                view! {
                                                    <div class="admin-row">
                                                        <div>
                                                            <span class={format!("badge badge-{}", e2.event_type)}>
                                                                {e2.event_type.clone()}
                                                            </span>
                                                            <strong style="display:block;margin-top:0.25rem;">{e2.title.clone()}</strong>
                                                            <small style="color:#bdafb2;">{e2.date.clone().unwrap_or_else(|| "No date set".into())}</small>
                                                            {deadline.clone().map(|d| view! {
                                                                <small style="display:block;color:#bdafb2;">{format!("RSVP by {d}")}</small>
                                                            })}
                                                            <span style="display:block;margin-top:0.25rem;color:#93d8b4;font-size:0.85rem;">
                                                                {format!("{count} going")}
                                                            </span>
                                                            {e2.poll_embed_url.clone().map(|_| view! {
                                                                <span class="poll-set">"✓ poll set"</span>
                                                            })}
                                                            {e2.poster_url.clone().map(|url| view! {
                                                                <img src={url} alt="poster"
                                                                    style="width:48px;height:72px;object-fit:cover;border-radius:2px;margin-top:0.4rem;display:block;" />
                                                            })}
                                                        </div>
                                                        <div class="admin-actions">
                                                            <Button
                                                                appearance=ButtonAppearance::Secondary
                                                                on_click=move |_| view_rsvps(rsvp_id.clone())
                                                            >{move || if open_rsvps.get() == Some(this_id.clone()) { "Hide RSVPs" } else { "View RSVPs" }}</Button>
                                                            <Button
                                                                appearance=ButtonAppearance::Secondary
                                                                on_click=move |_| handle_edit_start(e2.clone())
                                                            >"Edit"</Button>
                                                            <Button
                                                                appearance=ButtonAppearance::Secondary
                                                                on_click=move |_| handle_delete_event(id2.clone())
                                                            >"Delete"</Button>
                                                        </div>
                                                    </div>
                                                    {
                                                        let row_id = id.clone();
                                                        move || (open_rsvps.get() == Some(row_id.clone())).then(|| match rsvp_list.get() {
                                                            None => view! { <p class="rsvp-names">"Loading…"</p> }.into_any(),
                                                            Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                                                            Some(Ok(list)) if list.is_empty() =>
                                                                view! { <p class="rsvp-names">"No RSVPs yet."</p> }.into_any(),
                                                            Some(Ok(list)) => view! {
                                                                <ul class="rsvp-names">
                                                                    {list.into_iter().map(|r| view! {
                                                                        <li>{r.author}</li>
                                                                    }).collect::<Vec<_>>()}
                                                                </ul>
                                                            }.into_any(),
                                                        })
                                                    }
                                                }.into_any()
                                            }
                                        }}
                                    </Card>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any(),
                }}
            </Show>
        </main>
    }
}
