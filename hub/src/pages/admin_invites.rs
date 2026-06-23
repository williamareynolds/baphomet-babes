use auth_client::AuthUser;
use crate::api;
use crate::components::admin_nav::AdminNav;
use leptos::prelude::*;
use shared::CreateInviteRequest;
use thaw::{Button, ButtonAppearance, ButtonType, Card, Field, Input, Select};

/// Copy text to the clipboard (best effort; silently no-ops if unavailable).
fn copy_to_clipboard(text: &str) {
    if let Some(win) = web_sys::window() {
        let _ = win.navigator().clipboard().write_text(text);
    }
}

#[component]
pub fn AdminInvitesPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let is_admin = move || auth.get().map(|u| u.is_admin()).unwrap_or(false);
    let is_superadmin = move || auth.get().map(|u| u.is_superadmin()).unwrap_or(false);

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

    let invite_first = RwSignal::new(String::new());
    let invite_last = RwSignal::new(String::new());
    let invite_phone = RwSignal::new(String::new());
    let invite_role = RwSignal::new("member".to_string());
    let (invite_error, set_invite_error) = signal(String::new());
    let (invite_success, set_invite_success) = signal(String::new());
    // The id of the code whose copy button was last tapped, for "Copied!" feedback.
    let copied_id = RwSignal::new(String::new());

    let handle_create_invite = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_invite_error.set(String::new());
        set_invite_success.set(String::new());
        let Some(user) = auth.get() else { return };

        let first = invite_first.get().trim().to_string();
        if first.is_empty() {
            set_invite_error.set("First name is required.".into());
            return;
        }
        let last = invite_last.get().trim().to_string();
        let phone = invite_phone.get().trim().to_string();
        let req = CreateInviteRequest {
            role: invite_role.get(),
            first_name: first,
            last_name: (!last.is_empty()).then_some(last),
            phone: (!phone.is_empty()).then_some(phone),
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::create_invite(req, &user.token).await {
                Ok(code) => {
                    // Copy the fresh code so it's ready to paste straight into a DM.
                    copy_to_clipboard(&code.code);
                    set_invite_success.set(format!("Code {} created and copied.", code.code));
                    invite_first.set(String::new());
                    invite_last.set(String::new());
                    invite_phone.set(String::new());
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

    let handle_revoke_all = move |_| {
        let confirmed = web_sys::window()
            .and_then(|w| w.confirm_with_message(
                "Revoke ALL unused invite codes? This cannot be undone.",
            ).ok())
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        let Some(user) = auth.get() else { return };
        set_invite_error.set(String::new());
        set_invite_success.set(String::new());
        wasm_bindgen_futures::spawn_local(async move {
            match api::revoke_unused_invites(&user.token).await {
                Ok(n) => {
                    set_invite_success.set(format!("Revoked {n} unused code(s)."));
                    set_invite_refresh.update(|m| *m += 1);
                }
                Err(e) => set_invite_error.set(e),
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
                <AdminNav active="invites" is_superadmin=is_superadmin() />

                <Card>
                    <h2>"Generate Code"</h2>
                    <form on:submit=handle_create_invite>
                        <Field label="First name">
                            <Input value=invite_first placeholder="First name" />
                        </Field>
                        <Field label="Last name (optional)">
                            <Input value=invite_last placeholder="Last name" />
                        </Field>
                        <Field label="Phone (optional)">
                            <Input value=invite_phone placeholder="555-123-4567" />
                        </Field>
                        <Field label="Role">
                            <Select value=invite_role>
                                <option value="member">"Member"</option>
                                <Show when=is_superadmin>
                                    <option value="admin">"Admin"</option>
                                </Show>
                            </Select>
                        </Field>
                        <Show when=move || !invite_error.get().is_empty()>
                            <p class="error">{move || invite_error.get()}</p>
                        </Show>
                        <Show when=move || !invite_success.get().is_empty()>
                            <p class="success" style="font-size:1.05rem;">{move || invite_success.get()}</p>
                        </Show>
                        <Button button_type=ButtonType::Submit appearance=ButtonAppearance::Primary>
                            "Generate"
                        </Button>
                    </form>
                </Card>

                <div class="admin-row" style="margin-top:2rem;align-items:baseline;">
                    <h2 class="section-heading" style="margin:0;">"All Codes"</h2>
                    <Button appearance=ButtonAppearance::Subtle on_click=handle_revoke_all>
                        "Revoke all unused"
                    </Button>
                </div>
                {move || match invites.get() {
                    None => view! { <p>"Loading..."</p> }.into_any(),
                    Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                    Some(Ok(list)) => view! {
                        <div>
                            {list.into_iter().map(|c| {
                                let id = c.id.clone();
                                let code = c.code.clone();
                                let used = c.used;
                                // "First Last" — last name only when present.
                                let name = match c.last_name.as_deref() {
                                    Some(l) if !l.is_empty() => format!("{} {}", c.first_name, l),
                                    _ => c.first_name.clone(),
                                };
                                let copy_id = id.clone();
                                let copy_code = code.clone();
                                let on_copy = move |_| {
                                    copy_to_clipboard(&copy_code);
                                    copied_id.set(copy_id.clone());
                                };
                                let this_id = id.clone();
                                let copied_label = move || {
                                    if copied_id.get() == this_id { "Copied!" } else { "Copy" }
                                };
                                view! {
                                    <Card>
                                        <div class="admin-row">
                                            <div>
                                                <code style="font-size:1rem;">{code}</code>
                                                <span style={format!("margin-left:0.75rem;color:{};font-size:0.8rem;",
                                                    if used { "#bdafb2" } else { "#93d8b4" })}>
                                                    {if used { "used" } else { "active" }}
                                                </span>
                                                <span style="margin-left:0.75rem;color:#bdafb2;font-size:0.8rem;">
                                                    {c.role.clone()}
                                                </span>
                                                <Show when={let n = name.clone(); move || !n.is_empty()}>
                                                    <p style="margin-top:0.35rem;color:#d8cdcf;">{name.clone()}</p>
                                                </Show>
                                                {c.phone.clone().map(|p| view! {
                                                    <p style="color:#bdafb2;font-size:0.85rem;">{p}</p>
                                                })}
                                            </div>
                                            <div class="admin-actions">
                                                <Button
                                                    appearance=ButtonAppearance::Secondary
                                                    on_click=on_copy
                                                >{copied_label}</Button>
                                                {(!used).then(|| view! {
                                                    <Button
                                                        appearance=ButtonAppearance::Secondary
                                                        on_click=move |_| handle_delete_invite(id.clone())
                                                    >"Revoke"</Button>
                                                })}
                                            </div>
                                        </div>
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
