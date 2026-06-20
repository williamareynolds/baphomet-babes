//! "Install the app" onboarding. There's no single official cross-platform guide
//! worth linking (vendor docs are fragmented and developer-oriented), so we ship
//! our own: the steps for the visitor's detected platform up top, with every
//! other platform tucked into collapsible sections below.

use leptos::prelude::*;
use thaw::Card;

#[derive(Clone, Copy, PartialEq)]
enum Platform {
    Ios,
    Android,
    DesktopChromium,
    SafariMac,
    Firefox,
    Other,
}

/// Best-effort platform sniff from the user agent. Only drives which guide shows
/// first — every guide is still reachable below, so a wrong guess is harmless.
fn detect() -> Platform {
    let ua = web_sys::window()
        .and_then(|w| w.navigator().user_agent().ok())
        .unwrap_or_default()
        .to_lowercase();

    if ua.contains("iphone") || ua.contains("ipad") || ua.contains("ipod") {
        return Platform::Ios;
    }
    if ua.contains("android") {
        // Firefox on Android installs differently from Chrome.
        if ua.contains("firefox") || ua.contains("fxios") {
            return Platform::Firefox;
        }
        return Platform::Android;
    }
    if ua.contains("firefox") || ua.contains("fxios") {
        return Platform::Firefox;
    }
    // Chromium UAs also contain "safari", so check them before Safari proper.
    if ua.contains("edg") || ua.contains("chrome") || ua.contains("chromium") {
        return Platform::DesktopChromium;
    }
    if ua.contains("safari") && ua.contains("macintosh") {
        return Platform::SafariMac;
    }
    Platform::Other
}

struct Guide {
    id: Platform,
    icon: &'static str,
    name: &'static str,
    steps: &'static [&'static str],
    note: Option<&'static str>,
}

const GUIDES: &[Guide] = &[
    Guide {
        id: Platform::Ios,
        icon: "📱",
        name: "iPhone & iPad",
        steps: &[
            "Open this site in Safari. (In-app browsers like Instagram or Facebook can't install apps.)",
            "Tap the Share button — the square with an upward arrow — in the toolbar.",
            "Scroll down and tap \"Add to Home Screen.\"",
            "Tap \"Add\" in the top-right corner.",
            "Open Baphomet Babes from your Home Screen, then allow notifications when prompted.",
        ],
        note: Some(
            "On iPhone and iPad, notifications only work once the app is on your Home Screen. \
             Chrome and Edge on iOS 16.4+ use the same Share → Add to Home Screen steps.",
        ),
    },
    Guide {
        id: Platform::Android,
        icon: "🤖",
        name: "Android (Chrome)",
        steps: &[
            "Open this site in Chrome.",
            "Tap the ⋮ menu in the top-right corner.",
            "Tap \"Install app\" (or \"Add to Home screen\").",
            "Confirm with \"Install\" / \"Add.\"",
            "Launch it from your app drawer and allow notifications.",
        ],
        note: Some("Some phones show an \"Install\" banner at the bottom of the page — tapping that works too."),
    },
    Guide {
        id: Platform::DesktopChromium,
        icon: "💻",
        name: "Desktop (Chrome / Edge)",
        steps: &[
            "Look for the install icon — a monitor with a downward arrow — at the right end of the address bar.",
            "Click it, then click \"Install.\"",
            "No icon? Open the ⋮ / … menu and choose \"Install Baphomet Babes\" (under \"Apps\" in Edge).",
            "The app opens in its own window with a desktop / Start-menu shortcut.",
        ],
        note: None,
    },
    Guide {
        id: Platform::SafariMac,
        icon: "🍎",
        name: "Mac (Safari)",
        steps: &[
            "Open this site in Safari 17 or newer (macOS Sonoma and up).",
            "Click the Share button in the toolbar.",
            "Choose \"Add to Dock.\"",
            "Launch Baphomet Babes from the Dock.",
        ],
        note: Some("Older Safari versions can't install web apps — use Chrome or Edge instead."),
    },
    Guide {
        id: Platform::Firefox,
        icon: "🦊",
        name: "Firefox",
        steps: &[
            "On Android: tap the ⋮ menu, then \"Add to Home screen.\"",
            "On desktop: Firefox can't install web apps — open the site in Chrome or Edge for the full app.",
        ],
        note: Some("The installed app adds offline access and notifications; the desktop browser tab won't have those."),
    },
    Guide {
        id: Platform::Other,
        icon: "🌐",
        name: "Other browsers",
        steps: &[
            "Open your browser's main menu.",
            "Look for \"Install app,\" \"Add to Home screen,\" or \"Add to Dock.\"",
        ],
        note: Some(
            "For the best experience — offline access plus notifications — use Safari on iPhone/iPad, \
             or Chrome or Edge everywhere else.",
        ),
    },
];

fn steps_list(steps: &'static [&'static str]) -> impl IntoView {
    view! {
        <ol class="install-steps">
            {steps.iter().map(|s| view! { <li>{*s}</li> }).collect::<Vec<_>>()}
        </ol>
    }
}

#[component]
pub fn InstallPage() -> impl IntoView {
    let platform = detect();
    let installed = crate::pwa::is_standalone();

    let primary = GUIDES.iter().find(|g| g.id == platform).unwrap_or(&GUIDES[GUIDES.len() - 1]);
    let others: Vec<&Guide> = GUIDES.iter().filter(|g| g.id != platform).collect();

    view! {
        <main>
            <div style="max-width:680px;">
                <h1>"Install the App"</h1>
                <p style="color:#ccbfc0;font-size:1.1rem;line-height:1.75;margin-bottom:1.5rem;">
                    "Install Baphomet Babes to get push notifications, an icon on your "
                    "Home Screen, and offline access — it runs in its own window, just like "
                    "a native app. No app store, no download."
                </p>

                {installed.then(|| view! {
                    <Card>
                        <p style="color:#7ec699;">
                            "✓ You're already running the installed app. You're all set!"
                        </p>
                    </Card>
                })}

                <Card>
                    <p class="install-kicker">"Recommended for your device"</p>
                    <h2 class="install-name">{primary.icon}" "{primary.name}</h2>
                    {steps_list(primary.steps)}
                    {primary.note.map(|n| view! { <p class="install-note">{n}</p> })}
                </Card>

                <h2 class="section-heading">"Other devices"</h2>
                {others.into_iter().map(|g| view! {
                    <details class="install-details">
                        <summary>{g.icon}" "{g.name}</summary>
                        {steps_list(g.steps)}
                        {g.note.map(|n| view! { <p class="install-note">{n}</p> })}
                    </details>
                }).collect::<Vec<_>>()}

                <p class="install-note" style="margin-top:1.75rem;">
                    "Stuck? Mozilla keeps a plain-language overview of installing web apps at "
                    <a
                        href="https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Guides/Installing"
                        target="_blank"
                        rel="noopener"
                        style="color:#bda4e6;"
                    >"developer.mozilla.org"</a>
                    "."
                </p>
            </div>
        </main>
    }
}
