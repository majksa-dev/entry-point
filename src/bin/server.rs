use essentials::info;
use template_rust_app::env::Env;

fn main() {
    essentials::install();
    let env = Env::new().unwrap();
    info!("Server running on port: {}", env.port);
}
