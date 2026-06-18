mod api;
mod app;
mod components;
mod pages;
mod push;
mod pwa;
mod theme;

fn main() {
    leptos::mount::mount_to_body(app::App);
}
