use crate::{api, context::AuthUser};
use leptos::prelude::*;
use shared::{CreateEventRequest, CreateInviteRequest, UpdateEventRequest};

#[component]
pub fn AdminPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
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

    let (title, set_title) = signal(String::new());
    let (date, set_date) = signal(String::new());
    let (event_type, set_event_type) = signal("main".to_string());
    let (description, set_description) = signal(String::new());
    let (poll_url, set_poll_url) = signal(String::new());
    let (poster_url, set_poster_url) = signal(String::new());
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
    let (edit_title, set_edit_title) = signal(String::new());
    let (edit_date, set_edit_date) = signal(String::new());
    let (edit_type, set_edit_type) = signal(String::new());
    let (edit_description, set_edit_description) = signal(String::new());
    let (edit_poll_url, set_edit_poll_url) = signal(String::new());
    let (edit_poster_url, set_edit_poster_url) = signal(String::new());
    let (edit_error, set_edit_error) = signal(String::new());

    let handle_edit_start = move |e: shared::Event| {
        set_edit_title.set(e.title.clone());
        set_edit_date.set(e.date.clone());
        set_edit_type.set(e.event_type.clone());
        set_edit_description.set(e.description.clone().unwrap_or_default());
        set_edit_poll_url.set(e.poll_embed_url.clone().unwrap_or_default());
        set_edit_poster_url.set(e.poster_url.clone().unwrap_or_default());
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

    // --- Invite codes ---
    let (invite_refresh, set_invite_refresh) = signal(0u32);
    let invites: RwSignal<Option<Result<Vec<shared::InviteCode>, String>>> = RwSignal::new(None);

    Effect::new(move |_| {
        let _ = invite_refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let result = match token {
                None => return,
                Some(t) => api::fetch_invites(&t).await,
            };
            invites.set(Some(result));
        });
    });

    let (invite_role, set_invite_role) = signal("member".to_string());
    let (invite_error, set_invite_error) = signal(String::new());
    let (invite_success, set_invite_success) = signal(String::new());

    let handle_create_invite = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_invite_error.set(String::new());
        set_invite_success.set(String::new());
        let Some(user) = auth.get() else { return };
        let req = CreateInviteRequest { role: invite_role.get() };
        wasm_bindgen_futures::spawn_local(async move {
            match api::create_invite(req, &user.token).await {
                Ok(code) => {
                    set_invite_success.set(format!("Code: {}", code.code));
                    set_invite_refresh.update(|n| *n += 1);
                }
                Err(e) => set_invite_error.set(e),
            }
        });
    };

    let handle_delete_invite = move |id: String| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if api::delete_invite(&id, &user.token).await.is_ok() {
                set_invite_refresh.update(|n| *n += 1);
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

                <div class="card">
                    <h2>"Create Event"</h2>
                    <form on:submit=handle_create>
                        <label>"Type"</label>
                        <select
                            prop:value=event_type
                            on:change=move |e| set_event_type.set(event_target_value(&e))
                        >
                            <option value="main">"Main Event"</option>
                            <option value="special">"Special Feature"</option>
                        </select>
                        <label>"Title"</label>
                        <input type="text" required
                            prop:value=title
                            on:input=move |e| set_title.set(event_target_value(&e)) />
                        <label>"Date (YYYY-MM-DD)"</label>
                        <input type="date" required
                            prop:value=date
                            on:input=move |e| set_date.set(event_target_value(&e)) />
                        <label>"Description (optional)"</label>
                        <textarea rows="3"
                            prop:value=description
                            on:input=move |e| set_description.set(event_target_value(&e)) />
                        <label>"rcv123 poll embed URL (optional — src from their iframe code)"</label>
                        <input type="url"
                            placeholder="https://rcv123.org/poll/..."
                            prop:value=poll_url
                            on:input=move |e| set_poll_url.set(event_target_value(&e)) />
                        <label>"Poster image URL (optional)"</label>
                        <input type="text"
                            placeholder="https://..."
                            prop:value=poster_url
                            on:input=move |e| set_poster_url.set(event_target_value(&e)) />
                        <Show when=move || !form_error.get().is_empty()>
                            <p class="error">{move || form_error.get()}</p>
                        </Show>
                        <Show when=move || !form_success.get().is_empty()>
                            <p class="success">{move || form_success.get()}</p>
                        </Show>
                        <button type="submit">"Create Event"</button>
                    </form>
                </div>

                <h2 style="margin-top:2rem;">"All Events"</h2>
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
                                    <div class="card">
                                        {(!is_editing()).then(|| {
                                            let e2 = e.clone();
                                            let id2 = id.clone();
                                            view! {
                                                <div style="display:flex;justify-content:space-between;align-items:flex-start;">
                                                    <div>
                                                        <span class={format!("badge badge-{}", e2.event_type)}>
                                                            {e2.event_type.clone()}
                                                        </span>
                                                        <strong style="display:block;margin-top:0.25rem;">{e2.title.clone()}</strong>
                                                        <small style="color:#aaa;">{e2.date.clone()}</small>
                                                        {e2.poll_embed_url.clone().map(|_| view! {
                                                            <span style="color:#6bffb8;font-size:0.8rem;display:block;">"✓ poll set"</span>
                                                        })}
                                                        {e2.poster_url.clone().map(|url| view! {
                                                            <img src={url} alt="poster"
                                                                style="width:48px;height:72px;object-fit:cover;border-radius:2px;margin-top:0.4rem;display:block;" />
                                                        })}
                                                    </div>
                                                    <div style="display:flex;gap:0.5rem;">
                                                        <button class="secondary"
                                                            on:click=move |_| handle_edit_start(e2.clone())
                                                        >"Edit"</button>
                                                        <button class="secondary"
                                                            on:click=move |_| handle_delete_event(id2.clone())
                                                        >"Delete"</button>
                                                    </div>
                                                </div>
                                            }
                                        })}
                                        {is_editing().then(|| view! {
                                            <form on:submit=handle_edit_submit>
                                                <label>"Type"</label>
                                                <select
                                                    prop:value=edit_type
                                                    on:change=move |ev| set_edit_type.set(event_target_value(&ev))
                                                >
                                                    <option value="main">"Main Event"</option>
                                                    <option value="special">"Special Feature"</option>
                                                </select>
                                                <label>"Title"</label>
                                                <input type="text" required
                                                    prop:value=edit_title
                                                    on:input=move |ev| set_edit_title.set(event_target_value(&ev)) />
                                                <label>"Date (YYYY-MM-DD)"</label>
                                                <input type="date" required
                                                    prop:value=edit_date
                                                    on:input=move |ev| set_edit_date.set(event_target_value(&ev)) />
                                                <label>"Description (optional)"</label>
                                                <textarea rows="3"
                                                    prop:value=edit_description
                                                    on:input=move |ev| set_edit_description.set(event_target_value(&ev)) />
                                                <label>"Poll embed URL (optional)"</label>
                                                <input type="url"
                                                    placeholder="https://rcv123.org/poll/..."
                                                    prop:value=edit_poll_url
                                                    on:input=move |ev| set_edit_poll_url.set(event_target_value(&ev)) />
                                                <label>"Poster image URL (optional)"</label>
                                                <input type="text"
                                                    placeholder="https://..."
                                                    prop:value=edit_poster_url
                                                    on:input=move |ev| set_edit_poster_url.set(event_target_value(&ev)) />
                                                <Show when=move || !edit_error.get().is_empty()>
                                                    <p class="error">{move || edit_error.get()}</p>
                                                </Show>
                                                <div style="display:flex;gap:0.5rem;margin-top:0.5rem;">
                                                    <button type="submit">"Save"</button>
                                                    <button type="button" class="secondary"
                                                        on:click=move |_| editing_id.set(None)
                                                    >"Cancel"</button>
                                                </div>
                                            </form>
                                        })}
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any(),
                }}

                <h2 style="margin-top:2rem;">"Invite Codes"</h2>
                <div class="card">
                    <h3>"Generate Code"</h3>
                    <form on:submit=handle_create_invite>
                        <label>"Role"</label>
                        <select
                            prop:value=invite_role
                            on:change=move |e| set_invite_role.set(event_target_value(&e))
                        >
                            <option value="member">"Member"</option>
                            <Show when=is_superadmin>
                                <option value="admin">"Admin"</option>
                            </Show>
                        </select>
                        <Show when=move || !invite_error.get().is_empty()>
                            <p class="error">{move || invite_error.get()}</p>
                        </Show>
                        <Show when=move || !invite_success.get().is_empty()>
                            <p class="success" style="font-family:monospace;font-size:1.1rem;">{move || invite_success.get()}</p>
                        </Show>
                        <button type="submit">"Generate"</button>
                    </form>
                </div>

                {move || match invites.get() {
                    None => view! { <p>"Loading..."</p> }.into_any(),
                    Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                    Some(Ok(list)) => view! {
                        <div>
                            {list.into_iter().map(|c| {
                                let id = c.id.clone();
                                let used = c.used;
                                view! {
                                    <div class="card" style="display:flex;justify-content:space-between;align-items:center;">
                                        <div>
                                            <code style="font-size:1rem;">{c.code.clone()}</code>
                                            <span style={format!("margin-left:0.75rem;color:{};font-size:0.8rem;",
                                                if used { "#888" } else { "#6bffb8" })}>
                                                {if used { "used" } else { "active" }}
                                            </span>
                                            <span style="margin-left:0.75rem;color:#aaa;font-size:0.8rem;">
                                                {c.role.clone()}
                                            </span>
                                        </div>
                                        {(!used).then(|| view! {
                                            <button class="secondary"
                                                on:click=move |_| handle_delete_invite(id.clone())
                                            >"Revoke"</button>
                                        })}
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any(),
                }}
            </Show>
        </main>
    }
}
