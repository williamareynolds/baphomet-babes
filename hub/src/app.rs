use auth_client::{AuthUser, load_auth};
use crate::{
    components::nav::Nav,
    pages::{
        about::AboutPage,
        admin_events::AdminEventsPage,
        admin_invites::AdminInvitesPage,
        history::HistoryPage,
        home::HomePage,
        login::LoginPage,
        members::{MembersPage, MemberProfilePage},
        profile::ProfilePage,
        vote::VotePage,
    },
    pwa::PwaBars,
    theme::gothic_theme,
};
use leptos::prelude::*;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};
use thaw::{ConfigProvider, Theme};

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
                <Nav auth=auth />
                <Routes fallback=|| view! { <main><p>"Page not found."</p></main> }>
                    <Route path=path!("/") view=move || view! { <HomePage auth=auth /> } />
                    <Route path=path!("/about") view=|| view! { <AboutPage /> } />
                    <Route path=path!("/login") view=move || view! { <LoginPage auth=auth /> } />
                    <Route path=path!("/vote") view=move || view! { <VotePage auth=auth /> } />
                    <Route path=path!("/history") view=move || view! { <HistoryPage auth=auth /> } />
                    <Route path=path!("/members") view=move || view! { <MembersPage auth=auth /> } />
                    <Route path=path!("/members/:id") view=move || view! { <MemberProfilePage auth=auth /> } />
                    <Route path=path!("/profile") view=move || view! { <ProfilePage auth=auth /> } />
                    <Route path=path!("/admin/events") view=move || view! { <AdminEventsPage auth=auth /> } />
                    <Route path=path!("/admin/invites") view=move || view! { <AdminInvitesPage auth=auth /> } />
                </Routes>
            </Router>
        </ConfigProvider>
    }
}
