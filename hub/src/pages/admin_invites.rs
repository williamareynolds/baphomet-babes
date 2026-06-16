use auth_client::AuthUser;
use crate::api;
use crate::components::admin_nav::AdminNav;
use leptos::prelude::*;
use shared::CreateInviteRequest;
use thaw::{Button, ButtonAppearance, ButtonType, Card, Field, Select};

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

    let invite_role = RwSignal::new("member".to_string());
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
                <AdminNav active="invites" />

                <Card>
                    <h2>"Generate Code"</h2>
                    <form on:submit=handle_create_invite>
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

                <h2 class="section-heading">"All Codes"</h2>
                {move || match invites.get() {
                    None => view! { <p>"Loading..."</p> }.into_any(),
                    Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                    Some(Ok(list)) => view! {
                        <div>
                            {list.into_iter().map(|c| {
                                let id = c.id.clone();
                                let used = c.used;
                                view! {
                                    <Card>
                                        <div class="admin-row">
                                            <div>
                                                <code style="font-size:1rem;">{c.code.clone()}</code>
                                                <span style={format!("margin-left:0.75rem;color:{};font-size:0.8rem;",
                                                    if used { "#8a7a7a" } else { "#7ac09a" })}>
                                                    {if used { "used" } else { "active" }}
                                                </span>
                                                <span style="margin-left:0.75rem;color:#8a7a7a;font-size:0.8rem;">
                                                    {c.role.clone()}
                                                </span>
                                            </div>
                                            {(!used).then(|| view! {
                                                <div class="admin-actions">
                                                    <Button
                                                        appearance=ButtonAppearance::Secondary
                                                        on_click=move |_| handle_delete_invite(id.clone())
                                                    >"Revoke"</Button>
                                                </div>
                                            })}
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
