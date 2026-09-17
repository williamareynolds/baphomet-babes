//! Scoped permission grants ("Roles"). One row per member, one `<Switch>` per
//! `shared::KNOWN_PERMISSIONS` entry — adding the next scoped role is just
//! appending to that list, no changes needed here.

use auth_client::AuthUser;
use crate::api;
use crate::components::admin_nav::AdminNav;
use leptos::prelude::*;
use shared::{KNOWN_PERMISSIONS, MemberPermissions, UpdateMemberPermissionRequest};
use thaw::{Card, Switch};

#[component]
pub fn AdminRolesPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let is_admin = move || auth.get().map(|u| u.is_admin()).unwrap_or(false);
    let is_superadmin = move || auth.get().map(|u| u.is_superadmin()).unwrap_or(false);

    let (refresh, set_refresh) = signal(0u32);
    let members: RwSignal<Option<Result<Vec<MemberPermissions>, String>>> = RwSignal::new(None);
    let (error, set_error) = signal(String::new());

    Effect::new(move |_| {
        let _ = refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(t) = token else { return };
            members.set(Some(api::fetch_roles(&t).await));
        });
    });

    let toggle = move |user_id: String, permission: String, granted: bool| {
        set_error.set(String::new());
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            let req = UpdateMemberPermissionRequest { permission, granted };
            match api::update_role_permission(&user_id, req, &user.token).await {
                Ok(_) => set_refresh.update(|n| *n += 1),
                Err(e) => set_error.set(e),
            }
        });
    };

    view! {
        <main class="page">
            <Show
                when=is_admin
                fallback=|| view! { <p class="error">"Access denied."</p> }
            >
                <AdminNav active="roles" is_admin=true is_superadmin=is_superadmin() />
                <h1 class="page-title">"Roles"</h1>
                <p class="page-sub">
                    "Scoped permissions — a member can hold one of these without becoming a full admin."
                </p>

                <Show when=move || !error.get().is_empty()>
                    <p class="error">{move || error.get()}</p>
                </Show>

                {move || match members.get() {
                    None => view! { <p>"Loading…"</p> }.into_any(),
                    Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                    Some(Ok(list)) => view! {
                        <div>
                            {list.into_iter().map(|m| {
                                let label = m.display_name.clone().unwrap_or_else(|| m.username.clone());
                                view! {
                                    <Card>
                                        <div class="admin-row">
                                            <strong>{label}</strong>
                                            <div class="admin-actions">
                                                {KNOWN_PERMISSIONS.iter().map(|(key, name)| {
                                                    let key = key.to_string();
                                                    let has = m.permissions.iter().any(|p| p == &key);
                                                    let checked = RwSignal::new(has);
                                                    let user_id = m.user_id.clone();
                                                    // `Switch` two-way binds `checked` with no click callback
                                                    // of its own (see rides.rs's mountain-bike toggle for the
                                                    // same pattern) — an effect watches for it diverging from
                                                    // the value this row was fetched with and PUTs the change.
                                                    // Runs once harmlessly on mount (want == has there).
                                                    Effect::new(move |_| {
                                                        let want = checked.get();
                                                        if want == has {
                                                            return;
                                                        }
                                                        toggle(user_id.clone(), key.clone(), want);
                                                    });
                                                    view! {
                                                        <label style="display:flex;align-items:center;gap:0.4rem;">
                                                            <Switch checked=checked />
                                                            <span style="font-size:0.8rem;color:#bdafb2;">{*name}</span>
                                                        </label>
                                                    }
                                                }).collect::<Vec<_>>()}
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
