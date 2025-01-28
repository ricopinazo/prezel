use serde::Deserialize;
use std::fs;

use crate::paths::get_container_root;

#[derive(Deserialize, Clone, Debug)]
pub(crate) struct Conf {
    pub(crate) secret: String,
    pub(crate) hostname: String,
    pub(crate) provider: String,
}

impl Conf {
    pub(crate) fn read() -> Self {
        // let conf_data = env::var("CONFIG").expect("Unable to read CONFIG from env");
        let conf_path = get_container_root().join("config.json");
        // println!("reading conf from {conf_path:?}");
        let conf_data = fs::read_to_string(conf_path).expect("Unable to find config.json");
        serde_json::from_str(&conf_data).expect("Invalid content for conf.json")
    }

    pub(crate) fn api_hostname(&self) -> String {
        // TODO: compute this in read() and add it as an additional field
        format!("api.{}", self.hostname)
    }

    pub(crate) fn wildcard_domain(&self) -> String {
        format!("*.{}", self.hostname)
    }
}
