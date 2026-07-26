use std::time::Duration;

use reqwest::{blocking::Client as HttpClient, header::HeaderValue};
use serde::Deserialize;
use url::Url;

use crate::{AddMoviePayload, Error, RootFolder, SearchResult};

/// A blocking client for the Radarr v3 API.
#[derive(Clone, Debug)]
pub struct Client {
    http: HttpClient,
    api_base: Url,
    api_token: HeaderValue,
}

impl Client {
    /// Creates a builder using Radarr's conventional local URL and a 30-second timeout.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Searches Radarr for movies matching `term`.
    pub fn search(&self, term: &str) -> Result<Vec<SearchResult>, Error> {
        let url = self.endpoint("movie/lookup")?;
        self.get(url, &[("term", term)])
    }

    /// Fetches the root folders configured in Radarr.
    pub fn root_folders(&self) -> Result<Vec<RootFolder>, Error> {
        self.get(self.endpoint("rootfolder")?, &[])
    }

    /// Adds a movie using a validated payload.
    pub fn add_movie(&self, movie: &AddMoviePayload) -> Result<(), Error> {
        let response = self
            .http
            .post(self.endpoint("movie")?)
            .header("X-Api-Key", self.api_token.clone())
            .json(movie)
            .send()?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(api_error(response))
    }

    fn endpoint(&self, path: &str) -> Result<Url, Error> {
        Ok(self.api_base.join(path)?)
    }

    fn get<T>(&self, url: Url, query: &[(&str, &str)]) -> Result<T, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        let response = self
            .http
            .get(url)
            .header("X-Api-Key", self.api_token.clone())
            .query(query)
            .send()?;
        if !response.status().is_success() {
            return Err(api_error(response));
        }
        Ok(response.json()?)
    }
}

fn api_error(response: reqwest::blocking::Response) -> Error {
    let status = response.status();
    let body = response
        .text()
        .unwrap_or_else(|error| format!("unable to read response body: {error}"));
    Error::Api { status, body }
}

/// Configures and constructs a Radarr [`Client`].
#[derive(Clone, Debug)]
pub struct ClientBuilder {
    api_token: Option<String>,
    base_url: String,
    timeout: Duration,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            api_token: None,
            base_url: "http://localhost:7878/".into(),
            timeout: Duration::from_secs(30),
        }
    }
}

impl ClientBuilder {
    /// Sets the API token sent in the `X-Api-Key` header.
    pub fn api_token(mut self, token: impl Into<String>) -> Self {
        self.api_token = Some(token.into());
        self
    }

    /// Overrides the Radarr server's base URL.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Sets the timeout applied to each HTTP request.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Validates the configuration and constructs the client.
    pub fn build(self) -> Result<Client, Error> {
        let token = self
            .api_token
            .filter(|token| !token.is_empty())
            .ok_or(Error::MissingApiToken)?;
        let mut base_url = Url::parse(&self.base_url)?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let api_base = base_url.join("api/v3/")?;
        let http = HttpClient::builder().timeout(self.timeout).build()?;
        Ok(Client {
            http,
            api_base,
            api_token: HeaderValue::from_str(&token)?,
        })
    }
}
