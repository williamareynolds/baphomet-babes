use auth_client::AuthUser;
use crate::api;
use crate::components::admin_nav::AdminNav;
use leptos::prelude::*;
use shared::CreateExternalCalendarRequest;
use thaw::{Button, ButtonAppearance, ButtonType, Card, Field, Input};

/// Copy text to the clipboard (best effort; silently no-ops if unavailable).
fn copy_to_clipboard(text: &str) {
    if let Some(win) = web_sys::window() {
        let _ = win.navigator().clipboard().write_text(text);
    }
}

/// "2026-08-18" from unix seconds, for the issued-on column. Dates only — the
/// exact minute a link was cut has never been the interesting part.
fn issued_on(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Calendar links for people without accounts — a partner, a regular guest.
///
/// Superadmin only, and deliberately so: the link is a bearer credential for the
/// whole schedule, and the page holds the name and phone number of someone who
/// never signed up. Revoking deletes the record outright, so the URL dies and we
/// stop holding their details.
#[component]
pub fn AdminCalendarLinksPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let is_superadmin = move || auth.get().map(|u| u.is_superadmin()).unwrap_or(false);

    let (refresh, set_refresh) = signal(0u32);
    let links: RwSignal<Option<Result<Vec<shared::ExternalCalendarLink>, String>>> =
        RwSignal::new(None);

    Effect::new(move |_| {
        let _ = refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let result = match token {
                None => return,
                Some(t) => api::fetch_external_calendars(&t).await,
            };
            links.set(Some(result));
        });
    });

    let name = RwSignal::new(String::new());
    let phone = RwSignal::new(String::new());
    let (error, set_error) = signal(String::new());
    let (success, set_success) = signal(String::new());
    // Which link's copy button was last tapped, for "Copied!" feedback.
    let copied_id = RwSignal::new(String::new());

    let handle_create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_error.set(String::new());
        set_success.set(String::new());
        let Some(user) = auth.get() else { return };

        let req = CreateExternalCalendarRequest {
            name: name.get().trim().to_string(),
            phone: phone.get().trim().to_string(),
        };
        // Check before the round trip so the message is instant; the backend
        // validates the same rules regardless.
        if let Err(e) = shared::validate_external_calendar(&req.name, &req.phone) {
            set_error.set(e);
            return;
        }

        wasm_bindgen_futures::spawn_local(async move {
            match api::create_external_calendar(req, &user.token).await {
                Ok(link) => {
                    // Copy the feed URL straight away — it's the whole point of
                    // making one, and saves a second tap.
                    copy_to_clipboard(&api::calendar_feed_url(&link.token));
                    set_success.set(format!("Calendar link for {} created and copied.", link.name));
                    name.set(String::new());
                    phone.set(String::new());
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_error.set(e),
            }
        });
    };

    let handle_revoke = move |id: String, who: String| {
        let confirmed = web_sys::window()
            .and_then(|w| {
                w.confirm_with_message(&format!(
                    "Revoke {who}'s calendar link? Their calendar stops updating and the record is deleted."
                ))
                .ok()
            })
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        let Some(user) = auth.get() else { return };
        set_error.set(String::new());
        set_success.set(String::new());
        wasm_bindgen_futures::spawn_local(async move {
            match api::revoke_external_calendar(&id, &user.token).await {
                Ok(()) => {
                    set_success.set("Link revoked.".into());
                    set_refresh.update(|n| *n += 1);
                }
                Err(e) => set_error.set(e),
            }
        });
    };

    view! {
        <main>
            <Show
                when=is_superadmin
                fallback=|| view! { <p class="error">"Access denied."</p> }
            >
                <h1>"Admin"</h1>
                <AdminNav active="calendar-links" is_superadmin=true />

                <Card>
                    <h2>"New calendar link"</h2>
                    <p class="muted" style="margin-top:0;">
                        "For people without an account. They can subscribe to the schedule in any \
                         calendar app — anyone holding the link can read it, so hand it out the \
                         way you'd hand out a key."
                    </p>
                    <form on:submit=handle_create>
                        <Field label="Name">
                            <Input value=name placeholder="Who is this for?" />
                        </Field>
                        <Field label="Phone">
                            <Input value=phone placeholder="555-123-4567" />
                        </Field>
                        <Show when=move || !error.get().is_empty()>
                            <p class="error">{move || error.get()}</p>
                        </Show>
                        <Show when=move || !success.get().is_empty()>
                            <p class="success" style="font-size:1.05rem;">{move || success.get()}</p>
                        </Show>
                        <Button button_type=ButtonType::Submit appearance=ButtonAppearance::Primary>
                            "Create link"
                        </Button>
                    </form>
                </Card>

                <h2 class="section-heading" style="margin-top:2rem;">"Issued links"</h2>
                {move || match links.get() {
                    None => view! { <p>"Loading..."</p> }.into_any(),
                    Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                    Some(Ok(list)) if list.is_empty() => view! {
                        <p class="muted">"No calendar links issued yet."</p>
                    }.into_any(),
                    Some(Ok(list)) => view! {
                        // Wrapper class so the listing can be addressed on its
                        // own: the create form's success message quotes the
                        // guest's name, so a bare card selector matches both.
                        <div class="calendar-link-list">
                            {list.into_iter().map(|l| {
                                let id = l.id.clone();
                                let who = l.name.clone();
                                let url = api::calendar_feed_url(&l.token);
                                let copy_id = id.clone();
                                let copy_url = url.clone();
                                let on_copy = move |_| {
                                    copy_to_clipboard(&copy_url);
                                    copied_id.set(copy_id.clone());
                                };
                                let this_id = id.clone();
                                let copy_label = move || {
                                    if copied_id.get() == this_id { "Copied!" } else { "Copy link" }
                                };
                                let revoke_id = id.clone();
                                let revoke_who = who.clone();
                                view! {
                                    <Card>
                                        <div class="admin-row">
                                            <div>
                                                <strong>{l.name.clone()}</strong>
                                                <p style="margin-top:0.35rem;color:#d8cdcf;">{l.phone.clone()}</p>
                                                <p style="color:#bdafb2;font-size:0.8rem;">
                                                    {format!("issued {} by {}", issued_on(l.created_at), l.created_by)}
                                                </p>
                                            </div>
                                            <div class="admin-actions">
                                                <Button
                                                    appearance=ButtonAppearance::Primary
                                                    on_click=on_copy
                                                >{copy_label}</Button>
                                                <Button
                                                    appearance=ButtonAppearance::Secondary
                                                    on_click=move |_| handle_revoke(revoke_id.clone(), revoke_who.clone())
                                                >"Revoke"</Button>
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
