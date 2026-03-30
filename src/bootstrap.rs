use std::{env, time::Duration};

use serde::de::DeserializeOwned;

use crate::{Environment, EnvironmentRequest, Error, Result, SpringConfigClient};

/// Environment-driven bootstrap settings for Spring Cloud Config Server.
///
/// This is intended for real service startup paths where configuration should be
/// loaded from environment variables instead of hard-coded values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapConfig {
    server_url: String,
    application: String,
    profiles: Vec<String>,
    label: Option<String>,
    username: Option<String>,
    password: Option<String>,
    bearer_token: Option<String>,
    accept_invalid_tls: bool,
    timeout: Option<Duration>,
}

impl BootstrapConfig {
    /// Environment variable used for the Config Server base URL.
    pub const SERVER_URL_ENV: &'static str = "SPRING_CONFIG_SERVER_URL";
    /// Environment variable used for the application name.
    pub const APPLICATION_ENV: &'static str = "SPRING_APPLICATION_NAME";
    /// Environment variable used for the active profile list.
    pub const PROFILES_ENV: &'static str = "SPRING_PROFILES_ACTIVE";
    /// Environment variable used for the config label.
    pub const LABEL_ENV: &'static str = "SPRING_CONFIG_LABEL";
    /// Environment variable used for the Config Server username.
    pub const USERNAME_ENV: &'static str = "SPRING_CONFIG_USERNAME";
    /// Environment variable used for the Config Server password.
    pub const PASSWORD_ENV: &'static str = "SPRING_CONFIG_PASSWORD";
    /// Environment variable used for a Config Server bearer token.
    pub const BEARER_TOKEN_ENV: &'static str = "SPRING_CONFIG_BEARER_TOKEN";
    /// Environment variable used to disable TLS certificate and hostname validation.
    pub const INSECURE_TLS_ENV: &'static str = "SPRING_CONFIG_INSECURE_TLS";
    /// Environment variable used for request timeout in seconds.
    pub const TIMEOUT_SECONDS_ENV: &'static str = "SPRING_CONFIG_TIMEOUT_SECS";

    /// Creates a bootstrap configuration explicitly.
    pub fn new<A, S, I, P>(server_url: A, application: S, profiles: I) -> Result<Self>
    where
        A: Into<String>,
        S: Into<String>,
        I: IntoIterator<Item = P>,
        P: Into<String>,
    {
        Ok(Self {
            server_url: server_url.into().trim().to_string(),
            application: application.into().trim().to_string(),
            profiles: profiles
                .into_iter()
                .map(Into::into)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect(),
            label: None,
            username: None,
            password: None,
            bearer_token: None,
            accept_invalid_tls: false,
            timeout: None,
        }
        .validate()?)
    }

    /// Builds a bootstrap configuration from environment variables.
    ///
    /// Required:
    /// - `SPRING_CONFIG_SERVER_URL`
    /// - `SPRING_APPLICATION_NAME`
    ///
    /// Optional:
    /// - `SPRING_PROFILES_ACTIVE` defaults to `default`
    /// - `SPRING_CONFIG_LABEL`
    /// - `SPRING_CONFIG_USERNAME`
    /// - `SPRING_CONFIG_PASSWORD`
    /// - `SPRING_CONFIG_BEARER_TOKEN`
    /// - `SPRING_CONFIG_INSECURE_TLS`
    /// - `SPRING_CONFIG_TIMEOUT_SECS`
    pub fn from_env() -> Result<Self> {
        let server_url = required_env(Self::SERVER_URL_ENV)?;
        let application = required_env(Self::APPLICATION_ENV)?;
        let profiles = optional_env(Self::PROFILES_ENV)
            .map(split_profiles)
            .filter(|profiles| !profiles.is_empty())
            .unwrap_or_else(|| vec!["default".to_string()]);
        let label = optional_env(Self::LABEL_ENV);
        let username = optional_env(Self::USERNAME_ENV);
        let password = optional_env(Self::PASSWORD_ENV);
        let bearer_token = optional_env(Self::BEARER_TOKEN_ENV);
        let accept_invalid_tls = optional_env(Self::INSECURE_TLS_ENV)
            .map(parse_env_bool)
            .transpose()?
            .unwrap_or(false);
        let timeout = optional_env(Self::TIMEOUT_SECONDS_ENV)
            .map(parse_timeout_seconds)
            .transpose()?;

        Self {
            server_url,
            application,
            profiles,
            label,
            username,
            password,
            bearer_token,
            accept_invalid_tls,
            timeout,
        }
        .validate()
    }

