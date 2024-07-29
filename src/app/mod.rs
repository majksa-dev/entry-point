use crate::config::apps::Apps;
use anyhow::{Context, Result};
use essentials::debug;
use gateway::{self, http::HeaderMapExt, tcp, Request, Server};
use gateway::{
    tokio_rustls::{
        rustls::{
            crypto::aws_lc_rs::sign::any_supported_type,
            pki_types::{CertificateDer, PrivateKeyDer},
            server::ResolvesServerCertUsingSni,
            sign::CertifiedKey,
            ServerConfig,
        },
        TlsAcceptor,
    },
    AnyRouterBuilder,
};
use http::header;
use rustls_pemfile::{certs, private_key};
use std::{
    fs::File,
    io::{self, BufReader},
    sync::Arc,
};
use std::{net::IpAddr, path::Path};
use tokio::fs;

use crate::env::Env;

async fn load_config(config_path: impl AsRef<Path>) -> Result<Apps> {
    let config_data = fs::read_to_string(config_path)
        .await
        .with_context(|| "Failed to read config file")?;
    Apps::new(config_data).with_context(|| "Failed to parse config file")
}

fn parse_host<'a>(host: &'a str, hostname: &str) -> Option<(&'a str, &'a str)> {
    if !host.ends_with(hostname) {
        return None;
    }
    let host = &host[..host.len() - hostname.len()];
    if !host.ends_with('.') {
        return None;
    }
    let host = &host[..host.len() - 1];
    let environment = host.rfind('.')?;
    let app = host[..environment].rfind('.')?;
    let (rest, gateway) = host.split_at(app);
    Some((rest, &gateway[1..]))
}

fn peer_key_from_host(
    hostname: String,
) -> impl Fn(&Request) -> Option<(String, Option<String>)> + Send + Sync + 'static {
    move |req: &Request| {
        req.header(header::HOST)
            .and_then(|host| host.to_str().ok())
            .and_then(|host| {
                parse_host(host, &hostname)
                    .map(|(remains, gateway)| (gateway.to_string(), Some(remains.to_string())))
            })
    }
}

fn load_certs(path: &Path) -> io::Result<Vec<CertificateDer<'static>>> {
    certs(&mut BufReader::new(File::open(path)?)).collect()
}

fn load_keys(path: &Path) -> io::Result<PrivateKeyDer<'static>> {
    private_key(&mut BufReader::new(File::open(path)?))?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No keys found in file"))
}

pub async fn build(env: Env) -> Result<Server> {
    let config = load_config(env.config_file).await?;
    debug!("{:?}", config);
    let (peers, configs) = config.apps.into_iter().collect::<(Vec<_>, Vec<_>)>();
    let mut builder = gateway::builder(
        tcp::Builder::build(
            configs
                .into_iter()
                .map(|app| {
                    (
                        app.name,
                        tcp::config::Connection::new(format!(
                            "{}:{}",
                            app.upstream.host, app.upstream.port
                        )),
                    )
                })
                .collect(),
        ),
        peer_key_from_host(env.hostname.clone()),
    )
    .with_app_port(env.http_port.unwrap_or(80))
    .with_health_check_port(env.healthcheck_port.unwrap_or(9000))
    .with_host(env.host.unwrap_or(IpAddr::from([127, 0, 0, 1])));
    for peer in peers.into_iter() {
        builder = builder.register_peer(peer, AnyRouterBuilder);
    }
    let certs = load_certs(env.certs_dir.join("cert.pem").as_path())?;
    let key = load_keys(env.certs_dir.join("key.pem").as_path())?;
    let key = any_supported_type(&key)?;
    let private_key = CertifiedKey::new(certs, key);
    let mut tls_resolver = ResolvesServerCertUsingSni::new();
    for domain in config.domains {
        debug!("Adding TLS config for {}", domain);
        tls_resolver.add(&domain, private_key.clone())?;
    }
    let tls_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(tls_resolver));
    let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));
    builder
        .with_tls(env.https_port.unwrap_or(443), tls_acceptor)
        .build()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_host() {
        assert_eq!(
            parse_host("appka.fapi.prod.majnet.xyz", "majnet.xyz"),
            Some(("appka", "fapi.prod"))
        );
        assert_eq!(
            parse_host("app.fapi.dev.majnet.xyz", "majnet.xyz"),
            Some(("app", "fapi.dev"))
        );
        assert_eq!(parse_host("appka.fapi.prod.majnet.com", "majnet.xyz"), None);
        assert_eq!(parse_host("fapi.dev.majnet.xyz", "majnet.xyz"), None);
        assert_eq!(parse_host("dev.majnet.xyz", "majnet.xyz"), None);
        assert_eq!(parse_host("majnet.xyz", "majnet.xyz"), None);
    }
}
