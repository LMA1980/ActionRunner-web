mod app;
mod log_parser;
mod models;

use app::App;
use leptos::prelude::*;

fn main() {
    mount_to_body(App);
}
