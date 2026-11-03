use std::time::Duration;

use reqwest::{blocking::Client as HttpClient, header::HeaderValue, redirect::Policy};
use serde::Deserialize;
use url::Url;

use crate::{AddMoviePayload, Error, RootFolder, SearchResult};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddMovieResponse {
    id: u32,
    tmdb_id: u32,
}

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
        if !response.status().is_success() {
            return Err(api_error(response));
        }
        let added: AddMovieResponse = response.json()?;
        if added.id == 0 || added.tmdb_id != movie.tmdb_id() {
            return Err(Error::UnexpectedAddResponse {
                expected_tmdb_id: movie.tmdb_id(),
                actual_tmdb_id: added.tmdb_id,
                id: added.id,
            });
        }
        Ok(())
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
        let http = HttpClient::builder()
            .timeout(self.timeout)
            .redirect(Policy::none())
            .build()?;
        Ok(Client {
            http,
            api_base,
            api_token: HeaderValue::from_str(&token)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use reqwest::StatusCode;

    use super::*;

    fn movie() -> SearchResult {
        SearchResult {
            id: 0,
            title: "The Thing".into(),
            alternate_titles: vec![],
            year: 1982,
            tmdb_id: 1091,
            imdb_id: Some("tt0084787".into()),
            runtime: Some(109),
            title_slug: "the-thing-1982".into(),
            images: vec![],
            monitored: false,
            movie_file: None,
        }
    }

    fn payload() -> AddMoviePayload {
        AddMoviePayload::builder(&movie())
            .root_folder_path("/movies")
            .build()
            .unwrap()
    }

    fn server(response: impl Into<String>) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let response = response.into();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = [0; 8192];
            let length = stream.read(&mut bytes).unwrap();
            let request = String::from_utf8_lossy(&bytes[..length]).into_owned();
            stream.write_all(response.as_bytes()).unwrap();
            request
        });
        (format!("http://{address}"), handle)
    }

    fn client(base_url: String) -> Client {
        Client::builder()
            .base_url(base_url)
            .api_token("secret")
            .build()
            .unwrap()
    }

    #[test]
    fn add_movie_rejects_redirects() {
        let (base_url, request) = server(
            "HTTP/1.1 301 Moved Permanently\r\nLocation: https://radarr.example.test/api/v3/movie\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );

        let error = client(base_url).add_movie(&payload()).unwrap_err();

        assert!(
            matches!(error, Error::Api { status, .. } if status == StatusCode::MOVED_PERMANENTLY)
        );
        assert!(request.join().unwrap().starts_with("POST /api/v3/movie "));
    }

    #[test]
    fn add_movie_rejects_success_without_added_movie() {
        let body = "[]";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let (base_url, request) = server(response);

        assert!(client(base_url).add_movie(&payload()).is_err());
        request.join().unwrap();
    }

    #[test]
    fn add_movie_accepts_confirmed_radarr_resource() {
        let body = r#"{"id":42,"tmdbId":1091}"#;
        let response = format!(
            "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let (base_url, request) = server(response);

        client(base_url).add_movie(&payload()).unwrap();
        request.join().unwrap();
    }
}
