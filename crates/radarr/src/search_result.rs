use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An alternative title associated with a Radarr movie search result.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AlternativeTitle {
    /// The alternative movie title.
    pub title: String,
}

/// The subset of a Radarr movie lookup result needed to add and identify a movie.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// The Radarr resource identifier, or zero when the movie is not in the library.
    #[serde(default)]
    pub id: u32,
    /// The canonical movie title.
    pub title: String,
    /// Titles by which the movie is also known.
    #[serde(default)]
    pub alternate_titles: Vec<AlternativeTitle>,
    /// The movie's release year.
    #[serde(default)]
    pub year: u32,
    /// The movie's TMDB identifier.
    pub tmdb_id: u32,
    /// The movie's IMDb identifier, when supplied by Radarr.
    #[serde(default)]
    pub imdb_id: Option<String>,
    /// The movie's runtime in minutes, when supplied by Radarr.
    #[serde(default)]
    pub runtime: Option<u32>,
    /// Radarr's URL-safe title identifier.
    pub title_slug: String,
    /// Image metadata returned by Radarr and forwarded when adding the movie.
    #[serde(default)]
    pub images: Vec<Value>,
    /// Whether Radarr is already monitoring the movie.
    #[serde(default)]
    pub monitored: bool,
    /// Existing movie-file metadata, when Radarr already has a file for the movie.
    #[serde(default)]
    pub movie_file: Option<Value>,
}

impl SearchResult {
    /// Returns whether the result represents a movie already present in Radarr.
    pub fn is_already_added(&self) -> bool {
        self.id > 0 || self.monitored || self.movie_file.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radarr_resource_id_means_movie_is_already_added() {
        let movie = SearchResult {
            id: 42,
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
        };

        assert!(movie.is_already_added());
    }
}
