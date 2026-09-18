use auth_client::{AuthUser, save_auth};
use crate::api;
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};
use thaw::{Button, ButtonAppearance, ButtonType, Field, Input, InputType};

/// Public page for the link in a superadmin-issued reset email
/// (`/reset-password?token=...`). No session required — the token in the URL
/// is the credential. On success it logs the member straight in, same as
/// `LoginPage` does after login/register.
#[component]
pub fn ResetPasswordPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let reset_token = use_query_map().get_untracked().get("token").unwrap_or_default();

    let password = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let (error, set_error) = signal(String::new());
    let (loading, set_loading) = signal(false);

    // The account this link resets. Also gives password managers an
    // `autocomplete="username"` field to match the new password against —
    // without it, a manager may save the reset as a disconnected credential
    // instead of updating the member's existing one.
    let email = RwSignal::new(String::new());
    let (link_error, set_link_error) = signal(String::new());
    {
        let reset_token = reset_token.clone();
        Effect::new(move |_| {
            if reset_token.is_empty() {
                return;
            }
            let reset_token = reset_token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::password_reset_info(&reset_token).await {
                    Ok(info) => email.set(info.email),
                    Err(e) => set_link_error.set(e),
                }
            });
        });
    }

    let navigate = use_navigate();
    let token_for_submit = reset_token.clone();
    let handle_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_error.set(String::new());

        let pw = password.get();
        if pw.len() < 8 {
            set_error.set("Password must be at least 8 characters.".to_string());
            return;
        }
        if pw != confirm.get() {
            set_error.set("Passwords don't match.".to_string());
            return;
        }

        set_loading.set(true);
        let reset_token = token_for_submit.clone();
        let navigate = navigate.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match api::consume_password_reset(&reset_token, pw).await {
                Ok(resp) => {
                    let user = AuthUser {
                        id: resp.user.id,
                        email: resp.user.email,
                        username: resp.user.username,
                        role: resp.user.role,
                        token: resp.token,
                        permissions: resp.user.permissions,
                    };
                    save_auth(&user);
                    auth.set(Some(user));
                    navigate("/", Default::default());
                }
                Err(e) => { set_error.set(e); set_loading.set(false); }
            }
        });
    };

    view! {
        <main style="max-width:460px;padding:4rem 2rem 6rem;">
            <h1 style="font-size:clamp(2.2rem,10vw,3rem);line-height:0.95;letter-spacing:0.04em;color:#f3ebe3;margin-bottom:2rem;">
                "Choose a new password"
            </h1>

            <Show when=move || reset_token.is_empty()>
                <p class="error">"This link is missing its token."</p>
            </Show>

            <Show when=move || !link_error.get().is_empty()>
                <p class="error" style="margin-bottom:1.25rem;">{move || link_error.get()}</p>
            </Show>

            <Show when=move || !error.get().is_empty()>
                <p class="error" style="margin-bottom:1.25rem;">{move || error.get()}</p>
            </Show>

            <form on:submit=handle_submit>
                // Real, visible, readonly — not a hidden field, which some
                // password managers ignore. Its `autocomplete="username"` is
                // what lets the manager tie the new password to the member's
                // existing saved login instead of a new one.
                <Show when=move || !email.get().is_empty()>
                    <Field label="Account">
                        <Input
                            id="reset-email"
                            name="username"
                            autocomplete="username"
                            readonly=true
                            value=email
                        />
                    </Field>
                </Show>
                <Field label="New password">
                    <Input id="reset-password" name="new-password" autocomplete="new-password" value=password input_type=InputType::Password placeholder="••••••••" />
                </Field>
                <Field label="Confirm password">
                    <Input id="reset-confirm" name="confirm-password" autocomplete="new-password" value=confirm input_type=InputType::Password placeholder="••••••••" />
                </Field>
                <Button
                    button_type=ButtonType::Submit
                    appearance=ButtonAppearance::Primary
                    loading=loading
                    disabled=Signal::derive(move || loading.get() || !link_error.get().is_empty())
                >
                    {move || if loading.get() { "Saving..." } else { "Save password" }}
                </Button>
            </form>
        </main>
    }
}
