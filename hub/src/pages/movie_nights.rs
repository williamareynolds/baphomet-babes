use auth_client::AuthUser;
use crate::api;
use leptos::prelude::*;
use leptos_router::components::A;
use thaw::{Button, ButtonAppearance, Card};

/// The movie-night section's landing: upcoming screenings. History and voting
/// live on their own pages (reached from the nav).
#[component]
pub fn MovieNightsPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
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
            <h1>"Movie Nights"</h1>
            <Show
                when=move || auth.get().is_some()
                fallback=move || view! {
                    <Card>
                        <p>
                            <A href="/login" attr:style="color:#ee4b61;">"Log in"</A>
                            " to see what's screening next."
                        </p>
                    </Card>
                }
            >
                <h2 class="section-heading">"Upcoming"</h2>
                {
                    let today = today.clone();
                    move || {
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
                    }
                }
            </Show>
        </main>
    }
}
