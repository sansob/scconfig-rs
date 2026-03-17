use std::time::Duration;

use reqwest::{
    Client, Url,
    header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};

use crate::{
    ConfigDocument, ConfigResource, DocumentFormat, Environment, EnvironmentFormat,
    EnvironmentRequest, Error, ResourceRequest, Result,
};

#[derive(Debug, Clone)]
enum Auth {
    Basic {
        username: String,
        password: Option<String>,
    },
    Bearer(String),
}

/// Builder for [`SpringConfigClient`].
#[derive(Debug, Clone)]
pub struct SpringConfigClientBuilder {
    base_url: Url,
    default_label: Option<String>,
    auth: Option<Auth>,
    accept_invalid_certs: bool,
    accept_invalid_hostnames: bool,
    timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    user_agent: Option<String>,
    headers: HeaderMap,
}

impl SpringConfigClientBuilder {
    /// Sets a default label used when a request does not provide one explicitly.
    pub fn default_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into().trim().to_string();
        self.default_label = if label.is_empty() { None } else { Some(label) };
        self
    }

    /// Configures HTTP Basic authentication.
    pub fn basic_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.auth = Some(Auth::Basic {
            username: username.into(),
            password: Some(password.into()),
        });
        self
    }

    /// Configures Bearer token authentication.
    pub fn bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.auth = Some(Auth::Bearer(token.into()));
        self
    }

    /// Disables TLS certificate validation.
    ///
    /// This should only be enabled for development or controlled test environments
    /// that use self-signed or otherwise untrusted certificates.
    pub fn danger_accept_invalid_certs(mut self, enabled: bool) -> Self {
        self.accept_invalid_certs = enabled;
        self
    }

    /// Disables TLS hostname validation.
    ///
    /// This should only be enabled for development or controlled test environments
    /// where the certificate hostname does not match the requested host.
    pub fn danger_accept_invalid_hostnames(mut self, enabled: bool) -> Self {
        self.accept_invalid_hostnames = enabled;
        self
    }

    /// Disables both TLS certificate and hostname validation.
    ///
    /// This is a convenience method for local development or smoke tests against
    /// environments with broken or private TLS setups. Do not enable this in production.
    pub fn danger_accept_invalid_tls(mut self, enabled: bool) -> Self {
        self.accept_invalid_certs = enabled;
        self.accept_invalid_hostnames = enabled;
        self
    }

    /// Sets the total request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the connect timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Sets the User-Agent header.
    pub fn user_agent(mut self, value: impl Into<String>) -> Self {
        self.user_agent = Some(value.into());
        self
    }

    /// Adds a default HTTP header that will be sent with every request.
    pub fn header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Result<Self> {
        let name_string = name.as_ref().to_string();
        let value_string = value.as_ref().to_string();

        let name = HeaderName::from_bytes(name_string.as_bytes())
            .map_err(|_| Error::InvalidHeaderName(name_string.clone()))?;
        let value =
            HeaderValue::from_str(&value_string).map_err(|_| Error::InvalidHeaderValue {
                name: name_string,
                value: value_string,
            })?;

        self.headers.insert(name, value);
        Ok(self)
    }

    /// Builds the client.
    pub fn build(self) -> Result<SpringConfigClient> {
        let mut builder = Client::builder().default_headers(self.headers);

        if self.accept_invalid_certs {
            builder = builder.danger_accept_invalid_certs(true);
        }

        if self.accept_invalid_hostnames {
            builder = builder.danger_accept_invalid_hostnames(true);
        }

        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }

        if let Some(connect_timeout) = self.connect_timeout {
            builder = builder.connect_timeout(connect_timeout);
        }

        builder =
            builder.user_agent(self.user_agent.unwrap_or_else(|| {
                format!("rust-cloud-config-client/{}", env!("CARGO_PKG_VERSION"))
            }));

        let http_client = builder.build().map_err(|source| Error::Transport {
            url: self.base_url.to_string(),
            source,
        })?;

        Ok(SpringConfigClient {
            base_url: self.base_url,
            default_label: self.default_label,
            auth: self.auth,
            http_client,
        })
    }
}

