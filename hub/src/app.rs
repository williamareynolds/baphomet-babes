use auth_client::{AuthUser, load_auth, refresh_push};
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
        chat::ChatPage,
        install::InstallPage,
        login::LoginPage,
        members::{MembersPage, MemberProfilePage},
        movie_nights::MovieNightsPage,
        notifications::NotificationsPage,
        profile::ProfilePage,
        rides::RidesPage,
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

    // Self-healing push: on every load where permission is already granted,
    // silently re-mint the FCM token and re-register it with the backend.
    // Catches token rotation, iOS quietly dropping subscriptions, and tokens
    // the backend pruned — without asking the member to touch anything.
    Effect::new(move |_| {
        let Some(user) = auth.get() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(tok) = refresh_push().await {
                if crate::api::register_push_token(&tok, &user.token).await.is_ok() {
                    crate::push::save(&tok);
                }
            }
        });
    });

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
                    <Route path=path!("/rides") view=move || view! { <RidesPage auth=auth /> } />
                    <Route path=path!("/chat") view=move || view! { <ChatPage auth=auth /> } />
                    <Route path=path!("/install") view=|| view! { <InstallPage /> } />
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
