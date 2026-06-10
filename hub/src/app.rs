use auth_client::{AuthUser, clear_auth, load_auth};
use crate::pages::{
    about::AboutPage,
    home::HomePage,
    login::LoginPage,
    members::{MembersPage, MemberProfilePage},
    profile::ProfilePage,
};
use leptos::prelude::*;
use leptos_router::{
    components::{Route, Router, Routes, A},
    path,
};

#[component]
pub fn App() -> impl IntoView {
    let auth: RwSignal<Option<AuthUser>> = RwSignal::new(load_auth());

    let logout = move |_| {
        clear_auth();
        auth.set(None);
    };

    view! {
        <Router>
            <nav>
                <A href="/" attr:class="nav-brand">
                    <span class="brand-name">"Baphomet "</span>
                    <span class="brand-accent">"Babes"</span>
                </A>
                <A href="/about">"About"</A>
                <Show when=move || auth.get().is_some()>
                    <A href="/members">"Members"</A>
                    <A href="/profile">"My Profile"</A>
                    <button class="secondary" on:click=logout style="padding:0.25rem 0.75rem;font-size:0.85rem;">
                        "Logout"
                    </button>
                </Show>
                <Show when=move || auth.get().is_none()>
                    <A href="/login">"Login"</A>
                </Show>
            </nav>
            <Routes fallback=|| view! { <main><p>"Page not found."</p></main> }>
                <Route path=path!("/") view=move || view! { <HomePage auth=auth /> } />
                <Route path=path!("/about") view=|| view! { <AboutPage /> } />
                <Route path=path!("/login") view=move || view! { <LoginPage auth=auth /> } />
                <Route path=path!("/members") view=move || view! { <MembersPage auth=auth /> } />
                <Route path=path!("/members/:id") view=move || view! { <MemberProfilePage auth=auth /> } />
                <Route path=path!("/profile") view=move || view! { <ProfilePage auth=auth /> } />
            </Routes>
        </Router>
    }
}
