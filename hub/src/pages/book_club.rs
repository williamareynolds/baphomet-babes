//! Book Club: one book at a time. Reuses the Movie Nights hero
//! (`.next-feature`/`.kicker`/`.feature-*`) for the current pick and the
//! Gatherings RSVP pattern (`.rsvp-*`) for the next meetup. Joining is
//! ongoing club membership — separate from the meetup RSVP, which stays open
//! to any member either way.

use auth_client::AuthUser;
use crate::api;
use leptos::prelude::*;
use shared::{BookClubMeetup, BookClubStatus};
use thaw::{Button, ButtonAppearance, Card};

/// "2026-09-01T18:30" -> "September 1, 2026 · 6:30 PM". Same shape as
/// `gatherings::pretty_when`; duplicated rather than shared, matching how
/// each page in this codebase owns its own formatting helpers.
fn pretty_when(s: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    let Some((date, time)) = s.split_once('T') else { return s.to_string() };
    let d: Vec<&str> = date.split('-').collect();
    let t: Vec<&str> = time.split(':').collect();
    match (d.first(), d.get(1), d.get(2), t.first(), t.get(1)) {
        (Some(y), Some(m), Some(day), Some(h), Some(min)) => {
            let (Ok(mi), Ok(hh)) = (m.parse::<usize>(), h.parse::<u32>()) else {
                return s.to_string();
            };
            if !(1..=12).contains(&mi) || hh > 23 {
                return s.to_string();
            }
            let suffix = if hh < 12 { "AM" } else { "PM" };
            let h12 = match hh % 12 { 0 => 12, other => other };
            format!(
                "{} {}, {} · {}:{} {}",
                MONTHS[mi - 1],
                day.trim_start_matches('0'),
                y,
                h12,
                min,
                suffix
            )
        }
        _ => s.to_string(),
    }
}

fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            other => other
                .to_string()
                .bytes()
                .map(|b| format!("%{b:02X}"))
                .collect::<String>(),
        })
        .collect()
}

fn maps_link(address: Option<&str>, lat: Option<f64>, lng: Option<f64>) -> Option<String> {
    match (lat, lng) {
        (Some(lat), Some(lng)) => Some(format!("https://www.google.com/maps/search/?api=1&query={lat},{lng}")),
        _ => address.map(|a| format!("https://www.google.com/maps/search/?api=1&query={}", urlencode(a))),
    }
}

