use auth_client::AuthUser;
use crate::api;
use crate::components::admin_nav::AdminNav;
use leptos::prelude::*;
use shared::{CreateEventRequest, EventStage, Rsvp, UpdateEventRequest};
use thaw::{
    Button, ButtonAppearance, ButtonType, Card, Field, Input, InputType, Select, Textarea,
};

/// "2030-10-31" -> "2030-10-30", via the JS Date object so month/year rollover
/// is handled for us. Used to prefill the RSVP deadline to the day before the
/// screening. Falls back to the input on anything unparseable.
fn day_before(date: &str) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(&format!("{date}T12:00:00")));
    if d.get_time().is_nan() {
        return date.to_string();
    }
    d.set_date(d.get_date() - 1);
    format!(
        "{:04}-{:02}-{:02}",
        d.get_full_year() as i64,
        d.get_month() as i64 + 1, // JS months are 0-based
        d.get_date() as i64,
    )
}

/// Which inline lifecycle form is open on an event card.
#[derive(Clone, Copy, PartialEq)]
enum StageAction {
    AddPoll,
    SetDate,
    SetDeadline,
}

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
    // The create form posts a movie; the date normally comes later, from the
    // poll. This discloses date + deadline for events that skip voting.
    let show_schedule = RwSignal::new(false);

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
        let scheduled = show_schedule.get();
        let req = CreateEventRequest {
            event_type: event_type.get(),
            title: title.get(),
            date: Some(date.get()).filter(|d| scheduled && !d.is_empty()),
            description: if description.get().is_empty() { None } else { Some(description.get()) },
            poll_embed_url: if poll_url.get().is_empty() { None } else { Some(poll_url.get()) },
            poster_url: if poster_url.get().is_empty() { None } else { Some(poster_url.get()) },
            rsvp_deadline: Some(rsvp_deadline.get()).filter(|d| scheduled && !d.is_empty()),
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::create_event(req, &user.token).await {
                Ok(_) => {
                    set_form_success.set("Event created!".into());
                    title.set(String::new());
                    date.set(String::new());
                    rsvp_deadline.set(String::new());
                    description.set(String::new());
                    poll_url.set(String::new());
                    poster_url.set(String::new());
                    show_schedule.set(false);
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

    // --- Inline lifecycle actions (the stepper's "next step" forms) ---
    let stage_open: RwSignal<Option<(String, StageAction)>> = RwSignal::new(None);
    let stage_poll = RwSignal::new(String::new());
    let stage_date = RwSignal::new(String::new());
    let stage_deadline = RwSignal::new(String::new());
    let (stage_error, set_stage_error) = signal(String::new());

    let open_stage = move |id: String, action: StageAction, prefill_deadline: String| {
        stage_poll.set(String::new());
        stage_date.set(String::new());
        stage_deadline.set(prefill_deadline);
        set_stage_error.set(String::new());
        stage_open.set(Some((id, action)));
    };

    // Setting the screening date suggests a deadline of the day before; the
    // admin can still override it after picking the date.
    Effect::new(move |_| {
        let d = stage_date.get();
        if !d.is_empty() {
            stage_deadline.set(day_before(&d));
        }
    });

    let handle_stage_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some((id, action)) = stage_open.get() else { return };
        let Some(user) = auth.get() else { return };
        set_stage_error.set(String::new());
        let req = match action {
            StageAction::AddPoll => {
                let url = stage_poll.get();
                if url.is_empty() {
                    set_stage_error.set("Paste the poll embed URL first.".into());
                    return;
                }
                UpdateEventRequest { poll_embed_url: Some(url), ..Default::default() }
            }
            StageAction::SetDate => {
                let d = stage_date.get();
                if d.is_empty() {
                    set_stage_error.set("Pick the screening date first.".into());
                    return;
                }
                UpdateEventRequest {
                    date: Some(d),
                    rsvp_deadline: Some(stage_deadline.get()), // "" clears = none
                    ..Default::default()
                }
            }
            StageAction::SetDeadline => {
                let d = stage_deadline.get();
                if d.is_empty() {
                    set_stage_error.set("Pick the deadline first.".into());
                    return;
                }
                UpdateEventRequest { rsvp_deadline: Some(d), ..Default::default() }
            }
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::update_event(&id, req, &user.token).await {
                Ok(_) => {
                    stage_open.set(None);
                    set_event_refresh.update(|n| *n += 1);
                }
                Err(e) => set_stage_error.set(e),
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
                    <h2>"Post a Movie"</h2>
                    <p class="stage-hint" style="margin-top:0;">
                        "Post the pick first — the date usually comes later, once the poll closes."
                    </p>
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
                        <Field label="Description (optional)">
                            <Textarea value=description placeholder="A few words about the pick…" />
                        </Field>
                        <Field label="Poster image URL (optional)">
                            <Input value=poster_url input_type=InputType::Url placeholder="https://..." />
                        </Field>
                        <Field label="rcv123 poll embed URL (optional — src from their iframe code)">
                            <Input value=poll_url input_type=InputType::Url placeholder="https://rcv123.org/poll/..." />
                        </Field>

                        <button
                            type="button"
                            class="stage-disclosure"
                            on:click=move |_| show_schedule.update(|s| *s = !*s)
                        >
                            {move || if show_schedule.get() {
                                "▾ Already scheduled?"
                            } else {
                                "▸ Already scheduled? Set the date now"
                            }}
                        </button>
                        <Show when=move || show_schedule.get()>
                            <Field label="Date">
                                <Input value=date input_type=InputType::Date />
                            </Field>
                            <Field label="RSVP deadline (optional)">
                                <Input value=rsvp_deadline input_type=InputType::Date />
                            </Field>
                        </Show>

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
                                                let stage = e.stage();
                                                let has_poll = e.poll_embed_url.is_some();
                                                let has_deadline = e.rsvp_deadline.is_some();
                                                let step = |done: bool, label: &str| format!(
                                                    "{label} {}", if done { "✓" } else { "—" }
                                                );
                                                let steps = [
                                                    step(true, "posted"),
                                                    step(has_poll, "poll"),
                                                    step(stage == EventStage::Scheduled, "date"),
                                                    step(has_deadline, "rsvp"),
                                                ].join(" · ");
                                                let hint = match stage {
                                                    EventStage::Posted => Some("No date yet — add the poll, or set the date to skip voting."),
                                                    EventStage::Voting => Some("Voting open — set the winning date to close the poll."),
                                                    EventStage::Scheduled if !has_deadline => Some("Scheduled. RSVPs stay open — set a deadline to close them."),
                                                    EventStage::Scheduled => None,
                                                };
                                                // The one primary button per card: the next lifecycle step.
                                                let next_action = match stage {
                                                    EventStage::Posted => Some((StageAction::SetDate, "Set Date")),
                                                    EventStage::Voting => Some((StageAction::SetDate, "Close Poll & Set Date")),
                                                    EventStage::Scheduled if !has_deadline => Some((StageAction::SetDeadline, "Set RSVP Deadline")),
                                                    EventStage::Scheduled => None,
                                                };
                                                let deadline_prefill = e.date.clone()
                                                    .map(|d| day_before(&d))
                                                    .unwrap_or_default();
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
                                                            <span class="stage-steps">{steps}</span>
                                                            {hint.map(|h| view! { <span class="stage-hint">{h}</span> })}
                                                            <span style="display:block;margin-top:0.25rem;color:#93d8b4;font-size:0.85rem;">
                                                                {format!("{count} going")}
                                                            </span>
                                                            {e2.poster_url.clone().map(|url| view! {
                                                                <img src={url} alt="poster"
                                                                    style="width:48px;height:72px;object-fit:cover;border-radius:2px;margin-top:0.4rem;display:block;" />
                                                            })}
                                                        </div>
                                                        <div class="admin-actions">
                                                            {(stage == EventStage::Posted).then(|| {
                                                                let poll_id = id.clone();
                                                                view! {
                                                                    <Button
                                                                        appearance=ButtonAppearance::Primary
                                                                        on_click=move |_| open_stage(poll_id.clone(), StageAction::AddPoll, String::new())
                                                                    >"Add Poll"</Button>
                                                                }
                                                            })}
                                                            {next_action.map(|(action, label)| {
                                                                let act_id = id.clone();
                                                                let prefill = deadline_prefill.clone();
                                                                // "Set Date" is secondary while a poll is the primary
                                                                // path; once voting it becomes the primary close action.
                                                                let appearance = if stage == EventStage::Posted {
                                                                    ButtonAppearance::Secondary
                                                                } else {
                                                                    ButtonAppearance::Primary
                                                                };
                                                                view! {
                                                                    <Button
                                                                        appearance=appearance
                                                                        on_click=move |_| open_stage(act_id.clone(), action, prefill.clone())
                                                                    >{label}</Button>
                                                                }
                                                            })}
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
                                                        let form_id = id.clone();
                                                        move || (stage_open.get().map(|(sid, _)| sid).as_deref() == Some(form_id.as_str())).then(|| {
                                                            let action = stage_open.get().map(|(_, a)| a).unwrap_or(StageAction::SetDate);
                                                            view! {
                                                                <form class="stage-form" on:submit=handle_stage_submit>
                                                                    {matches!(action, StageAction::AddPoll).then(|| view! {
                                                                        <Field label="rcv123 poll embed URL (src from their iframe code)">
                                                                            <Input value=stage_poll input_type=InputType::Url placeholder="https://rcv123.org/poll/..." />
                                                                        </Field>
                                                                    })}
                                                                    {matches!(action, StageAction::SetDate).then(|| view! {
                                                                        <Field label="Screening date (closes the poll)">
                                                                            <Input value=stage_date input_type=InputType::Date />
                                                                        </Field>
                                                                    })}
                                                                    {matches!(action, StageAction::SetDate | StageAction::SetDeadline).then(|| view! {
                                                                        <Field label="RSVP deadline (suggested: day before — clear it to leave RSVPs open)">
                                                                            <Input value=stage_deadline input_type=InputType::Date />
                                                                        </Field>
                                                                    })}
                                                                    <Show when=move || !stage_error.get().is_empty()>
                                                                        <p class="error">{move || stage_error.get()}</p>
                                                                    </Show>
                                                                    <div class="admin-actions">
                                                                        <Button button_type=ButtonType::Submit appearance=ButtonAppearance::Primary>"Save"</Button>
                                                                        <Button
                                                                            button_type=ButtonType::Button
                                                                            appearance=ButtonAppearance::Secondary
                                                                            on_click=move |_| stage_open.set(None)
                                                                        >"Cancel"</Button>
                                                                    </div>
                                                                </form>
                                                            }
                                                        })
                                                    }
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
