use std::fs;

use api::server::get_open_api;

mod api;
mod conf;
mod container;
mod db;
mod deployments;
mod docker;
mod env;
mod github;
mod hooks;
mod label;
mod listener;
mod logging;
mod paths;
mod provider;
mod sqlite_db;
mod tls;
mod tokens;
mod utils;

fn main() {
    let openapi = get_open_api();
    fs::write("docs/public/openapi.json", openapi.to_json().unwrap()).unwrap();
}
