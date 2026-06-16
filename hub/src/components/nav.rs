use auth_client::{AuthUser, clear_auth};
use leptos::prelude::*;
use leptos_router::components::A;

/// Top navigation bar. This is bespoke "chrome" rather than a Thaw component
/// (Thaw's NavDrawer is a vertical sidebar), but it is styled entirely from the
/// theme tokens / fonts defined in `index.html` so it stays consistent with the
/// Thaw-driven pages. The sticky header is safe-area-aware so page content can
/// never peek above it on notched iPhones.
#[component]
pub fn Nav(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let logout = move |_| {
        clear_auth();
        auth.set(None);
    };

    view! {
        <nav class="site-nav">
            <A href="/" attr:class="nav-brand">
                <span class="brand-name">"Baphomet "</span>
                <span class="brand-accent">"Babes"</span>
            </A>
            <A href="/about">"About"</A>
            <Show when=move || auth.get().is_some()>
                <A href="/vote">"Vote"</A>
                <A href="/history">"History"</A>
                <A href="/members">"Members"</A>
                <A href="/profile">"My Profile"</A>
                <Show when=move || auth.get().map(|u| u.is_admin()).unwrap_or(false)>
                    <A href="/admin/events">"Admin"</A>
                </Show>
                <button class="nav-link-btn" on:click=logout>"Logout"</button>
            </Show>
            <Show when=move || auth.get().is_none()>
                <A href="/login">"Login"</A>
            </Show>
        </nav>
    }
}