/// Async Spring Cloud Config client for Rust applications.
#[derive(Debug, Clone)]
pub struct SpringConfigClient {
    base_url: Url,
    default_label: Option<String>,
    auth: Option<Auth>,
    http_client: Client,
}

impl SpringConfigClient {
    /// Creates a new builder from the Config Server base URL.
    ///
    /// The base URL may already contain a Config Server prefix such as `/config`.
    pub fn builder(base_url: impl AsRef<str>) -> Result<SpringConfigClientBuilder> {
        let base_url_string = base_url.as_ref().trim().to_string();
        let base_url = Url::parse(&base_url_string)
            .map_err(|_| Error::InvalidBaseUrl(base_url_string.clone()))?;

        if base_url.query().is_some() || base_url.fragment().is_some() {
            return Err(Error::InvalidBaseUrlShape(base_url_string));
        }

        Ok(SpringConfigClientBuilder {
            base_url,
            default_label: None,
            auth: None,
            accept_invalid_certs: false,
            accept_invalid_hostnames: false,
            timeout: None,
            connect_timeout: None,
            user_agent: None,
            headers: HeaderMap::new(),
        })
    }

    /// Fetches the Spring `Environment` JSON payload.
    pub async fn fetch_environment(&self, request: &EnvironmentRequest) -> Result<Environment> {
        let url = self.environment_url(request, None)?;
        let response = self.send(url.clone()).await?;
        let body = self.read_text(response, &url).await?;

        serde_json::from_str(&body).map_err(|source| Error::Json {
            url: url.to_string(),
            source,
        })
    }

