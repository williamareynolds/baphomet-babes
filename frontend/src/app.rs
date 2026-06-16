use auth_client::{AuthUser, load_auth};
use crate::{
    components::nav::Nav,
    pages::{admin::AdminPage, history::HistoryPage, home::HomePage, login::LoginPage, vote::VotePage},
    pwa::PwaBars,
};
use leptos::prelude::*;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

#[component]
pub fn App() -> impl IntoView {
    let auth: RwSignal<Option<AuthUser>> = RwSignal::new(load_auth());

    view! {
        <PwaBars />
        <Router>
            <Nav auth=auth />
            <Routes fallback=|| view! { <main><p>"Page not found."</p></main> }>
                <Route path=path!("/") view=move || view! { <HomePage auth=auth /> } />
                <Route path=path!("/login") view=move || view! { <LoginPage auth=auth /> } />
                <Route path=path!("/vote") view=move || view! { <VotePage auth=auth /> } />
                <Route path=path!("/history") view=move || view! { <HistoryPage auth=auth /> } />
                <Route path=path!("/admin") view=move || view! { <AdminPage auth=auth /> } />
            </Routes>
        </Router>
    }
}
