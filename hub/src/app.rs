use crate::pages::{about::AboutPage, home::HomePage};
use leptos::prelude::*;
use leptos_router::{
    components::{Route, Router, Routes, A},
    path,
};

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <nav>
                <A href="/" attr:class="nav-brand">"Baphomet Babes"</A>
                <A href="/about">"About"</A>
            </nav>
            <Routes fallback=|| view! { <main><p>"Page not found."</p></main> }>
                <Route path=path!("/") view=|| view! { <HomePage /> } />
                <Route path=path!("/about") view=|| view! { <AboutPage /> } />
            </Routes>
        </Router>
    }
}
