use auth_client::AuthUser;
use crate::api;
use crate::components::admin_nav::AdminNav;
use leptos::prelude::*;
use shared::{UpdateUserRequest, UserSummary};
use thaw::{Button, ButtonAppearance, Card, Field, Select};

#[component]
pub fn AdminUsersPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let is_superadmin = move || auth.get().map(|u| u.is_superadmin()).unwrap_or(false);
    let my_id = move || auth.get().map(|u| u.id).unwrap_or_default();

    let (refresh, set_refresh) = signal(0u32);
    let users: RwSignal<Option<Result<Vec<UserSummary>, String>>> = RwSignal::new(None);

    Effect::new(move |_| {
        let _ = refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let result = match token {
                None => return,
                Some(t) => api::fetch_users(&t).await,
            };
            users.set(Some(result));
        });
    });

    let (error, set_error) = signal(String::new());

    // Apply a partial update (role and/or disabled) to one user, then refresh.
    let apply = move |id: String, req: UpdateUserRequest| {
        set_error.set(String::new());
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            match api::update_user(&id, req, &user.token).await {
                Ok(_) => set_refresh.update(|n| *n += 1),
                Err(e) => set_error.set(e),
            }
        });
    };

    view! {
        <main>
            <Show
                when=is_superadmin
                fallback=|| view! { <p class="error">"Access denied."</p> }
            >
                <h1>"Admin"</h1>
                <AdminNav active="users" is_superadmin=true />

                <Show when=move || !error.get().is_empty()>
                    <p class="error">{move || error.get()}</p>
                </Show>

                <h2 class="section-heading">"All Accounts"</h2>
                {move || match users.get() {
                    None => view! { <p>"Loading..."</p> }.into_any(),
                    Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                    Some(Ok(list)) => view! {
                        <div>
                            {list.into_iter().map(|u| {
                                let is_self = u.id == my_id();
                                let disabled = u.disabled;
                                let device_count = u.device_count;
                                // `default_value` is required: without it Thaw's Select clobbers the
                                // bound signal to its first <option> on mount. The current role here
                                // both seeds the signal and selects the right <option> on screen.
                                let original_role = u.role.clone();

                                // Role selector. We write only on an explicit Save click (never on
                                // mount or on change), so simply viewing the page can't alter anyone.
                                // The PUT is idempotent, so re-saving an unchanged role is harmless.
                                let role_sig = RwSignal::new(u.role.clone());

                                let save_id = u.id.clone();
                                let apply_save = apply;
                                let on_save = move |_| apply_save(save_id.clone(), UpdateUserRequest {
                                    role: Some(role_sig.get()),
                                    disabled: None,
                                });

                                let toggle_id = u.id.clone();
                                let apply_toggle = apply;
                                let on_toggle = move |_| apply_toggle(toggle_id.clone(), UpdateUserRequest {
                                    role: None,
                                    disabled: Some(!disabled),
                                });

                                view! {
                                    <Card>
                                        <div class="admin-row">
                                            <div>
                                                <strong>{u.username.clone()}</strong>
                                                <span style="margin-left:0.75rem;color:#bdafb2;font-size:0.8rem;">
                                                    {u.email.clone()}
                                                </span>
                                                <Show when=move || disabled>
                                                    <span style="margin-left:0.75rem;color:#e09aa6;font-size:0.8rem;">
                                                        "disabled"
                                                    </span>
                                                </Show>
                                                <span style={format!("display:block;margin-top:0.25rem;font-size:0.8rem;color:{};",
                                                    if device_count > 0 { "#93d8b4" } else { "#bdafb2" })}>
                                                    {if device_count == 1 {
                                                        "1 device enrolled".to_string()
                                                    } else {
                                                        format!("{device_count} devices enrolled")
                                                    }}
                                                </span>
                                            </div>
                                            <div class="admin-actions">
                                                {if is_self {
                                                    view! {
                                                        <span style="color:#bdafb2;font-size:0.8rem;">
                                                            {format!("{} (you)", u.role)}
                                                        </span>
                                                    }.into_any()
                                                } else {
                                                    view! {
                                                        <Field label="Role">
                                                            <Select value=role_sig default_value=original_role.clone()>
                                                                <option value="member">"Member"</option>
                                                                <option value="admin">"Admin"</option>
                                                                <option value="superadmin">"Superadmin"</option>
                                                            </Select>
                                                        </Field>
                                                        <Button
                                                            appearance=ButtonAppearance::Primary
                                                            on_click=on_save
                                                        >
                                                            "Save"
                                                        </Button>
                                                        <Button
                                                            appearance=ButtonAppearance::Secondary
                                                            on_click=on_toggle
                                                        >
                                                            {if disabled { "Enable" } else { "Disable" }}
                                                        </Button>
                                                    }.into_any()
                                                }}
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
