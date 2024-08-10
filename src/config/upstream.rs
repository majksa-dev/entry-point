use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Upstream {
    pub host: String,
    #[serde(default = "Upstream::default_port")]
    pub port: u16,
}

impl Upstream {
    pub fn default_port() -> u16 {
        80
    }
}
