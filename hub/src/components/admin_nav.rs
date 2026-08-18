use leptos::prelude::*;
use leptos_router::components::A;

/// Sub-navigation for the admin areas. `active` is the current section
/// ("events", "invites", "users", …) so the matching tab is highlighted. The
/// Users and Calendar links tabs are only rendered for superadmins — the first
/// manages accounts, the second hands out a bearer credential for the whole
/// schedule to people who aren't members.
#[component]
pub fn AdminNav(active: &'static str, #[prop(default = false)] is_superadmin: bool) -> impl IntoView {
    let cls = move |section: &str| {
        if section == active { "admin-tab admin-tab-active" } else { "admin-tab" }
    };
    view! {
        <div class="admin-tabs">
            <A href="/admin/announcements" attr:class=cls("announcements")>"Announcements"</A>
            <A href="/admin/broadcast" attr:class=cls("broadcast")>"Broadcast"</A>
            <A href="/admin/events" attr:class=cls("events")>"Events"</A>
            <A href="/admin/invites" attr:class=cls("invites")>"Invites"</A>
            <Show when=move || is_superadmin>
                <A href="/admin/users" attr:class=cls("users")>"Users"</A>
                <A href="/admin/calendar-links" attr:class=cls("calendar-links")>"Calendar links"</A>
            </Show>
        </div>
    }
}
