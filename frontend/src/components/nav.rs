use crate::context::{AuthUser, clear_auth};
use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn Nav(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let logout = move |_| {
        clear_auth();
        auth.set(None);
    };

    view! {
        <nav>
            <A href="/" attr:class="nav-brand">"Baphomet Babes"</A>
            <a href="https://baphometbabes.com/about">"About"</a>
            <Show when=move || auth.get().is_some()>
                <A href="/vote">"Vote"</A>
                <A href="/history">"History"</A>
                <Show when=move || auth.get().map(|u| u.is_admin()).unwrap_or(false)>
                    <A href="/admin">"Admin"</A>
                </Show>
                <button class="secondary" on:click=logout style="padding:0.25rem 0.75rem;font-size:0.85rem;">
                    "Logout"
                </button>
            </Show>
            <Show when=move || auth.get().is_none()>
                <A href="/login">"Login"</A>
            </Show>
        </nav>
    }
}
