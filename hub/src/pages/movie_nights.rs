use auth_client::AuthUser;
use crate::api;
use crate::components::calendar_subscribe::CalendarSubscribe;
use crate::components::event_rsvp::EventRsvp;
use leptos::prelude::*;
use leptos_router::components::A;
use thaw::{Button, ButtonAppearance, Card};

const PER_PAGE: usize = 10;

/// Render "2030-10-31" as "October 31, 2030". Falls back to the raw string if
/// the shape isn't what we expect (we never trust stored data to be clean).
fn pretty_date(d: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    let parts: Vec<&str> = d.split('-').collect();
    match (parts.first(), parts.get(1), parts.get(2)) {
        (Some(y), Some(m), Some(day)) => {
            match m.parse::<usize>() {
                Ok(mi) if (1..=12).contains(&mi) => {
                    let day = day.trim_start_matches('0');
                    format!("{} {}, {}", MONTHS[mi - 1], day, y)
                }
                _ => d.to_string(),
            }
        }
        _ => d.to_string(),
    }
}

/// Pick the featured "next" screening and return
/// the full screening list in reverse-chronological order. The featured event is
/// the soonest dated screening today-or-later; if none is dated yet, fall back to
/// a planned (undated) pick — preferring one with an open poll — so an event being
/// voted on still headlines as "Date TBD".
fn split_events(mut list: Vec<shared::Event>, today: &str) -> (Option<shared::Event>, Vec<shared::Event>) {
    list.sort_by(|a, b| a.date.cmp(&b.date));
    let featured = list
        .iter()
        .find(|e| e.date.as_deref().is_some_and(|d| d >= today))
        .or_else(|| list.iter().find(|e| e.date.is_none() && e.poll_embed_url.is_some()))
        .or_else(|| list.iter().find(|e| e.date.is_none()))
        .cloned();
    list.sort_by(|a, b| b.date.cmp(&a.date));
    (featured, list)
}

