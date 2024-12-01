use std::fs;

use api::server::get_open_api;

mod alphabet;
mod api;
mod conf;
mod container;
mod db;
mod deployment_hooks;
mod deployments;
mod docker;
mod env;
mod github;
mod listener;
mod logging;
mod paths;
mod time;

fn main() {
    let openapi = get_open_api();
    fs::write("/tmp/openapi.json", openapi.to_json().unwrap()).unwrap();
}