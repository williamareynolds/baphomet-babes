use crate::api;
use auth_client::AuthUser;
use leptos::prelude::*;
use leptos_router::components::A;
use shared::Profile;
use thaw::{Button, ButtonAppearance, Card};

#[component]
pub fn MembersPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let members: RwSignal<Vec<Profile>> = RwSignal::new(vec![]);
    let error: RwSignal<String> = RwSignal::new(String::new());
    let loading = RwSignal::new(true);

    Effect::new(move |_| {
        if let Some(user) = auth.get() {
            let token = user.token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::list_members(&token).await {
                    Ok(data) => { members.set(data); loading.set(false); }
                    Err(e) => { error.set(e); loading.set(false); }
                }
            });
        }
    });

    view! {
        <main>
            <h1>"Members"</h1>
            <Show when=move || auth.get().is_none()>
                <p class="error">"Login required."</p>
            </Show>
            <Show when=move || loading.get() && auth.get().is_some()>
                <p style="color:#4a3a5a;font-family:'IBM Plex Mono',monospace;font-size:0.75rem;">"Loading..."</p>
            </Show>
            <Show when=move || !error.get().is_empty()>
                <p class="error">{move || error.get()}</p>
            </Show>
            <Show when=move || !loading.get() && members.get().is_empty() && error.get().is_empty()>
                <p style="color:#6a5a6a;">"No public member profiles yet."</p>
            </Show>
            <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(260px,1fr));gap:1rem;">
                {move || members.get().into_iter().map(|m| {
                    let id = m.user_id.clone();
                    view! {
                        <A href={format!("/members/{id}")} attr:style="text-decoration:none;">
                            <Card>
                                <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:0.75rem;">
                                    {if let Some(av) = m.avatar_url {
                                        view! {
                                            <img src={av} style="width:40px;height:40px;border-radius:50%;object-fit:cover;" />
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div style="width:40px;height:40px;border-radius:50%;background:#1e1526;display:flex;align-items:center;justify-content:center;font-family:'Bebas Neue',sans-serif;font-size:1.2rem;color:#4a3a6a;">
                                                {m.username.chars().next().unwrap_or('?').to_uppercase().to_string()}
                                            </div>
                                        }.into_any()
                                    }}
                                    <div>
                                        <div style="font-family:'Bebas Neue',sans-serif;font-size:1.1rem;color:#e2d8d0;letter-spacing:0.05em;">
                                            {m.display_name.unwrap_or_else(|| m.username.clone())}
                                        </div>
                                        {m.pronouns.map(|p| view! {
                                            <div style="font-family:'IBM Plex Mono',monospace;font-size:0.6rem;color:#4a3a5a;letter-spacing:0.1em;">{p}</div>
                                        })}
                                    </div>
                                </div>
                                {m.bio.map(|b| {
                                    let preview = if b.len() > 100 { format!("{}…", &b[..100]) } else { b };
                                    view! {
                                        <p style="color:#6a5a6a;font-size:0.95rem;line-height:1.5;">{preview}</p>
                                    }
                                })}
                            </Card>
                        </A>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </main>
    }
}

#[component]
pub fn MemberProfilePage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    use leptos_router::hooks::use_params_map;

    let params = use_params_map();
    let profile: RwSignal<Option<Profile>> = RwSignal::new(None);
    let error: RwSignal<String> = RwSignal::new(String::new());

    Effect::new(move |_| {
        let id = params.read().get("id").unwrap_or_default();
        if let Some(user) = auth.get() {
            let token = user.token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api::get_member(&id, &token).await {
                    Ok(p) => profile.set(Some(p)),
                    Err(e) => error.set(e),
                }
            });
        }
    });

    view! {
        <main>
            <Show when=move || !error.get().is_empty()>
                <p class="error">{move || error.get()}</p>
            </Show>
            <Show when=move || profile.get().is_none() && error.get().is_empty()>
                <p style="color:#4a3a5a;font-family:'IBM Plex Mono',monospace;font-size:0.75rem;">"Loading..."</p>
            </Show>
            {move || profile.get().map(|p| {
                let own_id = auth.get().map(|u| u.id).unwrap_or_default();
                let is_own = own_id == p.user_id;
                view! {
                    <div style="max-width:640px;">
                        <div style="display:flex;align-items:flex-start;gap:1.5rem;margin-bottom:2rem;">
                            {if let Some(av) = p.avatar_url.clone() {
                                view! {
                                    <img src={av} style="width:72px;height:72px;border-radius:50%;object-fit:cover;flex-shrink:0;" />
                                }.into_any()
                            } else {
                                view! {
                                    <div style="width:72px;height:72px;border-radius:50%;background:#1e1526;display:flex;align-items:center;justify-content:center;font-family:'Bebas Neue',sans-serif;font-size:2rem;color:#4a3a6a;flex-shrink:0;">
                                        {p.username.chars().next().unwrap_or('?').to_uppercase().to_string()}
                                    </div>
                                }.into_any()
                            }}
                            <div>
                                <h1 style="font-size:2.5rem;margin-bottom:0.1rem;">
                                    {p.display_name.clone().unwrap_or_else(|| p.username.clone())}
                                </h1>
                                {p.pronouns.clone().map(|pr| view! {
                                    <p style="font-family:'IBM Plex Mono',monospace;font-size:0.65rem;letter-spacing:0.12em;color:#4a3a5a;text-transform:uppercase;">{pr}</p>
                                })}
                                {p.email.clone().map(|e| view! {
                                    <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;color:#6a5a6a;margin-top:0.25rem;">{e}</p>
                                })}
                            </div>
                        </div>

                        {p.bio.clone().map(|b| view! {
                            <p style="font-size:1.15rem;line-height:1.75;color:#9a8a8a;margin-bottom:1.75rem;">{b}</p>
                        })}

                        {if !p.links.is_empty() {
                            Some(view! {
                                <div style="display:flex;flex-wrap:wrap;gap:0.5rem;margin-bottom:1.75rem;">
                                    {p.links.iter().map(|l| view! {
                                        <a href={l.url.clone()} target="_blank" rel="noopener noreferrer"
                                           style="font-family:'IBM Plex Mono',monospace;font-size:0.65rem;letter-spacing:0.1em;text-transform:uppercase;color:#c41e3a;border:1px solid rgba(196,30,58,0.3);padding:4px 10px;border-radius:2px;text-decoration:none;">
                                            {l.label.clone()}
                                        </a>
                                    }).collect::<Vec<_>>()}
                                </div>
                            })
                        } else { None }}

                        {if is_own {
                            Some(view! {
                                <A href="/profile">
                                    <Button appearance=ButtonAppearance::Secondary>"Edit Profile"</Button>
                                </A>
                            })
                        } else { None }}
                    </div>
                }
            })}
        </main>
    }
}
