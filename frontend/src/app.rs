use crate::{
    components::nav::Nav,
    context::{AuthUser, load_auth},
    pages::{about::AboutPage, admin::AdminPage, history::HistoryPage, home::HomePage, login::LoginPage, vote::VotePage},
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
        <Router>
            <Nav auth=auth />
            <Routes fallback=|| view! { <main><p>"Page not found."</p></main> }>
                <Route path=path!("/") view=move || view! { <HomePage auth=auth /> } />
                <Route path=path!("/about") view=|| view! { <AboutPage /> } />
                <Route path=path!("/login") view=move || view! { <LoginPage auth=auth /> } />
                <Route path=path!("/vote") view=move || view! { <VotePage auth=auth /> } />
                <Route path=path!("/history") view=move || view! { <HistoryPage auth=auth /> } />
                <Route path=path!("/admin") view=move || view! { <AdminPage auth=auth /> } />
            </Routes>
        </Router>
    }
}
