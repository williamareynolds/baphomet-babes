use crate::api;
use auth_client::AuthUser;
use leptos::prelude::*;
use leptos_router::components::A;
use std::time::Duration;
use thaw::{Button, ButtonAppearance, Switch};

/// Format a unix-seconds timestamp using the browser's locale (time only).
fn pretty_time(secs: i64) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64((secs as f64) * 1000.0));
    d.to_locale_time_string("default")
        .as_string()
        .unwrap_or_default()
}

/// Whole-group chat room. One shared feed, polled every few seconds. The author
/// can mute the chat notification channel right here (mirrors the profile
/// setting) without leaving the conversation.
#[component]
pub fn ChatPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let me = auth.get().map(|u| u.id).unwrap_or_default();

    let messages: RwSignal<Vec<shared::ChatMessage>> = RwSignal::new(vec![]);
    let loaded = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let draft = RwSignal::new(String::new());
    let sending = RwSignal::new(false);

    // Notification toggle for the chat channel. `pref_ready` arms the save
    // Effect after the initial load, and `last_saved` suppresses redundant
    // writes (so visiting the page doesn't re-PUT the unchanged value).
    let notify = RwSignal::new(false);
    let pref_ready = RwSignal::new(false);
    let last_saved: RwSignal<Option<bool>> = RwSignal::new(None);

    let scroller: NodeRef<leptos::html::Div> = NodeRef::new();
    let scroll_to_bottom = move || {
        if let Some(el) = scroller.get_untracked() {
            el.set_scroll_top(el.scroll_height());
        }
    };

    // Poll the feed: an interval bumps `tick`, and the fetch Effect depends on it.
    let (tick, set_tick) = signal(0u32);
    let handle = set_interval_with_handle(
        move || set_tick.update(|n| *n = n.wrapping_add(1)),
        Duration::from_secs(5),
    )
    .ok();
    on_cleanup(move || {
        if let Some(h) = handle {
            h.clear();
        }
    });

    Effect::new(move |_| {
        let _ = tick.get();
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            match api::fetch_chat(&user.token).await {
                Ok(list) => {
                    let grew = list.len() != messages.get_untracked().len();
                    messages.set(list);
                    error.set(String::new());
                    loaded.set(true);
                    if grew {
                        // Let the DOM paint the new rows before scrolling.
                        request_animation_frame(move || scroll_to_bottom());
                    }
                }
                Err(e) => {
                    if !loaded.get_untracked() {
                        error.set(e);
                    }
                }
            }
        });
    });

    // Load the chat notification preference once.
    Effect::new(move |_| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(p) = api::fetch_notif_prefs(&user.token).await {
                notify.set(p.chat);
                last_saved.set(Some(p.chat));
            }
            pref_ready.set(true);
        });
    });

    // Persist the toggle whenever the user flips it (after the initial load).
    Effect::new(move |_| {
        let on = notify.get();
        if !pref_ready.get() || last_saved.get_untracked() == Some(on) {
            return;
        }
        last_saved.set(Some(on));
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            let req = shared::UpdateNotificationPrefs { chat: Some(on), ..Default::default() };
            let _ = api::update_notif_prefs(req, &user.token).await;
        });
    });

    let send = move || {
        let body = draft.get();
        let body = body.trim().to_string();
        if body.is_empty() || sending.get() {
            return;
        }
        let Some(user) = auth.get() else { return };
        sending.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            match api::send_chat(&body, &user.token).await {
                Ok(msg) => {
                    messages.update(|m| m.push(msg));
                    draft.set(String::new());
                    request_animation_frame(move || scroll_to_bottom());
                }
                Err(e) => error.set(e),
            }
            sending.set(false);
        });
    };

    let on_keydown = move |ev: leptos::ev::KeyboardEvent| {
        // Enter sends; Shift+Enter inserts a newline.
        if ev.key() == "Enter" && !ev.shift_key() {
            ev.prevent_default();
            send();
        }
    };

    view! {
        <main>
            <div class="chat-header">
                <h1>"Group Chat"</h1>
                <div class="chat-tools">
                    <Switch checked=notify label="Notify" />
                    <A href="/profile">
                        <Button appearance=ButtonAppearance::Secondary>"Settings"</Button>
                    </A>
                </div>
            </div>

            <Show when=move || !error.get().is_empty()>
                <p class="error">{move || error.get()}</p>
            </Show>

            <div class="chat-feed" node_ref=scroller>
                {move || {
                    if !loaded.get() {
                        return view! { <p class="chat-empty">"Loading…"</p> }.into_any();
                    }
                    let list = messages.get();
                    if list.is_empty() {
                        return view! { <p class="chat-empty">"No messages yet. Say hello!"</p> }.into_any();
                    }
                    let me = me.clone();
                    view! {
                        <div class="chat-messages">
                            {list.into_iter().map(|m| {
                                let mine = m.user_id == me;
                                let cls = if mine { "chat-msg mine" } else { "chat-msg" };
                                view! {
                                    <div class=cls>
                                        <div class="chat-meta">
                                            <span class="chat-author">{m.author}</span>
                                            <span class="chat-time">{pretty_time(m.created_at)}</span>
                                        </div>
                                        <div class="chat-bubble">{m.body}</div>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }.into_any()
                }}
            </div>

            <div class="chat-composer">
                <textarea
                    class="chat-input"
                    rows="1"
                    placeholder="Message the group…"
                    prop:value=move || draft.get()
                    on:input=move |ev| draft.set(event_target_value(&ev))
                    on:keydown=on_keydown
                ></textarea>
                <Button
                    appearance=ButtonAppearance::Primary
                    loading=sending
                    disabled=sending
                    on_click=move |_| send()
                >
                    "Send"
                </Button>
            </div>
        </main>
    }
}
