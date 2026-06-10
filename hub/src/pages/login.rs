use auth_client::{AuthUser, save_auth};
use crate::api;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use shared::{AuthResponse, LoginRequest};

#[component]
pub fn LoginPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
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
                <label>"Email"</label>
                <input type="email" required
                    prop:value=email
                    on:input=move |e| set_email.set(event_target_value(&e)) />
                <label>"Password"</label>
                <input type="password" required
                    prop:value=password
                    on:input=move |e| set_password.set(event_target_value(&e)) />
                <button type="submit" disabled=loading>
                    {move || if loading.get() { "Logging in..." } else { "Login" }}
                </button>
            </form>
            <p style="margin-top:1.5rem;font-family:'IBM Plex Mono',monospace;font-size:0.7rem;color:#4a3a5a;">
                "Need an account? Register at "
                <a href="https://movienight.baphometbabes.com/login" style="color:#c41e3a;">
                    "movienight.baphometbabes.com"
                </a>
                " with an invite code."
            </p>
        </main>
    }
}
