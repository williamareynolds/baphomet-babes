use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <main>
            <div style="margin-bottom:3.5rem;">
                <h1 style="font-size:5rem;line-height:1;margin-bottom:0.15rem;">"Baphomet Babes"</h1>
                <p style="font-family:'IBM Plex Mono',monospace;font-size:0.75rem;letter-spacing:0.22em;text-transform:uppercase;color:#c41e3a;margin-bottom:1.5rem;">
                    "of Bentonville"
                </p>
                <p style="font-size:1.25rem;line-height:1.7;color:#8a7a8a;max-width:560px;">
                    "An inclusive collective for curious minds and bold spirits. "
                    "Cultural events, scientific discussions, crafts, sports, and more."
                </p>
            </div>

            <h3>"Apps"</h3>
            <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:1rem;margin-bottom:3rem;">
                <a href="https://movienight.baphometbabes.com" class="app-card">
                    <div class="app-label">"Weekly Gathering"</div>
                    <div class="app-name">"Movie Night"</div>
                    <div class="app-desc">"Vote on films, track watch history, and discover what the group is screening next."</div>
                    <div class="app-arrow">"Open →"</div>
                </a>
            </div>

            <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;letter-spacing:0.1em;color:#3a2a3a;border-top:1px solid #1e1526;padding-top:1.5rem;">
                "All are welcome. No exceptions."
            </p>
        </main>
    }
}