#[component]
pub fn BookClubPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let (refresh, set_refresh) = signal(0u32);
    let status: RwSignal<Option<Result<BookClubStatus, String>>> = RwSignal::new(None);
    let meetup: RwSignal<Option<Result<Option<BookClubMeetup>, String>>> = RwSignal::new(None);
    let archive: RwSignal<Option<Result<Vec<shared::BookClubBook>, String>>> = RwSignal::new(None);
    let busy = RwSignal::new(false);

    Effect::new(move |_| {
        let _ = refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(t) = token else { return };
            status.set(Some(api::fetch_book_club_current(&t).await));
        });
    });

    Effect::new(move |_| {
        let _ = refresh.get();
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(t) = token else { return };
            meetup.set(Some(api::fetch_book_club_meetup(&t).await));
        });
    });

    Effect::new(move |_| {
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let Some(t) = token else { return };
            archive.set(Some(api::fetch_book_club_archive(&t).await));
        });
    });

    let toggle_join = move |joined: bool| {
        let Some(user) = auth.get() else { return };
        busy.set(true);
        wasm_bindgen_futures::spawn_local(async move {
            let result = if joined {
                api::leave_book_club(&user.token).await
            } else {
                api::join_book_club(&user.token).await
            };
            if result.is_ok() {
                set_refresh.update(|n| *n += 1);
            }
            busy.set(false);
        });
    };

    let toggle_rsvp = move |going: bool| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if api::rsvp_book_club_meetup(going, &user.token).await.is_ok() {
                set_refresh.update(|n| *n += 1);
            }
        });
    };

    view! {
        <main class="page">
            <h1 class="page-title">"Book Club"</h1>
            <p class="page-sub">"One book at a time. We read it, then meet to talk about it."</p>

            {move || match status.get() {
                None => view! { <p>"Loading…"</p> }.into_any(),
                Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                Some(Ok(s)) => {
                    let joined = s.joined;
                    let member_count = s.member_count;
                    let reading_along = s.reading_along.clone();
                    match s.current {
                        None => view! {
                            <div class="next-feature">
                                <div class="feature-body">
                                    <p class="kicker">"Between Books"</p>
                                    <p class="feature-desc">"No current pick — the next one goes up soon."</p>
                                    <div class="feature-cta">
                                        <Button
                                            appearance=if joined { ButtonAppearance::Secondary } else { ButtonAppearance::Primary }
                                            disabled=Signal::derive(move || busy.get())
                                            on_click=move |_| toggle_join(joined)
                                        >
                                            {if joined { "In the club ✓ · tap to leave" } else { "Join the Book Club" }}
                                        </Button>
                                    </div>
                                </div>
                            </div>
                        }.into_any(),
                        Some(book) => {
                            let cover = book.cover_url.clone();
                            let meta = match book.published.clone() {
                                Some(p) => format!("{} · {p}", book.author),
                                None => book.author.clone(),
                            };
                            view! {
                                <div class="next-feature">
                                    {cover.map(|url| view! {
                                        <div class="feature-poster-wrap">
                                            <img src={url} alt="book cover" class="feature-poster" />
                                        </div>
                                    })}
                                    <div class="feature-body">
                                        <p class="kicker">"Now Reading"</p>
                                        <h2 class="feature-title">{book.title.clone()}</h2>
                                        <p class="feature-date">{meta}</p>
                                        {book.why.clone().map(|w| view! { <p class="feature-desc">{w}</p> })}
                                        <p class="feature-desc" style="color:#93d8b4;font-weight:600;">
                                            {format!("{member_count} member{} reading", if member_count == 1 { "" } else { "s" })}
                                        </p>
                                        <div class="feature-cta">
                                            <Button
                                                appearance=if joined { ButtonAppearance::Secondary } else { ButtonAppearance::Primary }
                                                disabled=Signal::derive(move || busy.get())
                                                on_click=move |_| toggle_join(joined)
                                            >
                                                {if joined { "In the club ✓ · tap to leave" } else { "Join the Book Club" }}
                                            </Button>
                                            <p style="font-family:'IBM Plex Mono',monospace;font-size:0.68rem;color:#95868f;margin-top:0.6rem;">
                                                {if joined {
                                                    "Book Club notifications are on — push and email."
                                                } else {
                                                    "Joining turns on Book Club notifications — push and email."
                                                }}
                                            </p>
                                        </div>
                                    </div>
                                </div>
                                {(!reading_along.is_empty()).then(|| view! {
                                    <h2 class="section-heading mn-archive-heading">"Reading Along"</h2>
                                    <p class="mn-desc">{reading_along.join(", ")}</p>
                                })}
                            }.into_any()
                        }
                    }
                }
            }}

            {move || match meetup.get() {
                None => Some(view! { <p>"Loading…"</p> }.into_any()),
                Some(Err(e)) => Some(view! { <p class="error">{e}</p> }.into_any()),
                // No meetup scheduled yet — nothing to show.
                Some(Ok(None)) => None,
                Some(Ok(Some(m))) => {
                    let going = m.my_rsvp;
                    let link = maps_link(m.address.as_deref(), m.lat, m.lng);
                    Some(view! {
                        <Card>
                            <p class="kicker">"Next Meetup"</p>
                            <h3 class="mn-title">{pretty_when(&m.starts_at)}</h3>
                            <p class="mn-desc">{format!("Hosted by {}", m.host)}</p>
                            {m.address.clone().map(|a| view! { <p class="mn-desc">{a}</p> })}
                            {link.map(|href| view! {
                                <p><a href={href} target="_blank" rel="noopener">"Open in maps →"</a></p>
                            })}
                            <p class="rsvp-count">
                                {match m.rsvp_count {
                                    0 => "Nobody going yet".to_string(),
                                    1 => "1 person going".to_string(),
                                    n => format!("{n} people going"),
                                }}
                            </p>
                            {(!m.going.is_empty()).then(|| view! { <p class="mn-desc">{m.going.join(", ")}</p> })}
                            <Button
                                class="rsvp-btn"
                                appearance=if going { ButtonAppearance::Secondary } else { ButtonAppearance::Primary }
                                on_click=move |_| toggle_rsvp(!going)
                            >
                                {if going { "Going ✓" } else { "RSVP — I'm going" }}
                            </Button>
                        </Card>
                    }.into_any())
                }
            }}

            <h2 class="section-heading mn-archive-heading">"Past Books"</h2>
            {move || match archive.get() {
                None => view! { <p>"Loading…"</p> }.into_any(),
                Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                Some(Ok(list)) if list.is_empty() => {
                    view! { <p class="mn-empty">"Nothing archived yet."</p> }.into_any()
                }
                Some(Ok(list)) => view! {
                    <div>
                        {list.into_iter().map(|b| {
                            let meta = match b.published.clone() {
                                Some(p) => format!("{} · {p}", b.author),
                                None => b.author.clone(),
                            };
                            view! {
                                <Card>
                                    <div class="mn-row">
                                        {b.cover_url.clone().map(|url| view! {
                                            <img src={url} alt="cover" class="mn-thumb" />
                                        })}
                                        <div class="mn-body">
                                            <h3 class="mn-title">{b.title.clone()}</h3>
                                            <p class="mn-date">{meta}</p>
                                            {b.why.clone().map(|w| view! { <p class="mn-desc">{w}</p> })}
                                        </div>
                                    </div>
                                </Card>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any(),
            }}
        </main>
    }
}
