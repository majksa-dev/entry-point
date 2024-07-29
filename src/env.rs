use std::{net::IpAddr, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Env {
    pub https_port: Option<u16>,
    pub http_port: Option<u16>,
    pub healthcheck_port: Option<u16>,
    pub host: Option<IpAddr>,
    pub hostname: String,
    pub config_file: PathBuf,
    pub certs_dir: PathBuf,
}

impl Env {
    pub fn new() -> Result<Self, envy::Error> {
        envy::from_env()
    }
}