    /// Fetches the effective configuration and deserializes it into a Rust type.
    pub async fn fetch_typed<T>(&self, request: &EnvironmentRequest) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.fetch_environment(request).await?.deserialize()
    }

    /// Fetches an alternative-format environment representation as UTF-8 text.
    pub async fn fetch_environment_as_text(
        &self,
        request: &EnvironmentRequest,
        format: EnvironmentFormat,
    ) -> Result<String> {
        let url = self.environment_url(request, Some(format))?;
        let response = self.send(url.clone()).await?;
        self.read_text(response, &url).await
    }

    /// Fetches an alternative-format environment representation and parses it into a document.
    pub async fn fetch_environment_document(
        &self,
        request: &EnvironmentRequest,
        format: EnvironmentFormat,
    ) -> Result<ConfigDocument> {
        let origin = self.environment_url(request, Some(format))?.to_string();
        let text = self.fetch_environment_as_text(request, format).await?;
        let document_format = match format {
            EnvironmentFormat::Yml | EnvironmentFormat::Yaml => DocumentFormat::Yaml,
            EnvironmentFormat::Properties => DocumentFormat::Properties,
        };

        ConfigDocument::from_text(&origin, document_format, text)
    }

    /// Fetches a resource from the plain-text Spring endpoint.
    ///
    /// The request always includes `Accept: application/octet-stream` so the same API works
    /// for both text and binary files.
    pub async fn fetch_resource(&self, request: &ResourceRequest) -> Result<ConfigResource> {
        let url = self.resource_url(request)?;
        let response = self
            .send_with_header(
                url.clone(),
                ACCEPT,
                HeaderValue::from_static("application/octet-stream"),
            )
            .await?;

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);

        let bytes = response
            .bytes()
            .await
            .map_err(|source| Error::Transport {
                url: url.to_string(),
                source,
            })?
            .to_vec();

        Ok(ConfigResource::new(
            request.path().to_string(),
            url.to_string(),
            content_type,
            bytes,
        ))
    }

    /// Fetches and parses a resource into a [`ConfigDocument`].
    pub async fn fetch_resource_document(
        &self,
        request: &ResourceRequest,
    ) -> Result<ConfigDocument> {
        self.fetch_resource(request).await?.parse()
    }

    /// Fetches a resource and deserializes it into a Rust type.
    pub async fn fetch_resource_typed<T>(&self, request: &ResourceRequest) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.fetch_resource(request).await?.deserialize()
    }

    async fn send(&self, url: Url) -> Result<reqwest::Response> {
        let request = self.apply_auth(self.http_client.get(url.clone()));
        let response = request.send().await.map_err(|source| Error::Transport {
            url: url.to_string(),
            source,
        })?;

        Self::ensure_success(url, response).await
    }

    async fn send_with_header(
        &self,
        url: Url,
        header_name: HeaderName,
        header_value: HeaderValue,
    ) -> Result<reqwest::Response> {
        let request = self
            .apply_auth(self.http_client.get(url.clone()))
            .header(header_name, header_value);
        let response = request.send().await.map_err(|source| Error::Transport {
            url: url.to_string(),
            source,
        })?;

        Self::ensure_success(url, response).await
    }

    async fn ensure_success(url: Url, response: reqwest::Response) -> Result<reqwest::Response> {
        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(Error::HttpStatus {
                status,
                url: url.to_string(),
                body,
            })
        }
    }

    async fn read_text(&self, response: reqwest::Response, url: &Url) -> Result<String> {
        let bytes = response
            .bytes()
            .await
            .map_err(|source| Error::Transport {
                url: url.to_string(),
                source,
            })?
            .to_vec();

        String::from_utf8(bytes).map_err(|source| Error::Utf8 {
            url: url.to_string(),
            source,
        })
    }

    fn apply_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.auth {
            Some(Auth::Basic { username, password }) => {
                request.basic_auth(username, password.clone())
            }
            Some(Auth::Bearer(token)) => request.bearer_auth(token),
            None => request,
        }
    }

    fn environment_url(
        &self,
        request: &EnvironmentRequest,
        format: Option<EnvironmentFormat>,
    ) -> Result<Url> {
        let mut url = self.base_url.clone();
        let error_url = url.to_string();
        let application = encode_segment(request.application());
        let profiles = encode_segment(&request.joined_profiles());
        let effective_label = request
            .label_ref()
            .or(self.default_label.as_deref())
            .map(encode_segment);

        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| Error::InvalidBaseUrl(error_url.clone()))?;

            segments.push(&application);

            match (format, effective_label.as_deref()) {
                (None, Some(label)) => {
                    segments.push(&profiles);
                    segments.push(label);
                }
                (None, None) => {
                    segments.push(&profiles);
                }
                (Some(format), Some(label)) => {
                    segments.push(&profiles);
                    segments.push(&format!("{label}{}", format.suffix()));
                }
                (Some(format), None) => {
                    segments.push(&format!("{profiles}{}", format.suffix()));
                }
            }
        }

        if format.is_some() && request.resolve_placeholders_enabled() {
            url.query_pairs_mut()
                .append_pair("resolvePlaceholders", "true");
        }

        Ok(url)
    }

    fn resource_url(&self, request: &ResourceRequest) -> Result<Url> {
        let mut url = self.base_url.clone();
        let error_url = url.to_string();
        let application = encode_segment(request.application());
        let profiles = encode_segment(&request.joined_profiles());
        let effective_label = request
            .label_ref()
            .or(self.default_label.as_deref())
            .map(encode_segment);
        let resource_segments = request.path_segments();

        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| Error::InvalidBaseUrl(error_url.clone()))?;

            segments.push(&application);
            segments.push(&profiles);

            if let Some(label) = effective_label.as_deref() {
                segments.push(label);
            }

            for segment in &resource_segments {
                segments.push(segment);
            }
        }

        if effective_label.is_none() {
            url.query_pairs_mut().append_pair("useDefaultLabel", "true");
        }

        Ok(url)
    }
}

fn encode_segment(value: &str) -> String {
    value.trim().replace('/', "(_)")
}