#[component]
pub fn MovieNightsPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let events: RwSignal<Option<Result<Vec<shared::Event>, String>>> = RwSignal::new(None);
    let page = RwSignal::new(0usize);

    Effect::new(move |_| {
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(t) = token else { return };
            // Serve the last-seen schedule if the network's down (offline bar
            // signals staleness); only error when there's nothing stashed.
            let result = match api::fetch_events(&t).await {
                Ok(list) => {
                    crate::cache::stash("events", &list);
                    Ok(list)
                }
                Err(e) => crate::cache::recall::<Vec<shared::Event>>("events")
                    .map(Ok)
                    .unwrap_or(Err(e)),
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

    // Page count for the archive, used by the pagination handlers.
    let total_pages = {
        let today = today.clone();
        Signal::derive(move || match events.get() {
            Some(Ok(list)) => {
                let (_, rest) = split_events(list, &today);
                rest.len().div_ceil(PER_PAGE).max(1)
            }
            _ => 1,
        })
    };

    let go_prev = move |_| page.update(|p| *p = p.saturating_sub(1));
    let go_next = move |_| {
        let total = total_pages.get();
        page.update(|p| if *p + 1 < total { *p += 1; });
    };
    // Named so the `>` doesn't get parsed as a tag close inside the view macro.
    let multi_page = move || total_pages.get() > 1;

    let archive = {
        let today = today.clone();
        move || {
            let today = today.clone();
            match events.get() {
                None => view! { <p>"Loading…"</p> }.into_any(),
                Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                Some(Ok(list)) => {
                    let (featured, rest) = split_events(list, &today);
                    let total = rest.len().div_ceil(PER_PAGE).max(1);
                    let cur = page.get().min(total - 1);
                    let slice: Vec<_> = rest
                        .into_iter()
                        .skip(cur * PER_PAGE)
                        .take(PER_PAGE)
                        .collect();

                    view! {
                        // ---- Next feature (marquee hero) ----
                        {match featured {
                            None => view! {
                                <Card>
                                    <p class="kicker">"Next Feature"</p>
                                    <p>"No screening on the calendar yet. Check back soon."</p>
                                </Card>
                            }.into_any(),
                            Some(f) => {
                                let poster = f.poster_url.clone();
                                let rsvp_event = f.clone();
                                // Voting is to pick a date — once one is set, it's over.
                                let voting_open = f.poll_embed_url.is_some() && f.date.is_none();
                                // Until the poll picks a date there's nothing to RSVP to,
                                // so a "3 going" count would be meaningless — hide it.
                                let dated = f.date.is_some();
                                let date_label = f.date.clone().map(|d| pretty_date(&d))
                                    .unwrap_or_else(|| if voting_open {
                                        "Voting open — help pick the date".to_string()
                                    } else {
                                        "Date TBD".to_string()
                                    });
                                view! {
                                    <div class="next-feature">
                                        {poster.map(|url| view! {
                                            <div class="feature-poster-wrap">
                                                <img src={url} alt="movie poster" class="feature-poster" />
                                            </div>
                                        })}
                                        <div class="feature-body">
                                            <p class="kicker">"Next Feature"</p>
                                            <span class={format!("badge badge-{}", f.event_type)}>
                                                {if f.event_type == "main" { "Featured Film" } else { "Special Feature" }}
                                            </span>
                                            <h2 class="feature-title">{f.title}</h2>
                                            <p class="feature-date">{date_label}</p>
                                            {f.description.map(|d| view! {
                                                <p class="feature-desc">{d}</p>
                                            })}
                                            {voting_open.then(|| view! {
                                                <div class="feature-cta">
                                                    <A href="/vote">
                                                        <Button appearance=ButtonAppearance::Primary>"Vote on Date →"</Button>
                                                    </A>
                                                </div>
                                            })}
                                            {dated.then(|| view! { <EventRsvp event=rsvp_event auth=auth /> })}
                                        </div>
                                    </div>
                                }.into_any()
                            }
                        }}

                        // ---- Subscribe (above the archive list) ----
                        <div style="margin:2.5rem 0;">
                            <CalendarSubscribe auth=auth />
                        </div>

                        // ---- Archive (reverse-chron, paginated) ----
                        <h2 class="section-heading mn-archive-heading">"All Screenings"</h2>
                        {if slice.is_empty() {
                            view! { <p class="mn-empty">"No screenings yet."</p> }.into_any()
                        } else {
                            view! {
                                <div>
                                    {slice.into_iter().map(|e| {
                                        let rsvp_event = e.clone();
                                        let dated = e.date.is_some();
                                        view! {
                                        <Card>
                                            <div class="mn-row">
                                                {e.poster_url.map(|url| view! {
                                                    <img src={url} alt="poster" class="mn-thumb" />
                                                })}
                                                <div class="mn-body">
                                                    <span class={format!("badge badge-{}", e.event_type)}>
                                                        {if e.event_type == "main" { "Featured Film" } else { "Special Feature" }}
                                                    </span>
                                                    <h3 class="mn-title">{e.title}</h3>
                                                    {e.date.as_deref().map(|d| view! {
                                                        <p class="mn-date">{pretty_date(d)}</p>
                                                    })}
                                                    {e.description.map(|d| view! {
                                                        <p class="mn-desc">{d}</p>
                                                    })}
                                                    {dated.then(|| view! { <EventRsvp event=rsvp_event auth=auth /> })}
                                                </div>
                                            </div>
                                        </Card>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>

                                <Show when=multi_page>
                                    <div class="pagination">
                                        <Button appearance=ButtonAppearance::Secondary on_click=go_prev>
                                            "← Prev"
                                        </Button>
                                        <span class="page-indicator">
                                            "Page " {move || page.get().min(total_pages.get() - 1) + 1}
                                            " of " {move || total_pages.get()}
                                        </span>
                                        <Button appearance=ButtonAppearance::Secondary on_click=go_next>
                                            "Next →"
                                        </Button>
                                    </div>
                                </Show>
                            }.into_any()
                        }}
                    }.into_any()
                }
            }
        }
    };

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
                {archive.clone()}
            </Show>
        </main>
    }
}
