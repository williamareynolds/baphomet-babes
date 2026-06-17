use leptos::prelude::*;
use leptos_router::components::A;

/// Sub-navigation for the admin areas. `active` is the current section
/// ("events", "invites" or "users") so the matching tab is highlighted. The
/// Users tab is only rendered for superadmins, who alone can manage accounts.
#[component]
pub fn AdminNav(active: &'static str, #[prop(default = false)] is_superadmin: bool) -> impl IntoView {
    let cls = move |section: &str| {
        if section == active { "admin-tab admin-tab-active" } else { "admin-tab" }
    };
    view! {
        <div class="admin-tabs">
            <A href="/admin/events" attr:class=cls("events")>"Events"</A>
            <A href="/admin/invites" attr:class=cls("invites")>"Invites"</A>
            <Show when=move || is_superadmin>
                <A href="/admin/users" attr:class=cls("users")>"Users"</A>
            </Show>
        </div>
    }
}
