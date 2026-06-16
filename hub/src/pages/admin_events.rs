use auth_client::AuthUser;
use crate::api;
use crate::components::admin_nav::AdminNav;
use leptos::prelude::*;
use shared::{CreateEventRequest, UpdateEventRequest};
use thaw::{
    Button, ButtonAppearance, ButtonType, Card, Field, Input, InputType, Select, Textarea,
};

#[component]
pub fn AdminEventsPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let is_admin = move || auth.get().map(|u| u.is_admin()).unwrap_or(false);

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
    let event_type = RwSignal::new("main".to_string());
    let description = RwSignal::new(String::new());
    let poll_url = RwSignal::new(String::new());
    let poster_url = RwSignal::new(String::new());
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
            date: date.get(),
            description: if description.get().is_empty() { None } else { Some(description.get()) },
            poll_embed_url: if poll_url.get().is_empty() { None } else { Some(poll_url.get()) },
            poster_url: if poster_url.get().is_empty() { None } else { Some(poster_url.get()) },
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::create_event(req, &user.token).await {
                Ok(_) => {
                    set_form_success.set("Event created!".into());
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
    let edit_type = RwSignal::new(String::new());
    let edit_description = RwSignal::new(String::new());
    let edit_poll_url = RwSignal::new(String::new());
    let edit_poster_url = RwSignal::new(String::new());
    let (edit_error, set_edit_error) = signal(String::new());

    let handle_edit_start = move |e: shared::Event| {
        edit_title.set(e.title.clone());
        edit_date.set(e.date.clone());
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
                <AdminNav active="events" />

                <Card>
                    <h2>"Create Event"</h2>
                    <form on:submit=handle_create>
                        <Field label="Type">
                            <Select value=event_type>
                                <option value="main">"Main Event"</option>
                                <option value="special">"Special Feature"</option>
                            </Select>
                        </Field>
                        <Field label="Title">
                            <Input value=title placeholder="Movie title" />
                        </Field>
                        <Field label="Date">
                            <Input value=date input_type=InputType::Date />
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
                                let is_editing = {
                                    let id = id.clone();
                                    move || editing_id.get().as_deref() == Some(&id)
                                };
                                view! {
                                    <Card>
                                        {(!is_editing()).then(|| {
                                            let e2 = e.clone();
                                            let id2 = id.clone();
                                            view! {
                                                <div class="admin-row">
                                                    <div>
                                                        <span class={format!("badge badge-{}", e2.event_type)}>
                                                            {e2.event_type.clone()}
                                                        </span>
                                                        <strong style="display:block;margin-top:0.25rem;">{e2.title.clone()}</strong>
                                                        <small style="color:#8a7a7a;">{e2.date.clone()}</small>
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
                                                            on_click=move |_| handle_edit_start(e2.clone())
                                                        >"Edit"</Button>
                                                        <Button
                                                            appearance=ButtonAppearance::Secondary
                                                            on_click=move |_| handle_delete_event(id2.clone())
                                                        >"Delete"</Button>
                                                    </div>
                                                </div>
                                            }
                                        })}
                                        {is_editing().then(|| view! {
                                            <form on:submit=handle_edit_submit>
                                                <Field label="Type">
                                                    <Select value=edit_type>
                                                        <option value="main">"Main Event"</option>
                                                        <option value="special">"Special Feature"</option>
                                                    </Select>
                                                </Field>
                                                <Field label="Title">
                                                    <Input value=edit_title />
                                                </Field>
                                                <Field label="Date">
                                                    <Input value=edit_date input_type=InputType::Date />
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
                                        })}
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
