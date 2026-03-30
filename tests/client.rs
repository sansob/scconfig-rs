use scconfig_rs::{
    ConfigDocument, EnvironmentFormat, EnvironmentRequest, ResourceRequest, SpringConfigClient,
};
use serde::Deserialize;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path, query_param},
};

#[derive(Debug, Deserialize)]
struct AppConfig {
    server: ServerConfig,
    features: FeatureFlags,
    replicas: Vec<Replica>,
}

#[derive(Debug, Deserialize)]
struct ServerConfig {
    port: u16,
}

#[derive(Debug, Deserialize)]
struct FeatureFlags {
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct Replica {
    id: String,
    url: Option<String>,
}

#[tokio::test]
async fn fetches_environment_and_binds_typed_config() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "name": "inventory-service",
        "profiles": ["dev", "aws"],
        "label": "main",
        "propertySources": [
            {
                "name": "inventory-service-dev.yml",
                "source": {
                    "server.port": "9090",
                    "features.enabled": "true",
                    "replicas[0].id": "blue"
                }
            },
            {
                "name": "application.yml",
                "source": {
                    "server.port": "8080",
                    "features.enabled": "false",
                    "replicas[0].url": "https://blue.internal",
                    "replicas[1].id": "green",
                    "replicas[1].url": "https://green.internal"
                }
            }
        ]
    });

    Mock::given(method("GET"))
        .and(path("/inventory-service/dev,aws/main"))
        .and(header(
            "authorization",
            "Basic Y29uZmlnLXVzZXI6Y29uZmlnLXBhc3M=",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(2)
        .mount(&server)
        .await;

    let client = SpringConfigClient::builder(server.uri())
        .unwrap()
        .basic_auth("config-user", "config-pass")
        .build()
        .unwrap();

    let request = EnvironmentRequest::new("inventory-service", ["dev", "aws"])
        .unwrap()
        .label("main");

    let environment = client.fetch_environment(&request).await.unwrap();
    assert_eq!(environment.name, "inventory-service");

    let typed: AppConfig = client.fetch_typed(&request).await.unwrap();
    assert_eq!(typed.server.port, 9090);
    assert!(typed.features.enabled);
    assert_eq!(typed.replicas[0].id, "blue");
    assert_eq!(
        typed.replicas[0].url.as_deref(),
        Some("https://blue.internal")
    );
    assert_eq!(typed.replicas[1].id, "green");
}

#[tokio::test]
async fn fetches_yaml_environment_with_placeholder_resolution() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/inventory-service/dev/main.yaml"))
        .and(query_param("resolvePlaceholders", "true"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/x-yaml")
                .set_body_string("server:\n  port: 9090\n"),
        )
        .mount(&server)
        .await;

    let client = SpringConfigClient::builder(server.uri())
        .unwrap()
        .build()
        .unwrap();
    let request = EnvironmentRequest::new("inventory-service", ["dev"])
        .unwrap()
        .label("main")
        .resolve_placeholders(true);

    let yaml = client
        .fetch_environment_as_text(&request, EnvironmentFormat::Yaml)
        .await
        .unwrap();
    assert!(yaml.contains("9090"));

    let document = client
        .fetch_environment_document(&request, EnvironmentFormat::Yaml)
        .await
        .unwrap();

    match document {
        ConfigDocument::Yaml(value) => {
            assert_eq!(value["server"]["port"], 9090);
        }
        other => panic!("unexpected document format: {:?}", other.format()),
    }
}

#[tokio::test]
async fn uses_default_label_query_parameter_for_plain_text_when_label_is_missing() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/inventory-service/dev/nginx.conf"))
        .and(query_param("useDefaultLabel", "true"))
        .and(header("accept", "application/octet-stream"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/plain")
                .set_body_string("server_name example.com;"),
        )
        .mount(&server)
        .await;

    let client = SpringConfigClient::builder(server.uri())
        .unwrap()
        .build()
        .unwrap();
    let request = ResourceRequest::new("inventory-service", ["dev"], "nginx.conf").unwrap();

    let resource = client.fetch_resource(&request).await.unwrap();
    assert_eq!(resource.text().unwrap(), "server_name example.com;");

    match resource.parse().unwrap() {
        ConfigDocument::Text(text) => assert_eq!(text, "server_name example.com;"),
        other => panic!("unexpected document format: {:?}", other.format()),
    }
}

#[tokio::test]
async fn fetches_binary_resources_and_escapes_labels() {
    let server = MockServer::start().await;
    let bytes = vec![0_u8, 159, 146, 150];

    Mock::given(method("GET"))
        .and(path(
            "/inventory-service/dev/release(_)2026-03/keystore.p12",
        ))
        .and(header("accept", "application/octet-stream"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/octet-stream")
                .set_body_raw(bytes.clone(), "application/octet-stream"),
        )
        .mount(&server)
        .await;

    let client = SpringConfigClient::builder(server.uri())
        .unwrap()
        .build()
        .unwrap();
    let request = ResourceRequest::new("inventory-service", ["dev"], "keystore.p12")
        .unwrap()
        .label("release/2026-03");

    let resource = client.fetch_resource(&request).await.unwrap();
    assert_eq!(resource.bytes(), bytes.as_slice());

    match resource.parse().unwrap() {
        ConfigDocument::Binary(body) => assert_eq!(body, bytes),
        other => panic!("unexpected document format: {:?}", other.format()),
    }
}

#[tokio::test]
async fn fetches_toml_resources_and_detects_format() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/inventory-service/dev/main/settings.toml"))
        .and(header("accept", "application/octet-stream"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/plain")
                .set_body_string("[server]\nport = 7070\n"),
        )
        .mount(&server)
        .await;

    let client = SpringConfigClient::builder(server.uri())
        .unwrap()
        .default_label("main")
        .build()
        .unwrap();
    let request = ResourceRequest::new("inventory-service", ["dev"], "settings.toml").unwrap();

    let document = client.fetch_resource_document(&request).await.unwrap();
    match document {
        ConfigDocument::Toml(value) => assert_eq!(value["server"]["port"], 7070),
        other => panic!("unexpected document format: {:?}", other.format()),
    }
}

#[test]
fn builder_accepts_development_tls_overrides() {
    let client = SpringConfigClient::builder("https://localhost:8443")
        .unwrap()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .danger_accept_invalid_tls(true)
        .build()
        .unwrap();

    let _ = client;
}
