use leptos::prelude::*;
use leptos_router::components::A;

/// Sub-navigation for the admin areas. `active` is the current section
/// ("events", "invites", "users", …) so the matching tab is highlighted.
/// Three visibility tiers:
/// - `is_admin` (full admin/superadmin): Announcements, Broadcast, Events,
///   Invites, Roles.
/// - `is_admin` OR `can_manage_book_club` (a scoped Book Club Manager who
///   isn't a full admin): Book Club.
/// - `is_superadmin`: Users, Calendar links — the first manages accounts, the
///   second hands out a bearer credential for the whole schedule to people
///   who aren't members.
#[component]
pub fn AdminNav(
    active: &'static str,
    #[prop(default = false)] is_admin: bool,
    #[prop(default = false)] is_superadmin: bool,
    #[prop(default = false)] can_manage_book_club: bool,
) -> impl IntoView {
    let cls = move |section: &str| {
        if section == active { "admin-tab admin-tab-active" } else { "admin-tab" }
    };
    view! {
        <div class="admin-tabs">
            <Show when=move || is_admin>
                <A href="/admin/announcements" attr:class=cls("announcements")>"Announcements"</A>
                <A href="/admin/broadcast" attr:class=cls("broadcast")>"Broadcast"</A>
                <A href="/admin/events" attr:class=cls("events")>"Events"</A>
            </Show>
            <Show when=move || is_admin || can_manage_book_club>
                <A href="/admin/book-club" attr:class=cls("book-club")>"Book Club"</A>
            </Show>
            <Show when=move || is_admin>
                <A href="/admin/invites" attr:class=cls("invites")>"Invites"</A>
                <A href="/admin/roles" attr:class=cls("roles")>"Roles"</A>
            </Show>
            <Show when=move || is_superadmin>
                <A href="/admin/users" attr:class=cls("users")>"Users"</A>
                <A href="/admin/calendar-links" attr:class=cls("calendar-links")>"Calendar links"</A>
            </Show>
        </div>
    }
}
