use scconfig_rs::{EnvironmentRequest, SpringConfigClient};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AppConfig {
    server: ServerConfig,
    features: FeatureFlags,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    port: u16,
}

#[derive(Debug, Deserialize)]
struct FeatureFlags {
    enabled: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SpringConfigClient::builder("http://localhost:8888")?
        .default_label("main")
        .build()?;

    let request = EnvironmentRequest::new("inventory-service", ["dev"])?;
    let config: AppConfig = client.fetch_typed(&request).await?;

    println!("server.port={}", config.server.port);
    println!("features.enabled={}", config.features.enabled);
    println!("{config:#?}");
    Ok(())
}
