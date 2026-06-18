use auth_client::{AuthUser, load_auth};
use crate::{
    components::{nav::Nav, notify_onboard::NotifyOnboard},
    pages::{
        about::AboutPage,
        admin_announcements::AdminAnnouncementsPage,
        admin_broadcast::AdminBroadcastPage,
        admin_events::AdminEventsPage,
        admin_invites::AdminInvitesPage,
        admin_users::AdminUsersPage,
        announcements::AnnouncementsPage,
        login::LoginPage,
        members::{MembersPage, MemberProfilePage},
        movie_nights::MovieNightsPage,
        notifications::NotificationsPage,
        profile::ProfilePage,
        vote::VotePage,
    },
    pwa::PwaBars,
    theme::gothic_theme,
};
use leptos::prelude::*;
use leptos_router::{
    NavigateOptions,
    components::{Route, Router, Routes},
    hooks::{use_location, use_navigate},
    path,
};
use thaw::{ConfigProvider, Theme};

/// Site-wide auth gate. Anyone without a session is redirected to `/login`
/// regardless of the route they hit; `/login` itself is the only public page.
/// Reactive on both `auth` and the current path, so logging out anywhere bounces
/// the user straight back to the login screen.
#[component]
fn AuthGuard(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let location = use_location();
    let navigate = use_navigate();
    Effect::new(move |_| {
        let path = location.pathname.get();
        if auth.get().is_none() && path != "/login" {
            navigate("/login", NavigateOptions::default());
        }
    });
}

#[component]
pub fn App() -> impl IntoView {
    let auth: RwSignal<Option<AuthUser>> = RwSignal::new(load_auth());
    // The single source of truth for the look. ConfigProvider injects these
    // tokens as CSS variables onto its wrapper, which every Thaw component
    // (and our own nav CSS) reads. See crate::theme.
    let theme: RwSignal<Theme> = RwSignal::new(gothic_theme());

    view! {
        <ConfigProvider theme class="app-shell">
            <PwaBars />
            <Router>
                <AuthGuard auth=auth />
                <Nav auth=auth />
                <NotifyOnboard auth=auth />
                <Routes fallback=|| view! { <main><p>"Page not found."</p></main> }>
                    <Route path=path!("/") view=move || view! { <AnnouncementsPage auth=auth /> } />
                    <Route path=path!("/about") view=|| view! { <AboutPage /> } />
                    <Route path=path!("/login") view=move || view! { <LoginPage auth=auth /> } />
                    <Route path=path!("/movie-nights") view=move || view! { <MovieNightsPage auth=auth /> } />
                    <Route path=path!("/vote") view=move || view! { <VotePage auth=auth /> } />
                    <Route path=path!("/members") view=move || view! { <MembersPage auth=auth /> } />
                    <Route path=path!("/members/:id") view=move || view! { <MemberProfilePage auth=auth /> } />
                    <Route path=path!("/notifications") view=move || view! { <NotificationsPage auth=auth /> } />
                    <Route path=path!("/profile") view=move || view! { <ProfilePage auth=auth /> } />
                    <Route path=path!("/admin/announcements") view=move || view! { <AdminAnnouncementsPage auth=auth /> } />
                    <Route path=path!("/admin/broadcast") view=move || view! { <AdminBroadcastPage auth=auth /> } />
                    <Route path=path!("/admin/events") view=move || view! { <AdminEventsPage auth=auth /> } />
                    <Route path=path!("/admin/invites") view=move || view! { <AdminInvitesPage auth=auth /> } />
                    <Route path=path!("/admin/users") view=move || view! { <AdminUsersPage auth=auth /> } />
                </Routes>
            </Router>
        </ConfigProvider>
    }
}
