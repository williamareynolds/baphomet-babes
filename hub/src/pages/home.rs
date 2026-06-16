use auth_client::{AuthUser, load_identity};
use crate::api;
use leptos::prelude::*;
use leptos_router::components::A;
use thaw::{Button, ButtonAppearance, Card};

#[component]
pub fn HomePage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let identity = load_identity();

    let events: RwSignal<Option<Result<Vec<shared::Event>, String>>> = RwSignal::new(None);

    Effect::new(move |_| {
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let result = match token {
                Some(t) => api::fetch_events(&t).await,
                None => return,
            };
            events.set(Some(result));
        });
    });

    let today = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default()
        .chars()
        .take(10)
        .collect::<String>();

    view! {
        <main>
            <div style="margin-bottom:3.5rem;">
                <h1 style="font-size:5rem;line-height:1;margin-bottom:0.15rem;">"Baphomet Babes"</h1>
                <p style="font-family:'IBM Plex Mono',monospace;font-size:0.75rem;letter-spacing:0.22em;text-transform:uppercase;color:#ee4b61;margin-bottom:1.5rem;">
                    "of Bentonville"
                </p>
                <p style="font-size:1.25rem;line-height:1.7;color:#bdafb2;max-width:560px;">
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
                                        " to see what's screening next."
                                    </p>
                                }.into_any()
                            }
                            None => view! {
                                <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;color:#95868f;margin-bottom:2rem;">
                                    <A href="/login" attr:style="color:#ee4b61;">"Log in"</A>
                                    " to see what's screening next."
                                </p>
                            }.into_any()
                        }}
                    }
                }
            >
                {
                    let today = today.clone();
                    view! {
                        <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;color:#ad9ea4;margin-bottom:1rem;">
                            "Welcome back, "
                            {move || auth.get().map(|u| u.username).unwrap_or_default()}
                            "."
                        </p>
                        <h2 class="section-heading">"Upcoming Movie Nights"</h2>
                        {move || {
                            let today = today.clone();
                            match events.get() {
                                None => view! { <p>"Loading..."</p> }.into_any(),
                                Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                                Some(Ok(list)) => {
                                    let upcoming: Vec<_> = list.into_iter()
                                        .filter(|e| e.date >= today)
                                        .collect();
                                    if upcoming.is_empty() {
                                        view! { <p>"No upcoming events. Check back soon!"</p> }.into_any()
                                    } else {
                                        view! {
                                            <div>
                                                {upcoming.into_iter().map(|e| view! {
                                                    <Card>
                                                        <span class={format!("badge badge-{}", e.event_type)}>
                                                            {if e.event_type == "main" { "Main Event" } else { "Special Feature" }}
                                                        </span>
                                                        {e.poster_url.map(|url| view! {
                                                            <div class="poster-wrap">
                                                                <img src={url} alt="movie poster" class="poster" />
                                                            </div>
                                                        })}
                                                        <h2 style="margin-top:0.5rem;">{e.title}</h2>
                                                        <p class="event-date">{e.date}</p>
                                                        {e.description.map(|d| view! { <p style="margin-top:0.5rem;">{d}</p> })}
                                                        {e.poll_embed_url.map(|_| view! {
                                                            <div style="margin-top:0.75rem;">
                                                                <A href="/vote">
                                                                    <Button appearance=ButtonAppearance::Primary>"Vote on Date →"</Button>
                                                                </A>
                                                            </div>
                                                        })}
                                                    </Card>
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }.into_any()
                                    }
                                }
                            }
                        }}
                    }
                }
            </Show>

            <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;letter-spacing:0.1em;color:#95868f;border-top:1px solid #1e1526;padding-top:1.5rem;margin-top:3rem;">
                "All are welcome. No exceptions."
            </p>
        </main>
    }
}
