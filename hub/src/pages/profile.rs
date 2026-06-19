use crate::api;
use auth_client::{AuthUser, enable_push, notif_permission};
use leptos::prelude::*;
use shared::{Profile, ProfileLink, UpdateNotificationPrefs, UpdateProfileRequest};
use thaw::{Button, ButtonAppearance, ButtonType, Card, Field, Input, Switch, Textarea};

#[component]
pub fn ProfilePage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let profile: RwSignal<Option<Profile>> = RwSignal::new(None);
    let error: RwSignal<String> = RwSignal::new(String::new());
    let success: RwSignal<String> = RwSignal::new(String::new());
    let saving = RwSignal::new(false);

    // Form fields
    let display_name = RwSignal::new(String::new());
    let bio = RwSignal::new(String::new());
    let pronouns = RwSignal::new(String::new());
    let avatar_url = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let is_public = RwSignal::new(false);
    // Links: each row owns two stable signals so Thaw Inputs can bind to them
    // directly and survive add/remove re-renders without recreating state.
    let links: RwSignal<Vec<(RwSignal<String>, RwSignal<String>)>> = RwSignal::new(vec![]);

    // Load profile on mount
    Effect::new(move |_| {
        if let Some(user) = auth.get() {
            let token = user.token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::get_my_profile(&token).await {
                    Ok(p) => {
                        display_name.set(p.display_name.clone().unwrap_or_default());
                        bio.set(p.bio.clone().unwrap_or_default());
                        pronouns.set(p.pronouns.clone().unwrap_or_default());
                        avatar_url.set(p.avatar_url.clone().unwrap_or_default());
                        email.set(p.email.clone().unwrap_or_default());
                        is_public.set(p.is_public);
                        links.set(p.links.iter().map(|l| (RwSignal::new(l.label.clone()), RwSignal::new(l.url.clone()))).collect());
                        profile.set(Some(p));
                    }
                    Err(e) => error.set(e),
                }
            });
        }
    });

    // ---- Notification settings ----
    let perm = RwSignal::new(notif_permission());
    let ch_announce = RwSignal::new(true);
    let ch_general = RwSignal::new(true);
    let ch_movie = RwSignal::new(true);
    let ch_chat = RwSignal::new(true);
    let notif_msg = RwSignal::new(String::new());

    Effect::new(move |_| {
        if let Some(user) = auth.get() {
            let token = user.token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(p) = api::fetch_notif_prefs(&token).await {
                    ch_announce.set(p.announcements);
                    ch_general.set(p.general);
                    ch_movie.set(p.movie_night);
                    ch_chat.set(p.chat);
                }
            });
        }
    });

    let on_enable_push = move |_| {
        let Some(user) = auth.get() else { return };
        notif_msg.set(String::new());
        wasm_bindgen_futures::spawn_local(async move {
            match enable_push().await {
                Ok(tok) => {
                    let _ = api::register_push_token(&tok, &user.token).await;
                    crate::push::save(&tok);
                    notif_msg.set("Push enabled on this device.".into());
                }
                Err(e) => notif_msg.set(format!("Couldn't enable push: {e}")),
            }
            perm.set(notif_permission());
        });
    };

    let save_prefs = move |_| {
        let Some(user) = auth.get() else { return };
        notif_msg.set(String::new());
        let req = UpdateNotificationPrefs {
            announcements: Some(ch_announce.get()),
            general: Some(ch_general.get()),
            movie_night: Some(ch_movie.get()),
            chat: Some(ch_chat.get()),
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::update_notif_prefs(req, &user.token).await {
                Ok(_) => notif_msg.set("Notification settings saved.".into()),
                Err(e) => notif_msg.set(e),
            }
        });
    };

    let handle_save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if let Some(user) = auth.get() {
            error.set(String::new());
            success.set(String::new());
            saving.set(true);

            let req = UpdateProfileRequest {
                display_name: Some(display_name.get()).filter(|s| !s.is_empty()),
                bio: Some(bio.get()).filter(|s| !s.is_empty()),
                pronouns: Some(pronouns.get()).filter(|s| !s.is_empty()),
                avatar_url: Some(avatar_url.get()).filter(|s| !s.is_empty()),
                email: Some(email.get()).filter(|s| !s.is_empty()),
                links: Some(
                    links.get()
                        .into_iter()
                        .map(|(l, u)| (l.get(), u.get()))
                        .filter(|(l, u)| !l.is_empty() && !u.is_empty())
                        .map(|(label, url)| ProfileLink { label, url })
                        .collect()
                ),
                is_public: Some(is_public.get()),
            };

            let token = user.token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::update_my_profile(req, &token).await {
                    Ok(_) => { success.set("Profile saved.".into()); saving.set(false); }
                    Err(e) => { error.set(e); saving.set(false); }
                }
            });
        }
    };

    view! {
        <main>
            <h1>"My Profile"</h1>
            <Show when=move || auth.get().is_none()>
                <p class="error">"Login required."</p>
            </Show>
            <Show when=move || auth.get().is_some()>
                <Show when=move || !error.get().is_empty()>
                    <p class="error">{move || error.get()}</p>
                </Show>
                <Show when=move || !success.get().is_empty()>
                    <p class="success">{move || success.get()}</p>
                </Show>

                <form on:submit=handle_save>
                    <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem;">
                        <Switch checked=is_public label="Public profile" />
                        <span style="font-family:'IBM Plex Mono',monospace;font-size:0.6rem;color:#ad9ea4;">
                            {move || if is_public.get() { "visible in member directory" } else { "hidden from directory" }}
                        </span>
                    </div>

                    <Field label="Display Name">
                        <Input value=display_name placeholder="Leave blank to use username" />
                    </Field>
                    <Field label="Pronouns">
                        <Input value=pronouns placeholder="they/them, she/her, …" />
                    </Field>
                    <Field label="Email (shown on profile)">
                        <Input value=email placeholder="Optional — only shown if profile is public" />
                    </Field>
                    <Field label="Avatar URL">
                        <Input value=avatar_url placeholder="https://…" />
                    </Field>
                    <Field label="Bio">
                        <Textarea value=bio placeholder="A few words about yourself…" />
                    </Field>

                    <Field label="Links">
                        {move || links.get().into_iter().enumerate().map(|(i, (label_sig, url_sig))| {
                            view! {
                                <div style="display:flex;gap:0.5rem;margin-bottom:0.4rem;align-items:center;">
                                    <div style="flex:1;min-width:0;"><Input value=label_sig placeholder="Label" /></div>
                                    <div style="flex:2;min-width:0;"><Input value=url_sig placeholder="https://…" /></div>
                                    <Button
                                        appearance=ButtonAppearance::Secondary
                                        on_click=move |_| links.update(|ls| { if i < ls.len() { ls.remove(i); } })
                                    >"×"</Button>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                        <div style="margin-top:0.5rem;">
                            <Button
                                appearance=ButtonAppearance::Secondary
                                on_click=move |_| links.update(|ls| ls.push((RwSignal::new(String::new()), RwSignal::new(String::new()))))
                            >"+ Add Link"</Button>
                        </div>
                    </Field>

                    <Button
                        button_type=ButtonType::Submit
                        appearance=ButtonAppearance::Primary
                        loading=saving
                        disabled=saving
                    >
                        {move || if saving.get() { "Saving…" } else { "Save Profile" }}
                    </Button>
                </form>

                <Card>
                    <h2 id="notifications">"Notifications"</h2>
                    <Show
                        when=move || perm.get() == "granted"
                        fallback=move || view! {
                            <p style="color:#bdafb2;margin-bottom:0.75rem;">
                                "Turn on push notifications for this device."
                            </p>
                            <Button appearance=ButtonAppearance::Primary on_click=on_enable_push>
                                "Enable Push"
                            </Button>
                        }
                    >
                        <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;color:#93d8b4;margin-bottom:0.75rem;">
                            "Push enabled on this device."
                        </p>
                    </Show>

                    <p style="font-family:'IBM Plex Mono',monospace;font-size:0.6rem;letter-spacing:0.1em;text-transform:uppercase;color:#ad9ea4;margin:1.25rem 0 0.5rem;">
                        "Channels"
                    </p>
                    <div style="display:flex;flex-direction:column;gap:0.6rem;">
                        <Switch checked=ch_announce label="Announcements" />
                        <Switch checked=ch_general label="General" />
                        <Switch checked=ch_movie label="Movie Nights" />
                        <Switch checked=ch_chat label="Group Chat" />
                    </div>

                    <Show when=move || !notif_msg.get().is_empty()>
                        <p class="success" style="margin-top:0.75rem;">{move || notif_msg.get()}</p>
                    </Show>

                    <div style="margin-top:1rem;">
                        <Button appearance=ButtonAppearance::Primary on_click=save_prefs>
                            "Save Notification Settings"
                        </Button>
                    </div>
                </Card>
            </Show>
        </main>
    }
}
