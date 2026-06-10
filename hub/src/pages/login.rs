use auth_client::{AuthUser, save_auth};
use crate::api;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use shared::{AuthResponse, LoginRequest, RegisterRequest};

#[component]
pub fn LoginPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let (tab, set_tab) = signal("login");
    let (error, set_error) = signal(String::new());
    let (loading, set_loading) = signal(false);
    let (auth_response, set_auth_response) = signal(None::<AuthResponse>);

    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());

    let (reg_email, set_reg_email) = signal(String::new());
    let (reg_username, set_reg_username) = signal(String::new());
    let (reg_password, set_reg_password) = signal(String::new());
    let (invite_code, set_invite_code) = signal(String::new());

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

    view! {
        <main style="max-width:460px;padding:4rem 2rem 6rem;">

            <p style="font-family:'IBM Plex Mono',monospace;font-size:0.55rem;letter-spacing:0.35em;text-transform:uppercase;color:#5a4a5a;margin-bottom:0.5rem;">
                "Members Only"
            </p>
            <h1 style="font-size:4.5rem;line-height:0.92;letter-spacing:0.04em;color:#e2d8d0;margin-bottom:0.35rem;">
                "Baphomet"
                <span style="display:block;color:#c41e3a;">"Babes"</span>
            </h1>
            <p style="font-family:'IBM Plex Mono',monospace;font-size:0.6rem;letter-spacing:0.22em;text-transform:uppercase;color:#7a5a6a;margin-bottom:3rem;">
                "of Bentonville"
            </p>

            <div style="display:flex;border-bottom:1px solid #251e2c;margin-bottom:2rem;">
                <button
                    on:click=move |_| { set_tab.set("login"); set_error.set(String::new()); }
                    style={move || format!(
                        "background:transparent;border:none;border-bottom:2px solid {};border-radius:0;\
                         padding:0.5rem 1.25rem 0.65rem;margin-bottom:-1px;\
                         font-family:'IBM Plex Mono',monospace;font-size:0.62rem;letter-spacing:0.14em;\
                         text-transform:uppercase;color:{};cursor:pointer;",
                        if tab.get() == "login" { "#c41e3a" } else { "transparent" },
                        if tab.get() == "login" { "#e2d8d0" } else { "#6a5a6a" }
                    )}
                >"Enter"</button>
                <button
                    on:click=move |_| { set_tab.set("register"); set_error.set(String::new()); }
                    style={move || format!(
                        "background:transparent;border:none;border-bottom:2px solid {};border-radius:0;\
                         padding:0.5rem 1.25rem 0.65rem;margin-bottom:-1px;\
                         font-family:'IBM Plex Mono',monospace;font-size:0.62rem;letter-spacing:0.14em;\
                         text-transform:uppercase;color:{};cursor:pointer;",
                        if tab.get() == "register" { "#c41e3a" } else { "transparent" },
                        if tab.get() == "register" { "#e2d8d0" } else { "#6a5a6a" }
                    )}
                >"Request Entry"</button>
            </div>

            <Show when=move || !error.get().is_empty()>
                <p class="error" style="margin-bottom:1.25rem;">{move || error.get()}</p>
            </Show>

            <Show when=move || tab.get() == "login">
                <form on:submit=handle_login style="max-width:100%;">
                    <div style="margin-bottom:1.25rem;">
                        <label style="color:#9a8a9a;">"Email"</label>
                        <input id="login-email" type="email" required
                            prop:value=email
                            on:input=move |e| set_email.set(event_target_value(&e))
                            style="background:#0f0b14;border-color:#2e2438;color:#e2d8d0;" />
                    </div>
                    <div style="margin-bottom:0.5rem;">
                        <label style="color:#9a8a9a;">"Password"</label>
                        <input id="login-password" type="password" required
                            prop:value=password
                            on:input=move |e| set_password.set(event_target_value(&e))
                            style="background:#0f0b14;border-color:#2e2438;color:#e2d8d0;" />
                    </div>
                    <div style="margin-top:1.5rem;">
                        <button type="submit"
                            disabled=move || loading.get()
                            style="display:inline-block;padding:0.65rem 2.5rem;background:#c41e3a;color:#fff;border:none;border-radius:3px;cursor:pointer;font-family:'IBM Plex Mono',monospace;font-size:0.7rem;font-weight:600;letter-spacing:0.12em;text-transform:uppercase;">
                            {move || if loading.get() { "Entering..." } else { "Enter" }}
                        </button>
                    </div>
                </form>
            </Show>

            <Show when=move || tab.get() == "register">
                <div style="margin-bottom:1.5rem;">
                    <span class="badge badge-special">"Invitation Required"</span>
                    <p style="font-size:1.05rem;line-height:1.65;color:#7a6a7a;margin-top:0.75rem;font-style:italic;">
                        "Membership is by invitation. If you have a code, you're welcome here."
                    </p>
                </div>
                <form on:submit=handle_register style="max-width:100%;">
                    <div>
                        <label style="color:#9a8a9a;">"Email"</label>
                        <input id="reg-email" type="email" required
                            prop:value=reg_email
                            on:input=move |e| set_reg_email.set(event_target_value(&e))
                            style="background:#0f0b14;border-color:#2e2438;color:#e2d8d0;" />
                    </div>
                    <div>
                        <label style="color:#9a8a9a;">"Username"</label>
                        <input id="reg-username" type="text" required
                            prop:value=reg_username
                            on:input=move |e| set_reg_username.set(event_target_value(&e))
                            style="background:#0f0b14;border-color:#2e2438;color:#e2d8d0;" />
                    </div>
                    <div>
                        <label style="color:#9a8a9a;">"Password"</label>
                        <input id="reg-password" type="password" required
                            prop:value=reg_password
                            on:input=move |e| set_reg_password.set(event_target_value(&e))
                            style="background:#0f0b14;border-color:#2e2438;color:#e2d8d0;" />
                    </div>
                    <div>
                        <label style="color:#c41e3a;">"Invite Code"</label>
                        <input id="reg-invite" type="text" required
                            prop:value=invite_code
                            on:input=move |e| set_invite_code.set(event_target_value(&e))
                            style="background:#0f0b14;border-color:rgba(196,30,58,0.4);color:#e2d8d0;" />
                    </div>
                    <div style="margin-top:1.5rem;">
                        <button type="submit"
                            disabled=move || loading.get()
                            style="display:inline-block;padding:0.65rem 2.5rem;background:#c41e3a;color:#fff;border:none;border-radius:3px;cursor:pointer;font-family:'IBM Plex Mono',monospace;font-size:0.7rem;font-weight:600;letter-spacing:0.12em;text-transform:uppercase;">
                            {move || if loading.get() { "Joining..." } else { "Join" }}
                        </button>
                    </div>
                </form>
            </Show>

            <p style="margin-top:3rem;padding-top:1.5rem;border-top:1px solid #1a1220;font-family:'IBM Plex Mono',monospace;font-size:0.6rem;letter-spacing:0.08em;color:#4a3a4a;line-height:1.7;">
                "All are welcome. No exceptions."
            </p>

        </main>
    }
}
