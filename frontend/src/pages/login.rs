use auth_client::{AuthUser, save_auth};
use crate::api;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use shared::{AuthResponse, LoginRequest};
use thaw::{Button, ButtonAppearance, ButtonType, Field, Input, InputType};

#[component]
pub fn LoginPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let (error, set_error) = signal(String::new());
    let (loading, set_loading) = signal(false);
    let (auth_response, set_auth_response) = signal(None::<AuthResponse>);

    let navigate = use_navigate();
    Effect::new(move |_| {
        if let Some(resp) = auth_response.get() {
            let user = AuthUser {
                id: resp.user.id,
                email: resp.user.email,
                username: resp.user.username,
                role: resp.user.role,
                token: resp.token,
            };
            save_auth(&user);
            auth.set(Some(user));
            navigate("/", Default::default());
        }
    });

    let handle_login = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_error.set(String::new());
        set_loading.set(true);
        let req = LoginRequest { email: email.get(), password: password.get() };
        wasm_bindgen_futures::spawn_local(async move {
            match api::login(req).await {
                Ok(resp) => set_auth_response.set(Some(resp)),
                Err(e) => { set_error.set(e); set_loading.set(false); }
            }
        });
    };

    view! {
        <main>
            <h1>"Login"</h1>
            <Show when=move || !error.get().is_empty()>
                <p class="error">{move || error.get()}</p>
            </Show>
            <form on:submit=handle_login>
                <Field label="Email">
                    <Input value=email input_type=InputType::Email placeholder="you@example.com" />
                </Field>
                <Field label="Password">
                    <Input value=password input_type=InputType::Password placeholder="••••••••" />
                </Field>
                <Button
                    button_type=ButtonType::Submit
                    appearance=ButtonAppearance::Primary
                    loading=loading
                    disabled=loading
                >
                    "Login"
                </Button>
            </form>
            <p class="login-hint">
                "New member? "
                <a href="https://baphometbabes.com/login">"Register at baphometbabes.com"</a>
            </p>
        </main>
    }
}
