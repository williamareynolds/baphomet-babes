use auth_client::{AuthUser, load_identity};
use crate::api;
use leptos::prelude::*;
use leptos_router::components::A;
use thaw::Card;

/// Landing page: the community hub. Shows the brand hero and a feed of
/// announcements (newest first), each with an optional embedded poll. Gated to
/// logged-in members like the rest of the site; logged-out visitors get a
/// welcome + login prompt.
#[component]
pub fn AnnouncementsPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let identity = load_identity();

    let announcements: RwSignal<Option<Result<Vec<shared::Announcement>, String>>> =
        RwSignal::new(None);

    Effect::new(move |_| {
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(t) = token else { return };
            // Serve the last-seen feed if the network's down (offline bar signals
            // the staleness); only surface an error when there's nothing stashed.
            let result = match api::fetch_announcements(&t).await {
                Ok(list) => {
                    crate::cache::stash("announcements", &list);
                    Ok(list)
                }
                Err(e) => crate::cache::recall::<Vec<shared::Announcement>>("announcements")
                    .map(Ok)
                    .unwrap_or(Err(e)),
            };
            announcements.set(Some(result));
        });
    });

    view! {
        <main>
            <div style="margin-bottom:3.5rem;">
                <h1 style="font-size:clamp(2.75rem,14vw,5rem);line-height:1;margin-bottom:0.15rem;">"Baphomet Babes"</h1>
                <p style="font-family:'IBM Plex Mono',monospace;font-size:0.75rem;letter-spacing:0.22em;text-transform:uppercase;color:#ee4b61;margin-bottom:1.5rem;">
                    "of Bentonville"
                </p>
                <p style="font-size:1.25rem;line-height:1.7;color:#bdafb2;max-width:min(560px,100%);">
                    "An inclusive collective for curious minds and bold spirits. "
                    "Cultural events, scientific discussions, crafts, sports, and more."
                </p>
            </div>

            <Show
                when=move || auth.get().is_some()
                fallback=move || {
                    let identity = identity.clone();
                    view! {
                        {match &identity {
                            Some(id) => {
                                let username = id.username.clone();
                                view! {
                                    <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;color:#95868f;margin-bottom:2rem;">
                                        "Welcome back, " {username} ". "
                                        <A href="/login" attr:style="color:#ee4b61;">"Log in"</A>
                                        " to see the latest."
                                    </p>
                                }.into_any()
                            }
                            None => view! {
                                <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;color:#95868f;margin-bottom:2rem;">
                                    <A href="/login" attr:style="color:#ee4b61;">"Log in"</A>
                                    " to see the latest."
                                </p>
                            }.into_any()
                        }}
                    }
                }
            >
                <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;color:#ad9ea4;margin-bottom:1rem;">
                    "Welcome back, "
                    {move || auth.get().map(|u| u.username).unwrap_or_default()}
                    "."
                </p>
                <h2 class="section-heading">"Announcements"</h2>
                {move || match announcements.get() {
                    None => view! { <p>"Loading..."</p> }.into_any(),
                    Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                    Some(Ok(list)) => {
                        if list.is_empty() {
                            view! { <p>"Nothing posted yet. Check back soon!"</p> }.into_any()
                        } else {
                            view! {
                                <div>
                                    {list.into_iter().map(|a| view! {
                                        <Card>
                                            <h2 style="margin-bottom:0.5rem;">{a.title}</h2>
                                            <p style="white-space:pre-wrap;">{a.body}</p>
                                            {a.poll_embed_url.map(|url| view! {
                                                <div style="margin-top:1rem;">
                                                    <iframe src={url} class="poll-frame" />
                                                </div>
                                            })}
                                        </Card>
                                    }).collect::<Vec<_>>()}
                                </div>
                            }.into_any()
                        }
                    }
                }}
            </Show>

            <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;letter-spacing:0.1em;color:#95868f;border-top:1px solid #1e1526;padding-top:1.5rem;margin-top:3rem;">
                "All are welcome. No exceptions."
            </p>
        </main>
    }
}
