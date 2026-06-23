use auth_client::{AuthUser, save_auth};
use crate::api;
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};
use shared::{AuthResponse, LoginRequest, RegisterRequest};
use thaw::{Button, ButtonAppearance, ButtonType, Field, Input, InputType};

#[component]
pub fn LoginPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let (tab, set_tab) = signal("login");
    let (error, set_error) = signal(String::new());
    let (loading, set_loading) = signal(false);
    let (auth_response, set_auth_response) = signal(None::<AuthResponse>);

    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());

    let reg_email = RwSignal::new(String::new());
    let reg_username = RwSignal::new(String::new());
    let reg_password = RwSignal::new(String::new());
    let invite_code = RwSignal::new(String::new());

    // Single-use invite links: /login?code=XXX prefills the code and drops the
    // recipient straight onto the register tab so they never type it by hand.
    if let Some(code) = use_query_map().get_untracked().get("code") {
        if !code.is_empty() {
            invite_code.set(code);
            set_tab.set("register");
        }
    }

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
                Err(e) => { set_error.set(e); set_loading.set(false); }
            }
        });
    };

    let tab_style = move |which: &str| format!(
        "background:transparent;border:none;border-bottom:2px solid {};border-radius:0;\
         padding:0.5rem 1.25rem 0.65rem;margin-bottom:-1px;\
         font-family:'IBM Plex Mono',monospace;font-size:0.62rem;letter-spacing:0.14em;\
         text-transform:uppercase;color:{};cursor:pointer;",
        if tab.get() == which { "#c41e3a" } else { "transparent" },
        if tab.get() == which { "#f3ebe3" } else { "#ad9ea4" },
    );

    view! {
        <main style="max-width:460px;padding:4rem 2rem 6rem;">
            <p style="font-family:'IBM Plex Mono',monospace;font-size:0.55rem;letter-spacing:0.35em;text-transform:uppercase;color:#ad9ea4;margin-bottom:0.5rem;">
                "Members Only"
            </p>
            <h1 style="font-size:4.5rem;line-height:0.92;letter-spacing:0.04em;color:#f3ebe3;margin-bottom:0.35rem;">
                "Baphomet"
                <span style="display:block;color:#ee4b61;">"Babes"</span>
            </h1>
            <p style="font-family:'IBM Plex Mono',monospace;font-size:0.6rem;letter-spacing:0.22em;text-transform:uppercase;color:#bdafb2;margin-bottom:3rem;">
                "of Bentonville"
            </p>

            <div style="display:flex;border-bottom:1px solid #251e2c;margin-bottom:2rem;">
                <button
                    on:click=move |_| { set_tab.set("login"); set_error.set(String::new()); }
                    style=move || tab_style("login")
                >"Enter"</button>
                <button
                    on:click=move |_| { set_tab.set("register"); set_error.set(String::new()); }
                    style=move || tab_style("register")
                >"Request Entry"</button>
            </div>

            <Show when=move || !error.get().is_empty()>
                <p class="error" style="margin-bottom:1.25rem;">{move || error.get()}</p>
            </Show>

            <Show when=move || tab.get() == "login">
                <form on:submit=handle_login>
                    <Field label="Email">
                        <Input id="login-email" name="email" autocomplete="username" value=email input_type=InputType::Email placeholder="you@example.com" />
                    </Field>
                    <Field label="Password">
                        <Input id="login-password" name="password" autocomplete="current-password" value=password input_type=InputType::Password placeholder="••••••••" />
                    </Field>
                    <Button
                        button_type=ButtonType::Submit
                        appearance=ButtonAppearance::Primary
                        loading=loading
                        disabled=loading
                    >
                        {move || if loading.get() { "Entering..." } else { "Enter" }}
                    </Button>
                </form>
            </Show>

            <Show when=move || tab.get() == "register">
                <div style="margin-bottom:1.5rem;">
                    <span class="badge badge-special">"Invitation Required"</span>
                    <p style="font-size:1.05rem;line-height:1.65;color:#bdafb2;margin-top:0.75rem;font-style:italic;">
                        "Membership is by invitation. If you have a code, you're welcome here."
                    </p>
                </div>
                <form on:submit=handle_register>
                    <Field label="Email">
                        <Input id="reg-email" name="email" autocomplete="email" value=reg_email input_type=InputType::Email placeholder="you@example.com" />
                    </Field>
                    <Field label="Username">
                        <Input id="reg-username" name="username" autocomplete="username" value=reg_username placeholder="username" />
                    </Field>
                    <Field label="Password">
                        <Input id="reg-password" name="new-password" autocomplete="new-password" value=reg_password input_type=InputType::Password placeholder="••••••••" />
                    </Field>
                    <Field label="Invite Code">
                        <Input id="reg-invite" name="invite-code" autocomplete="off" value=invite_code placeholder="your invite code" />
                    </Field>
                    <Button
                        button_type=ButtonType::Submit
                        appearance=ButtonAppearance::Primary
                        loading=loading
                        disabled=loading
                    >
                        {move || if loading.get() { "Joining..." } else { "Join" }}
                    </Button>
                </form>
            </Show>

            <p style="margin-top:3rem;padding-top:1.5rem;border-top:1px solid #1a1220;font-family:'IBM Plex Mono',monospace;font-size:0.6rem;letter-spacing:0.08em;color:#95868f;line-height:1.7;">
                "All are welcome. No exceptions."
            </p>
        </main>
    }
}
