use crate::api;
use auth_client::AuthUser;
use leptos::prelude::*;
use shared::{Profile, ProfileLink, UpdateProfileRequest};

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
    // Links: Vec<(label, url)>
    let links: RwSignal<Vec<(String, String)>> = RwSignal::new(vec![]);

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
                        links.set(p.links.iter().map(|l| (l.label.clone(), l.url.clone())).collect());
                        profile.set(Some(p));
                    }
                    Err(e) => error.set(e),
                }
            });
        }
    });

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
                        <label style="margin:0;cursor:pointer;">
                            <input type="checkbox"
                                prop:checked=is_public
                                on:change=move |e| {
                                    use wasm_bindgen::JsCast;
                                    let checked = e.target()
                                        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                        .map(|el| el.checked())
                                        .unwrap_or(false);
                                    is_public.set(checked);
                                }
                                style="margin-right:0.5rem;"
                            />
                            <span style="font-family:'IBM Plex Mono',monospace;font-size:0.65rem;letter-spacing:0.1em;text-transform:uppercase;color:#5a4a5a;">
                                "Public profile"
                            </span>
                        </label>
                        <span style="font-family:'IBM Plex Mono',monospace;font-size:0.6rem;color:#3a2a3a;">
                            {move || if is_public.get() { "visible in member directory" } else { "hidden from directory" }}
                        </span>
                    </div>

                    <label>"Display Name"</label>
                    <input type="text" placeholder="Leave blank to use username"
                        prop:value=display_name
                        on:input=move |e| display_name.set(event_target_value(&e)) />

                    <label>"Pronouns"</label>
                    <input type="text" placeholder="they/them, she/her, …"
                        prop:value=pronouns
                        on:input=move |e| pronouns.set(event_target_value(&e)) />

                    <label>"Email (shown on profile)"</label>
                    <input type="text" placeholder="Optional — only shown if profile is public"
                        prop:value=email
                        on:input=move |e| email.set(event_target_value(&e)) />

                    <label>"Avatar URL"</label>
                    <input type="text" placeholder="https://…"
                        prop:value=avatar_url
                        on:input=move |e| avatar_url.set(event_target_value(&e)) />

                    <label>"Bio"</label>
                    <textarea rows="4" placeholder="A few words about yourself…"
                        prop:value=bio
                        on:input=move |e| bio.set(event_target_value(&e))
                        style="resize:vertical;" />

                    <div style="margin-bottom:0.875rem;">
                        <label>"Links"</label>
                        {move || links.get().into_iter().enumerate().map(|(i, (label, url))| {
                            view! {
                                <div style="display:flex;gap:0.5rem;margin-bottom:0.4rem;">
                                    <input type="text" placeholder="Label"
                                        prop:value={label.clone()}
                                        on:input=move |e| {
                                            let val = event_target_value(&e);
                                            links.update(|ls| { if let Some(l) = ls.get_mut(i) { l.0 = val; } });
                                        }
                                        style="flex:1;margin:0;" />
                                    <input type="text" placeholder="https://…"
                                        prop:value={url.clone()}
                                        on:input=move |e| {
                                            let val = event_target_value(&e);
                                            links.update(|ls| { if let Some(l) = ls.get_mut(i) { l.1 = val; } });
                                        }
                                        style="flex:2;margin:0;" />
                                    <button type="button" class="secondary"
                                        on:click=move |_| {
                                            links.update(|ls| { ls.remove(i); });
                                        }
                                        style="padding:0.4rem 0.6rem;flex-shrink:0;">"×"</button>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                        <button type="button" class="secondary"
                            on:click=move |_| links.update(|ls| ls.push((String::new(), String::new())))
                            style="margin-top:0.25rem;">
                            "+ Add Link"
                        </button>
                    </div>

                    <button type="submit" disabled=saving>
                        {move || if saving.get() { "Saving…" } else { "Save Profile" }}
                    </button>
                </form>
            </Show>
        </main>
    }
}
