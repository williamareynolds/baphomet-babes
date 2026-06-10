use auth_client::AuthUser;
use crate::api;
use leptos::prelude::*;

#[component]
pub fn HistoryPage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
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
            <h1>"Past Movie Nights"</h1>
            <Show
                when=move || auth.get().is_none()
                fallback=move || {
                    let today = today.clone();
                    view! {
                        {move || {
                            let today = today.clone();
                            match events.get() {
                                None => view! { <p>"Loading..."</p> }.into_any(),
                                Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                                Some(Ok(list)) => {
                                    let mut past: Vec<_> = list.into_iter()
                                        .filter(|e| e.date < today)
                                        .collect();
                                    past.sort_by(|a, b| b.date.cmp(&a.date));
                                    if past.is_empty() {
                                        view! { <p>"No past events yet."</p> }.into_any()
                                    } else {
                                        view! {
                                            <div>
                                                {past.into_iter().map(|e| view! {
                                                    <div class="card">
                                                        <span class={format!("badge badge-{}", e.event_type)}>
                                                            {if e.event_type == "main" { "Main Event" } else { "Special Feature" }}
                                                        </span>
                                                        {e.poster_url.map(|url| view! {
                                                            <div style="text-align:center;margin:1rem 0;">
                                                                <img src={url} alt="movie poster"
                                                                    style="width:100%;max-width:500px;border-radius:6px;" />
                                                            </div>
                                                        })}
                                                        <h2 style="margin-top:0.5rem;">{e.title}</h2>
                                                        <p style="color:#aaa;">{e.date}</p>
                                                        {e.description.map(|d| view! { <p style="margin-top:0.5rem;">{d}</p> })}
                                                    </div>
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }.into_any()
                                    }
                                }
                            }
                        }}
                    }
                }
            >
                <div class="card">
                    <p>"Please log in to see past events."</p>
                </div>
            </Show>
        </main>
    }
}
