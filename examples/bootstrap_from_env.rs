use scconfig_rs::BootstrapConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AppConfig {
    server: ServerConfig,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap = BootstrapConfig::from_env()?;
    let config: AppConfig = bootstrap.load_typed().await?;

    println!("server.port={}", config.server.port);
    Ok(())
}
