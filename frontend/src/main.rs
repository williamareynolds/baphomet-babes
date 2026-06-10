mod api;
mod app;
mod components;
mod pages;

use leptos::prelude::*;

fn main() {
    mount_to_body(app::App);
}
