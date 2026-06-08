use crate::{
    api,
    context::{AuthUser, save_auth},
};
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use shared::{AuthResponse, LoginRequest, RegisterRequest};

#[component]
pub fn LoginPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let (tab, set_tab) = signal("login");
    let (error, set_error) = signal(String::new());
    let (loading, set_loading) = signal(false);

    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());

    let (reg_email, set_reg_email) = signal(String::new());
    let (reg_username, set_reg_username) = signal(String::new());
    let (reg_password, set_reg_password) = signal(String::new());
    let (invite_code, set_invite_code) = signal(String::new());

    // Auth response set after successful login/register; Effect reacts to trigger navigation
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
        let req = LoginRequest {
            email: email.get(),
            password: password.get(),
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::login(req).await {
                Ok(resp) => set_auth_response.set(Some(resp)),
                Err(e) => {
                    set_error.set(e);
                    set_loading.set(false);
                }
            }
        });
    };

    let handle_register = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_error.set(String::new());
        set_loading.set(true);
        let req = RegisterRequest {
            email: reg_email.get(),
            username: reg_username.get(),
            password: reg_password.get(),
            invite_code: invite_code.get(),
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::register(req).await {
                Ok(resp) => set_auth_response.set(Some(resp)),
                Err(e) => {
                    set_error.set(e);
                    set_loading.set(false);
                }
            }
        });
    };

    view! {
        <main>
            <h1>"Account"</h1>
            <div style="display:flex;gap:1rem;margin-bottom:1.5rem;">
                <button
                    class={move || if tab.get() == "login" { "" } else { "secondary" }}
                    on:click=move |_| { set_tab.set("login"); set_error.set(String::new()); }
                >"Login"</button>
                <button
                    class={move || if tab.get() == "register" { "" } else { "secondary" }}
                    on:click=move |_| { set_tab.set("register"); set_error.set(String::new()); }
                >"Register"</button>
            </div>

            <Show when=move || !error.get().is_empty()>
                <p class="error">{move || error.get()}</p>
            </Show>

            <Show when=move || tab.get() == "login">
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
            </Show>

            <Show when=move || tab.get() == "register">
                <form on:submit=handle_register>
                    <label>"Email"</label>
                    <input type="email" required
                        prop:value=reg_email
                        on:input=move |e| set_reg_email.set(event_target_value(&e)) />
                    <label>"Username"</label>
                    <input type="text" required
                        prop:value=reg_username
                        on:input=move |e| set_reg_username.set(event_target_value(&e)) />
                    <label>"Password"</label>
                    <input type="password" required
                        prop:value=reg_password
                        on:input=move |e| set_reg_password.set(event_target_value(&e)) />
                    <label>"Invite Code"</label>
                    <input type="text" required
                        prop:value=invite_code
                        on:input=move |e| set_invite_code.set(event_target_value(&e)) />
                    <button type="submit" disabled=loading>
                        {move || if loading.get() { "Registering..." } else { "Register" }}
                    </button>
                </form>
            </Show>
        </main>
    }
}
