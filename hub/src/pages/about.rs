use leptos::prelude::*;
use thaw::Card;

#[component]
pub fn AboutPage() -> impl IntoView {
    let pillars = [
        ("Cultural Events", "Film screenings, music, art — celebrating culture in all its forms."),
        ("Scientific Discussions", "Deep dives into the natural world, technology, and the cosmos."),
        ("Crafts", "Making things with our hands — workshops, projects, collaborative builds."),
        ("Sports", "Getting outside and moving — casual games to organized outings."),
    ];

    view! {
        <main>
            <div style="max-width:680px;">
                <div style="margin-bottom:3rem;">
                    <h1 style="font-size:4.5rem;line-height:1;margin-bottom:0.1rem;">"Baphomet Babes"</h1>
                    <p style="font-family:'IBM Plex Mono',monospace;font-size:0.75rem;letter-spacing:0.22em;text-transform:uppercase;color:#c41e3a;margin-bottom:2rem;">
                        "of Bentonville"
                    </p>
                    <p style="font-size:1.35rem;line-height:1.7;color:#c8b8b0;font-style:italic;margin-bottom:1.75rem;">
                        "An inclusive collective for curious minds and bold spirits."
                    </p>
                    <p style="font-size:1.15rem;line-height:1.8;color:#9a8a8a;">
                        "We are the Baphomet Babes of Bentonville — a welcoming community open to anyone "
                        "who wants to connect, learn, and create together. We believe the most interesting "
                        "conversations happen at the intersection of art, science, and lived experience."
                    </p>
                </div>

                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:1rem;margin-bottom:3rem;">
                    {pillars.into_iter().map(|(label, desc)| view! {
                        <Card>
                            <p style="font-family:'IBM Plex Mono',monospace;font-size:0.6rem;letter-spacing:0.16em;text-transform:uppercase;color:#c41e3a;margin-bottom:0.6rem;">
                                {label}
                            </p>
                            <p style="color:#8a7a7a;font-size:1.05rem;line-height:1.65;">
                                {desc}
                            </p>
                        </Card>
                    }).collect::<Vec<_>>()}
                </div>

                <p style="font-family:'IBM Plex Mono',monospace;font-size:0.7rem;letter-spacing:0.1em;color:#3a2a3a;border-top:1px solid #1e1526;padding-top:1.5rem;">
                    "All are welcome. No exceptions."
                </p>
            </div>
        </main>
    }
}
