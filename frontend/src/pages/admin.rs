use crate::{api, context::AuthUser};
use leptos::prelude::*;
use shared::CreateEventRequest;

#[component]
pub fn AdminPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let is_admin = move || auth.get().map(|u| u.is_admin()).unwrap_or(false);

    let (refresh, set_refresh) = signal(0u32);
    let events: RwSignal<Option<Result<Vec<shared::Event>, String>>> = RwSignal::new(None);

    Effect::new(move |_| {
        let _ = refresh.get(); // subscribe
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
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::create_event(req, &user.token).await {
                Ok(_) => {
                    set_form_success.set("Event created!".into());
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_form_error.set(e),
            }
        });
    };

    let handle_delete = move |id: String| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if api::delete_event(&id, &user.token).await.is_ok() {
                set_refresh.update(|n| *n += 1);
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
                                view! {
                                    <div class="card" style="display:flex;justify-content:space-between;align-items:flex-start;">
                                        <div>
                                            <span class={format!("badge badge-{}", e.event_type)}>
                                                {e.event_type.clone()}
                                            </span>
                                            <strong style="display:block;margin-top:0.25rem;">{e.title}</strong>
                                            <small style="color:#aaa;">{e.date}</small>
                                            {e.poll_embed_url.map(|_| view! {
                                                <span style="color:#6bffb8;font-size:0.8rem;display:block;">"✓ poll set"</span>
                                            })}
                                        </div>
                                        <button class="secondary"
                                            on:click=move |_| handle_delete(id.clone())
                                        >"Delete"</button>
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
