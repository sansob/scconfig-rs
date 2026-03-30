# rust-cloud-config-client

[![CI](https://github.com/sansob/rust-cloud-config-client/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/sansob/rust-cloud-config-client/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/rust-cloud-config-client.svg)](https://crates.io/crates/rust-cloud-config-client)
[![docs.rs](https://docs.rs/rust-cloud-config-client/badge.svg)](https://docs.rs/rust-cloud-config-client)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/sansob/rust-cloud-config-client/blob/master/LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

`rust-cloud-config-client` is a production-oriented Rust library for consuming [Spring Cloud Config Server](https://docs.spring.io/spring-cloud-config/reference/index.html) from non-Spring applications.

It is built for Rust services that need:

- typed configuration loading from the standard Spring `Environment` endpoint
- YAML and Java properties output from the alternative format endpoints
- arbitrary file retrieval through the plain-text resource endpoint
- binary-safe downloads for files such as `.p12`, `.jks`, or any other non-text asset
- a clean async API with explicit request models, error types, and documentation

## What This Library Supports

### Spring Cloud Config endpoints

- `GET /{application}/{profile}`
- `GET /{application}/{profile}/{label}`
- `GET /{application}/{profile}.yml`
- `GET /{application}/{profile}.yaml`
- `GET /{application}/{profile}.properties`
- `GET /{application}/{profile}/{label}.yml`
- `GET /{application}/{profile}/{label}.yaml`
- `GET /{application}/{profile}/{label}.properties`
- `GET /{application}/{profile}/{label}/{path}`
- `GET /{application}/{profile}/{path}?useDefaultLabel=true`

### Configuration format support

All file extensions are supported on the resource endpoint.

Built-in parsing and typed deserialization are available for:

- JSON
- YAML / YML
- TOML
- Java properties

Unknown extensions are still supported. The library returns them as UTF-8 text when possible, or as raw bytes otherwise.

## Installation

From crates.io:

```toml
[dependencies]
rust-cloud-config-client = "0.1.1"
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quick Start

```rust,no_run
use rust_cloud_config_client::{EnvironmentRequest, SpringConfigClient};
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

    println!("{config:#?}");
    Ok(())
}
```

## Alternative Format Example

```rust,no_run
use rust_cloud_config_client::{EnvironmentFormat, EnvironmentRequest, SpringConfigClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SpringConfigClient::builder("http://localhost:8888")?.build()?;

    let request = EnvironmentRequest::new("inventory-service", ["dev"])?
        .label("main")
        .resolve_placeholders(true);

    let yaml = client
        .fetch_environment_as_text(&request, EnvironmentFormat::Yaml)
        .await?;

    println!("{yaml}");
    Ok(())
}
```

## Arbitrary File Example

```rust,no_run
use rust_cloud_config_client::{ResourceRequest, SpringConfigClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SpringConfigClient::builder("http://localhost:8888")?
        .default_label("main")
        .build()?;

    let request = ResourceRequest::new("inventory-service", ["dev"], "nginx.conf")?;
    let resource = client.fetch_resource(&request).await?;

    println!("{}", resource.text()?);
    Ok(())
}
```

## Binary File Example

```rust,no_run
use rust_cloud_config_client::{ConfigDocument, ResourceRequest, SpringConfigClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SpringConfigClient::builder("http://localhost:8888")?
        .default_label("main")
        .build()?;

    let request = ResourceRequest::new("inventory-service", ["prod"], "keystore.p12")?;
    let resource = client.fetch_resource(&request).await?;

    match resource.parse()? {
        ConfigDocument::Binary(bytes) => println!("downloaded {} bytes", bytes.len()),
        other => println!("received {:?}", other.format()),
    }

    Ok(())
}
```

## Bootstrap From Environment

```rust,no_run
use rust_cloud_config_client::BootstrapConfig;
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
```

Supported bootstrap environment variables:

- `SPRING_CONFIG_SERVER_URL` required
- `SPRING_APPLICATION_NAME` required
- `SPRING_PROFILES_ACTIVE` optional, defaults to `default`
- `SPRING_CONFIG_LABEL` optional
- `SPRING_CONFIG_USERNAME` optional
- `SPRING_CONFIG_PASSWORD` optional
- `SPRING_CONFIG_BEARER_TOKEN` optional
- `SPRING_CONFIG_INSECURE_TLS` optional, defaults to `false`
- `SPRING_CONFIG_TIMEOUT_SECS` optional

When bootstrapping against development Config Server endpoints that use private or
self-signed certificates, set `SPRING_CONFIG_INSECURE_TLS=true` to disable both
certificate and hostname validation for bootstrap requests only.

## Development TLS

For local development or controlled smoke tests against self-signed or otherwise
untrusted certificates, the builder can disable TLS validation explicitly:

```rust,no_run
use rust_cloud_config_client::{EnvironmentRequest, SpringConfigClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = SpringConfigClient::builder("https://config.dev.example.org")?
        .danger_accept_invalid_tls(true)
        .build()?;

    let request = EnvironmentRequest::new("sample-api", ["dev"])?;
    let environment = client.fetch_environment(&request).await?;

    println!("{}", environment.name);
    Ok(())
}
```

Available builder methods:

- `danger_accept_invalid_certs(true)` disables certificate validation
- `danger_accept_invalid_hostnames(true)` disables hostname validation
- `danger_accept_invalid_tls(true)` disables both

These options are unsafe for production and should only be used when you fully
control the target environment.

## Design Notes

- The `Environment` endpoint is the preferred source when you want typed Rust structs.
- The library preserves Spring property-source precedence. Earlier property sources win.
- Labels containing `/` are automatically encoded as `(_)`, matching Spring Cloud Config server rules.
- Resource downloads always request `application/octet-stream`, which keeps binary retrieval safe without breaking text-based files.
- Smart scalar coercion is enabled for typed binding. Strings such as `"8080"` and `"true"` can bind to numeric and boolean fields.

## Error Handling

All fallible operations return `rust_cloud_config_client::Error`.

Errors include:

- invalid base URL or request input
- transport failures
- non-success HTTP responses with response body captured
- invalid JSON, YAML, TOML, or Java properties content
- UTF-8 decoding failures for text resources
- typed binding failures

## Included Examples

- [`examples/load_typed_config.rs`](./examples/load_typed_config.rs)
- [`examples/fetch_resource.rs`](./examples/fetch_resource.rs)
- [`examples/bootstrap_from_env.rs`](./examples/bootstrap_from_env.rs)

## References

- [Spring Cloud Config Reference](https://docs.spring.io/spring-cloud-config/reference/index.html)
- [Environment Repository](https://docs.spring.io/spring-cloud-config/reference/server/environment-repository.html)
- [Serving Alternative Formats](https://docs.spring.io/spring-cloud-config/reference/server/serving-alternative-formats.html)
- [Serving Plain Text](https://docs.spring.io/spring-cloud-config/reference/server/serving-plain-text.html)
- [Serving Binary Files](https://docs.spring.io/spring-cloud-config/reference/server/serving-binary-files.html)
