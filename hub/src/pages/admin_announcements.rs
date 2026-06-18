use auth_client::AuthUser;
use crate::api;
use crate::components::admin_nav::AdminNav;
use leptos::prelude::*;
use shared::{CreateAnnouncementRequest, UpdateAnnouncementRequest};
use thaw::{
    Button, ButtonAppearance, ButtonType, Card, Field, Input, InputType, Textarea,
};

#[component]
pub fn AdminAnnouncementsPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let is_admin = move || auth.get().map(|u| u.is_admin()).unwrap_or(false);
    let is_superadmin = move || auth.get().map(|u| u.is_superadmin()).unwrap_or(false);

    let (refresh, set_refresh) = signal(0u32);
    let announcements: RwSignal<Option<Result<Vec<shared::Announcement>, String>>> =
        RwSignal::new(None);

    Effect::new(move |_| {
        let _ = refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let result = match token {
                None => return,
                Some(t) => api::fetch_announcements(&t).await,
            };
            announcements.set(Some(result));
        });
    });

    let title = RwSignal::new(String::new());
    let body = RwSignal::new(String::new());
    let poll_url = RwSignal::new(String::new());
    let (form_error, set_form_error) = signal(String::new());
    let (form_success, set_form_success) = signal(String::new());

    let handle_create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_form_error.set(String::new());
        set_form_success.set(String::new());
        let Some(user) = auth.get() else { return };
        let req = CreateAnnouncementRequest {
            title: title.get(),
            body: body.get(),
            poll_embed_url: if poll_url.get().is_empty() { None } else { Some(poll_url.get()) },
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::create_announcement(req, &user.token).await {
                Ok(_) => {
                    set_form_success.set("Announcement posted!".into());
                    title.set(String::new());
                    body.set(String::new());
                    poll_url.set(String::new());
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_form_error.set(e),
            }
        });
    };

    let handle_delete = move |id: String| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if api::delete_announcement(&id, &user.token).await.is_ok() {
                set_refresh.update(|n| *n += 1);
            }
        });
    };

    let editing_id: RwSignal<Option<String>> = RwSignal::new(None);
    let edit_title = RwSignal::new(String::new());
    let edit_body = RwSignal::new(String::new());
    let edit_poll_url = RwSignal::new(String::new());
    let (edit_error, set_edit_error) = signal(String::new());

    let handle_edit_start = move |a: shared::Announcement| {
        edit_title.set(a.title.clone());
        edit_body.set(a.body.clone());
        edit_poll_url.set(a.poll_embed_url.clone().unwrap_or_default());
        set_edit_error.set(String::new());
        editing_id.set(Some(a.id));
    };

    let handle_edit_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(id) = editing_id.get() else { return };
        let Some(user) = auth.get() else { return };
        set_edit_error.set(String::new());
        let req = UpdateAnnouncementRequest {
            title: Some(edit_title.get()),
            body: Some(edit_body.get()),
            poll_embed_url: if edit_poll_url.get().is_empty() { None } else { Some(edit_poll_url.get()) },
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::update_announcement(&id, req, &user.token).await {
                Ok(_) => {
                    editing_id.set(None);
                    set_refresh.update(|n| *n += 1);
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
                <AdminNav active="announcements" is_superadmin=is_superadmin() />

                <Card>
                    <h2>"New Announcement"</h2>
                    <form on:submit=handle_create>
                        <Field label="Title">
                            <Input value=title placeholder="What's happening" />
                        </Field>
                        <Field label="Body">
                            <Textarea value=body placeholder="Tell the members…" />
                        </Field>
                        <Field label="rcv123 poll embed URL (optional — src from their iframe code)">
                            <Input value=poll_url input_type=InputType::Url placeholder="https://rcv123.org/poll/..." />
                        </Field>
                        <Show when=move || !form_error.get().is_empty()>
                            <p class="error">{move || form_error.get()}</p>
                        </Show>
                        <Show when=move || !form_success.get().is_empty()>
                            <p class="success">{move || form_success.get()}</p>
                        </Show>
                        <Button button_type=ButtonType::Submit appearance=ButtonAppearance::Primary>
                            "Post Announcement"
                        </Button>
                    </form>
                </Card>

                <h2 class="section-heading">"All Announcements"</h2>
                {move || match announcements.get() {
                    None => view! { <p>"Loading..."</p> }.into_any(),
                    Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                    Some(Ok(list)) => view! {
                        <div>
                            {list.into_iter().map(|a| {
                                let id = a.id.clone();
                                // Reactive body so toggling `editing_id` swaps
                                // display↔form in place (see admin_events).
                                view! {
                                    <Card>
                                        {move || {
                                            let editing = editing_id.get().as_deref() == Some(id.as_str());
                                            if editing {
                                                view! {
                                                    <form on:submit=handle_edit_submit>
                                                        <Field label="Title">
                                                            <Input value=edit_title />
                                                        </Field>
                                                        <Field label="Body">
                                                            <Textarea value=edit_body />
                                                        </Field>
                                                        <Field label="Poll embed URL (optional)">
                                                            <Input value=edit_poll_url input_type=InputType::Url placeholder="https://rcv123.org/poll/..." />
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
                                                let a2 = a.clone();
                                                let id2 = id.clone();
                                                view! {
                                                    <div class="admin-row">
                                                        <div>
                                                            <strong style="display:block;">{a2.title.clone()}</strong>
                                                            {a2.poll_embed_url.clone().map(|_| view! {
                                                                <span class="poll-set">"✓ poll set"</span>
                                                            })}
                                                        </div>
                                                        <div class="admin-actions">
                                                            <Button
                                                                appearance=ButtonAppearance::Secondary
                                                                on_click=move |_| handle_edit_start(a2.clone())
                                                            >"Edit"</Button>
                                                            <Button
                                                                appearance=ButtonAppearance::Secondary
                                                                on_click=move |_| handle_delete(id2.clone())
                                                            >"Delete"</Button>
                                                        </div>
                                                    </div>
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
