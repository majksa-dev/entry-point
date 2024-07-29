use serde::Deserialize;

use super::upstream::Upstream;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfigRaw {
    pub upstream: Upstream,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub name: String,
    pub upstream: Upstream,
}

impl AppConfig {
    pub fn from_raw(data: AppConfigRaw, name: String) -> Self {
        AppConfig {
            name,
            upstream: data.upstream,
        }
    }
}
