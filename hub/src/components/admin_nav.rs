use leptos::prelude::*;
use leptos_router::components::A;

/// Sub-navigation for the two admin areas (events / invites). `active` is the
/// current section ("events" or "invites") so the matching tab is highlighted.
#[component]
pub fn AdminNav(active: &'static str) -> impl IntoView {
    let cls = move |section: &str| {
        if section == active { "admin-tab admin-tab-active" } else { "admin-tab" }
    };
    view! {
        <div class="admin-tabs">
            <A href="/admin/events" attr:class=cls("events")>"Events"</A>
            <A href="/admin/invites" attr:class=cls("invites")>"Invites"</A>
        </div>
    }
}
