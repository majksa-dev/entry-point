use async_trait::async_trait;
use essentials::{debug, info};
use gateway::http::response::ResponseBody;
use gateway::http::HeaderMapExt;
use gateway::{ReadResponse, Request, Response, WriteHalf, WriteRequest};
use http::header;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use serde_json::json;
use std::fs::read_to_string;
use std::io;
use std::net::SocketAddr;
use std::process::Child;
use std::{env, sync::Arc};
use testing_utils::fs::fixture::ChildPath;
use testing_utils::fs::prelude::{FileTouch, FileWriteStr, PathChild, PathCreateDir};
use testing_utils::fs::TempDir;
use testing_utils::{fs, server_cmd};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;
use wiremock::Respond;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

pub const HOSTNAME: &str = "majnet.xyz";
pub const ID: &str = "papi.prod";
pub const DOMAIN: &str = "hello.papi.prod.majnet.xyz";

fn single_server_config(app_port: u16) -> serde_json::Value {
    json!({
        "apps": {
            ID: {
                "upstream": {
                    "host": "127.0.0.1",
                    "port": app_port
                }
            }
        },
        "domains": [DOMAIN]
    })
}

pub struct Context {
    app: u16,
    connector: TlsConnector,
    _origin_server: MockServer,
    _app_server: Child,
}

#[derive(Debug)]
pub struct Body(pub String);

#[async_trait]
impl ResponseBody for Body {
    async fn read_all(self: Box<Self>, _len: usize) -> io::Result<String> {
        Ok(self.0)
    }

    async fn copy_to<'a>(
        &mut self,
        writer: &'a mut WriteHalf,
        _length: Option<usize>,
    ) -> io::Result<()> {
        writer.write_all(self.0.as_bytes()).await?;
        Ok(())
    }
}

pub async fn run_request(request: Request, ctx: &Context) -> Response {
    let stream = TcpStream::connect(&format!("127.0.0.1:{}", ctx.app))
        .await
        .unwrap();
    let domain = ServerName::try_from(DOMAIN).unwrap().to_owned();
    let mut stream = ctx.connector.connect(domain, stream).await.unwrap();
    stream.write_request(&request).await.unwrap();
    stream.flush().await.unwrap();
    // stream.shutdown().await.unwrap();
    let (mut response, remains) = stream.read_response().await.unwrap();
    debug!(?response, "read response");
    let mut body = String::from_utf8(remains.to_vec()).unwrap();
    let length = response
        .get_content_length()
        .unwrap()
        .saturating_sub(remains.len());
    if length > 0 {
        let mut buf = vec![0; length];
        stream.read_exact(&mut buf).await.unwrap();
        body.push_str(&String::from_utf8(buf).unwrap());
    }
    debug!(?response, "read response body");
    response.set_body(Body(body));
    response
}

struct RespondWithHost;

impl Respond for RespondWithHost {
    fn respond(&self, req: &wiremock::Request) -> wiremock::ResponseTemplate {
        match req.headers.get(header::HOST) {
            Some(host) => ResponseTemplate::new(200)
                .set_body_string(host.to_str().unwrap())
                .insert_header(header::CONTENT_LENGTH, host.len()),
            None => ResponseTemplate::new(502),
        }
    }
}

pub async fn before_each() -> Context {
    if env::var("CI").is_err() {
        env::set_var("RUST_LOG", "debug");
        env::set_var("RUST_BACKTRACE", "0");
        env::set_var("APP_ENV", "d");
        essentials::install();
    }

    let temp = fs::TempDir::new().unwrap();
    let (mock_server, mock_port) = create_origin_server().await;
    let input_file = create_config(mock_port, &temp);
    let (certs_dir, connector) = setup_tls(&temp);
    let ports = testing_utils::get_random_ports(3);
    let app = server_cmd()
        .env("RUST_BACKTRACE", "full")
        .env("RUST_LOG", "debug")
        .env("HOSTNAME", HOSTNAME)
        .env("HTTP_PORT", ports[0].to_string())
        .env("HTTPS_PORT", ports[1].to_string())
        .env("HEALTHCHECK_PORT", ports[2].to_string())
        .env("CONFIG_FILE", input_file.path())
        .env("CERTS_DIR", certs_dir.path())
        .spawn()
        .unwrap();

    wait_for_server(ports[2]).await;
    info!("Server started");
    Context {
        app: ports[1],
        connector,
        _app_server: app,
        _origin_server: mock_server,
    }
}

pub async fn after_each(_ctx: ()) {}

async fn create_origin_server() -> (MockServer, u16) {
    let listener = std::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let addr = listener.local_addr().unwrap();
    let mock_server = MockServer::builder().listener(listener).start().await;
    Mock::given(method("GET"))
        .and(path("/hello"))
        .respond_with(RespondWithHost)
        .mount(&mock_server)
        .await;
    (mock_server, addr.port())
}

fn create_config(origin_port: u16, temp: &TempDir) -> ChildPath {
    let config = single_server_config(origin_port);
    let file = temp.child("config.json");
    file.touch().unwrap();
    file.write_str(&config.to_string()).unwrap();
    debug!("Provided config: {}", config.to_string());
    file
}

fn setup_tls(temp: &TempDir) -> (ChildPath, TlsConnector) {
    let subject_alt_names = vec![DOMAIN.to_string()];
    let CertifiedKey { cert, key_pair } = generate_simple_self_signed(subject_alt_names).unwrap();
    let dir = temp.child("certs");
    dir.create_dir_all().unwrap();
    let cert_file = dir.child("cert.pem");
    cert_file.write_str(&cert.pem()).unwrap();
    let key_file = dir.child("key.pem");
    key_file.write_str(&key_pair.serialize_pem()).unwrap();
    debug!("Generated cert: {}", read_to_string(cert_file).unwrap());
    debug!("Generated key: {}", read_to_string(key_file).unwrap());
    let mut root_cert_store = RootCertStore::empty();
    root_cert_store.add(cert.der().clone()).unwrap();

    let config = ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));
    (dir, connector)
}

async fn wait_for_server(health_check: u16) {
    use testing_utils::surf;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    loop {
        if let Ok(response) = surf::get(format!("http://127.0.0.1:{}", health_check)).await {
            debug!("Health check response: {:?}", response);
            if response.status() == 200 {
                break;
            }
        }
        interval.tick().await;
    }
    debug!("Health check passed");
}
