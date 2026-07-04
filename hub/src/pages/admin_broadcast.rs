use auth_client::AuthUser;
use crate::api;
use crate::components::admin_nav::AdminNav;
use leptos::prelude::*;
use shared::{BroadcastRequest, CHANNEL_TEST};
use thaw::{Button, ButtonAppearance, ButtonType, Card, Field, Input, Switch, Textarea};

/// Send a one-off push + inbox notification to the General channel. Unlike
/// announcements, a broadcast leaves no stored post — just the notification.
#[component]
pub fn AdminBroadcastPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let is_admin = move || auth.get().map(|u| u.is_admin()).unwrap_or(false);
    let is_superadmin = move || auth.get().map(|u| u.is_superadmin()).unwrap_or(false);

    let title = RwSignal::new(String::new());
    let body = RwSignal::new(String::new());
    let test_channel = RwSignal::new(false);
    let (error, set_error) = signal(String::new());
    let (success, set_success) = signal(String::new());
    let sending = RwSignal::new(false);

    let handle_send = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_error.set(String::new());
        set_success.set(String::new());
        let Some(user) = auth.get() else { return };
        sending.set(true);
        let to_test = test_channel.get();
        let req = BroadcastRequest {
            title: title.get(),
            body: body.get(),
            channel: to_test.then(|| CHANNEL_TEST.to_string()),
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::broadcast(req, &user.token).await {
                Ok(()) => {
                    set_success.set(if to_test {
                        "Test broadcast sent — only admins receive it.".into()
                    } else {
                        "Broadcast sent to the General channel.".into()
                    });
                    title.set(String::new());
                    body.set(String::new());
                    sending.set(false);
                }
                Err(e) => { set_error.set(e); sending.set(false); }
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
                <AdminNav active="broadcast" is_superadmin=is_superadmin() />

                <Card>
                    <h2>"Broadcast"</h2>
                    <p style="color:#bdafb2;margin-bottom:1rem;">
                        "Sends a push notification to everyone subscribed to the General "
                        "channel. It appears in their inbox but isn't saved as an announcement."
                    </p>
                    <form on:submit=handle_send>
                        <Field label="Title">
                            <Input value=title placeholder="Short headline" />
                        </Field>
                        <Field label="Message">
                            <Textarea value=body placeholder="What do you want people to know?" />
                        </Field>
                        <div style="margin:0.5rem 0 0.75rem;">
                            <Switch checked=test_channel label="Send to Test channel (only admins receive it, skips inboxes)" />
                        </div>
                        <Show when=move || !error.get().is_empty()>
                            <p class="error">{move || error.get()}</p>
                        </Show>
                        <Show when=move || !success.get().is_empty()>
                            <p class="success">{move || success.get()}</p>
                        </Show>
                        <Button
                            button_type=ButtonType::Submit
                            appearance=ButtonAppearance::Primary
                            loading=sending
                            disabled=sending
                        >
                            {move || if sending.get() { "Sending…" } else { "Send Broadcast" }}
                        </Button>
                    </form>
                </Card>
            </Show>
        </main>
    }
}
