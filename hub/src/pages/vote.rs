use auth_client::AuthUser;
use crate::api;
use leptos::prelude::*;

#[component]
pub fn VotePage(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let poll: RwSignal<Option<Result<Option<shared::Event>, String>>> = RwSignal::new(None);

    Effect::new(move |_| {
        let token = auth.get().map(|u| u.token);
        wasm_bindgen_futures::spawn_local(async move {
            let result = match token {
                None => return,
                Some(t) => api::fetch_events(&t).await.map(|events| {
                    events
                        .into_iter()
                        // Voting picks a date — a dated event's poll is closed.
                        .find(|e| e.event_type == "main" && e.poll_embed_url.is_some() && e.date.is_none())
                }),
            };
            poll.set(Some(result));
        });
    });

    view! {
        <main>
            <h1>"Vote for Next Movie Night"</h1>
            <Show
                when=move || auth.get().is_none()
                fallback=move || view! {
                    {move || match poll.get() {
                        None => view! { <p>"Loading..."</p> }.into_any(),
                        Some(Err(e)) => view! { <p class="error">{e}</p> }.into_any(),
                        Some(Ok(None)) => view! {
                            <p>"No active poll right now. Check back soon!"</p>
                        }.into_any(),
                        Some(Ok(Some(event))) => view! {
                            <div>
                                <p class="event-date" style="margin-bottom:1rem;">
                                    "Voting for: "<strong>{event.title}</strong>
                                </p>
                                <iframe
                                    src={event.poll_embed_url.unwrap_or_default()}
                                    class="poll-frame"
                                />
                            </div>
                        }.into_any(),
                    }}
                }
            >
                <p>"Please log in to vote."</p>
            </Show>
        </main>
    }
}