    /// Sets the config label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        let label = label.into().trim().to_string();
        self.label = if label.is_empty() { None } else { Some(label) };
        self
    }

    /// Sets HTTP Basic authentication.
    pub fn basic_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self.bearer_token = None;
        self
    }

    /// Sets Bearer token authentication.
    pub fn bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self.username = None;
        self.password = None;
        self
    }

    /// Disables both TLS certificate and hostname validation for Config Server requests.
    ///
    /// This should only be enabled for development or controlled test environments.
    pub fn danger_accept_invalid_tls(mut self, enabled: bool) -> Self {
        self.accept_invalid_tls = enabled;
        self
    }

    /// Sets the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Returns the Config Server base URL.
    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    /// Returns the Spring application name.
    pub fn application(&self) -> &str {
        &self.application
    }

    /// Returns the active profiles.
    pub fn profiles(&self) -> &[String] {
        &self.profiles
    }

    /// Returns the configured label, when set.
    pub fn label_ref(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Builds a [`SpringConfigClient`] from the bootstrap settings.
    pub fn build_client(&self) -> Result<SpringConfigClient> {
        let mut builder = SpringConfigClient::builder(&self.server_url)?;

        if self.accept_invalid_tls {
            builder = builder.danger_accept_invalid_tls(true);
        }

        if let Some(label) = &self.label {
            builder = builder.default_label(label);
        }

        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }

        if let Some(token) = &self.bearer_token {
            builder = builder.bearer_auth(token);
        } else if let Some(username) = &self.username {
            builder = builder.basic_auth(username, self.password.clone().unwrap_or_default());
        }

        builder.build()
    }

    /// Builds an [`EnvironmentRequest`] from the bootstrap settings.
    pub fn environment_request(&self) -> Result<EnvironmentRequest> {
        let mut request = EnvironmentRequest::new(&self.application, self.profiles.clone())?;
        if let Some(label) = &self.label {
            request = request.label(label.clone());
        }
        Ok(request)
    }

    /// Loads the raw Spring `Environment`.
    pub async fn load_environment(&self) -> Result<Environment> {
        let client = self.build_client()?;
        let request = self.environment_request()?;
        client.fetch_environment(&request).await
    }

    /// Loads typed configuration directly from Spring Config Server.
    pub async fn load_typed<T>(&self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let client = self.build_client()?;
        let request = self.environment_request()?;
        client.fetch_typed(&request).await
    }

    fn validate(mut self) -> Result<Self> {
        self.server_url = self.server_url.trim().to_string();
        self.application = self.application.trim().to_string();
        self.profiles = self
            .profiles
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
        self.label = self
            .label
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        if self.server_url.is_empty() {
            return Err(Error::MissingEnvironmentVariable {
                name: Self::SERVER_URL_ENV,
            });
        }

        if self.application.is_empty() {
            return Err(Error::MissingEnvironmentVariable {
                name: Self::APPLICATION_ENV,
            });
        }

        if self.profiles.is_empty() {
            return Err(Error::InvalidBootstrapConfiguration(
                "at least one profile must be provided".to_string(),
            ));
        }

        if self.bearer_token.is_some() && (self.username.is_some() || self.password.is_some()) {
            return Err(Error::InvalidBootstrapConfiguration(
                "basic authentication and bearer authentication are mutually exclusive".to_string(),
            ));
        }

        Ok(self)
    }
}

fn required_env(name: &'static str) -> Result<String> {
    optional_env(name).ok_or(Error::MissingEnvironmentVariable { name })
}

fn optional_env(name: &'static str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn split_profiles(value: String) -> Vec<String> {
    value
        .split(',')
        .map(|profile| profile.trim().to_string())
        .filter(|profile| !profile.is_empty())
        .collect()
}

fn parse_timeout_seconds(value: String) -> Result<Duration> {
    let seconds = value
        .parse::<u64>()
        .map_err(|_| Error::InvalidEnvironmentVariable {
            name: BootstrapConfig::TIMEOUT_SECONDS_ENV,
            reason: "expected an unsigned integer",
            value: value.clone(),
        })?;

    Ok(Duration::from_secs(seconds))
}

fn parse_env_bool(value: String) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => Ok(true),
        "0" | "false" | "no" => Ok(false),
        _ => Err(Error::InvalidEnvironmentVariable {
            name: BootstrapConfig::INSECURE_TLS_ENV,
            reason: "expected true, false, yes, no, 1, or 0",
            value,
        }),
    }
}
