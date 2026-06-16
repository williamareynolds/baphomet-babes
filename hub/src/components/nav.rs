use auth_client::{AuthUser, clear_auth};
use leptos::prelude::*;
use leptos_router::components::A;

/// Top navigation bar. Bespoke "chrome" rather than a Thaw component (Thaw's
/// NavDrawer is a vertical sidebar), styled from the theme tokens / fonts in
/// `index.html` so it stays consistent with the Thaw-driven pages. The sticky
/// header is safe-area-aware so page content can never peek above it on notched
/// iPhones.
///
/// On wide viewports the links sit inline; below the breakpoint (see the
/// `@media` block in index.html) they collapse behind a hamburger toggle that
/// drops down as an overlay panel. `open` drives both the panel and the icon's
/// bars→X morph.
#[component]
pub fn Nav(auth: RwSignal<Option<AuthUser>>) -> impl IntoView {
    let (open, set_open) = signal(false);

    let logout = move |_| {
        clear_auth();
        auth.set(None);
        set_open.set(false);
    };
    // Any tap inside the links panel (i.e. following a link) closes the menu so
    // the overlay never lingers over the page after navigating on mobile.
    let close = move |_| set_open.set(false);

    view! {
        <nav class="site-nav">
            <A href="/" attr:class="nav-brand" on:click=close>
                <span class="brand-name">"Baphomet "</span>
                <span class="brand-accent">"Babes"</span>
            </A>
            <button
                class="nav-toggle"
                class:open=open
                aria-label="Toggle menu"
                aria-expanded=move || open.get().to_string()
                on:click=move |_| set_open.update(|o| *o = !*o)
            >
                <span></span>
                <span></span>
                <span></span>
            </button>
            <div class="nav-links" class:open=open on:click=close>
                <A href="/about">"About"</A>
                <Show when=move || auth.get().is_some()>
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
            </div>
        </nav>
    }
}
