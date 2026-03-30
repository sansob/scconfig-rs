use rust_cloud_config_client::{BootstrapConfig, Error};
use std::time::Duration;
use temp_env::with_vars;

#[test]
fn reads_bootstrap_settings_from_environment() {
    with_vars(
        [
            (
                BootstrapConfig::SERVER_URL_ENV,
                Some("http://localhost:8888"),
            ),
            (BootstrapConfig::APPLICATION_ENV, Some("inventory-service")),
            (BootstrapConfig::PROFILES_ENV, Some("dev,aws")),
            (BootstrapConfig::LABEL_ENV, Some("main")),
            (BootstrapConfig::INSECURE_TLS_ENV, Some("true")),
            (BootstrapConfig::TIMEOUT_SECONDS_ENV, Some("15")),
        ],
        || {
            let bootstrap = BootstrapConfig::from_env().expect("bootstrap config should parse");
            assert_eq!(
                bootstrap,
                BootstrapConfig::new("http://localhost:8888", "inventory-service", ["dev", "aws"])
                    .expect("bootstrap config should build")
                    .label("main")
                    .danger_accept_invalid_tls(true)
                    .timeout(Duration::from_secs(15))
            );
        },
    );
}

#[test]
fn defaults_profile_to_default_when_missing() {
    with_vars(
        [
            (
                BootstrapConfig::SERVER_URL_ENV,
                Some("http://localhost:8888"),
            ),
            (BootstrapConfig::APPLICATION_ENV, Some("inventory-service")),
            (BootstrapConfig::PROFILES_ENV, None),
        ],
        || {
            let bootstrap = BootstrapConfig::from_env().expect("bootstrap config should parse");
            assert_eq!(bootstrap.profiles(), &["default".to_string()]);
        },
    );
}

#[test]
fn rejects_conflicting_auth_environment_variables() {
    with_vars(
        [
            (
                BootstrapConfig::SERVER_URL_ENV,
                Some("http://localhost:8888"),
            ),
            (BootstrapConfig::APPLICATION_ENV, Some("inventory-service")),
            (BootstrapConfig::PROFILES_ENV, Some("dev")),
            (BootstrapConfig::USERNAME_ENV, Some("user")),
            (BootstrapConfig::PASSWORD_ENV, Some("pass")),
            (BootstrapConfig::BEARER_TOKEN_ENV, Some("token")),
        ],
        || {
            let error = BootstrapConfig::from_env().expect_err("bootstrap config should fail");
            assert!(matches!(error, Error::InvalidBootstrapConfiguration(_)));
        },
    );
}

#[test]
fn rejects_invalid_insecure_tls_environment_variable() {
    with_vars(
        [
            (
                BootstrapConfig::SERVER_URL_ENV,
                Some("http://localhost:8888"),
            ),
            (BootstrapConfig::APPLICATION_ENV, Some("inventory-service")),
            (BootstrapConfig::INSECURE_TLS_ENV, Some("sometimes")),
        ],
        || {
            let error = BootstrapConfig::from_env().expect_err("bootstrap config should fail");
            assert!(matches!(
                error,
                Error::InvalidEnvironmentVariable {
                    name: BootstrapConfig::INSECURE_TLS_ENV,
                    ..
                }
            ));
        },
    );
}
