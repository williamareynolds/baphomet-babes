use auth_client::AuthUser;
use crate::api;
use leptos::prelude::*;
use leptos_router::components::A;
use thaw::{Button, ButtonAppearance, Card};

fn channel_label(c: &str) -> &'static str {
    match c {
        shared::CHANNEL_ANNOUNCEMENTS => "Announcement",
        shared::CHANNEL_GENERAL => "General",
        shared::CHANNEL_MOVIE_NIGHT => "Movie Night",
        _ => "Notice",
    }
}

/// Format a unix-seconds timestamp using the browser's locale.
fn pretty_time(secs: i64) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64((secs as f64) * 1000.0));
    d.to_locale_string("default", &wasm_bindgen::JsValue::UNDEFINED)
        .as_string()
        .unwrap_or_default()
}

#[component]
pub fn NotificationsPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let items: RwSignal<Option<Result<Vec<shared::Notification>, String>>> = RwSignal::new(None);

    Effect::new(move |_| {
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let result = match token {
                Some(t) => api::fetch_notifications(&t).await,
                None => return,
            };
            items.set(Some(result));
        });
    });

    view! {
        <main>
            <div class="notif-header">
                <h1>"Notifications"</h1>
                <A href="/profile">
                    <Button appearance=ButtonAppearance::Secondary>"Notification Settings"</Button>
                </A>
            </div>

            {move || match items.get() {
                None => view! { <p>"Loading…"</p> }.into_any(),
                Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                Some(Ok(list)) => {
                    if list.is_empty() {
                        view! { <p class="mn-empty">"No notifications yet."</p> }.into_any()
                    } else {
                        view! {
                            <div>
                                {list.into_iter().map(|n| {
                                    let channel = n.channel.clone();
                                    view! {
                                        <Card>
                                            <div class="notif-row">
                                                <span class={format!("badge badge-{}", channel)}>
                                                    {channel_label(&channel)}
                                                </span>
                                                <span class="notif-time">{pretty_time(n.created_at)}</span>
                                            </div>
                                            <h3 class="notif-title">{n.title}</h3>
                                            <p class="notif-body">{n.body}</p>
                                        </Card>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_any()
                    }
                }
            }}
        </main>
    }
}
