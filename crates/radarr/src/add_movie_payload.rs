use serde::Serialize;
use serde_json::Value;

use crate::{Error, SearchResult};

/// Options controlling Radarr's behavior after adding a movie.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddOptions {
    search_for_movie: bool,
    monitor: &'static str,
}

/// A request body for Radarr's add-movie endpoint.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddMoviePayload {
    title: String,
    quality_profile_id: u32,
    title_slug: String,
    images: Vec<Value>,
    tmdb_id: u32,
    year: u32,
    root_folder_path: String,
    monitored: bool,
    add_options: AddOptions,
}

impl AddMoviePayload {
    /// Creates a payload builder initialized from a movie lookup result.
    pub fn builder(movie: &SearchResult) -> AddMoviePayloadBuilder<'_> {
        AddMoviePayloadBuilder {
            movie,
            quality_profile_id: 1,
            root_folder_path: None,
            monitored: true,
            search_for_movie: true,
        }
    }
}

/// Builds and validates an [`AddMoviePayload`].
#[derive(Debug)]
pub struct AddMoviePayloadBuilder<'a> {
    movie: &'a SearchResult,
    quality_profile_id: u32,
    root_folder_path: Option<String>,
    monitored: bool,
    search_for_movie: bool,
}

impl AddMoviePayloadBuilder<'_> {
    /// Sets the Radarr quality profile identifier.
    pub fn quality_profile_id(mut self, id: u32) -> Self {
        self.quality_profile_id = id;
        self
    }

    /// Sets the destination root-folder path.
    pub fn root_folder_path(mut self, path: impl Into<String>) -> Self {
        self.root_folder_path = Some(path.into());
        self
    }

    /// Sets whether Radarr should monitor the movie.
    pub fn monitored(mut self, monitored: bool) -> Self {
        self.monitored = monitored;
        self
    }

    /// Sets whether Radarr should search for the movie immediately after adding it.
    pub fn search_for_movie(mut self, search: bool) -> Self {
        self.search_for_movie = search;
        self
    }

    /// Validates the required settings and constructs the payload.
    pub fn build(self) -> Result<AddMoviePayload, Error> {
        let root_folder_path = self
            .root_folder_path
            .filter(|path| !path.trim().is_empty())
            .ok_or(Error::MissingRootFolder)?;
        Ok(AddMoviePayload {
            title: self.movie.title.clone(),
            quality_profile_id: self.quality_profile_id,
            title_slug: self.movie.title_slug.clone(),
            images: self.movie.images.clone(),
            tmdb_id: self.movie.tmdb_id,
            year: self.movie.year,
            root_folder_path,
            monitored: self.monitored,
            add_options: AddOptions {
                search_for_movie: self.search_for_movie,
                monitor: "movieOnly",
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn movie() -> SearchResult {
        SearchResult {
            id: 0,
            title: "The Thing".into(),
            alternate_titles: vec![],
            year: 1982,
            tmdb_id: 1091,
            title_slug: "the-thing-1982".into(),
            images: vec![],
            monitored: false,
            movie_file: None,
        }
    }

    #[test]
    fn payload_builder_uses_radarr_field_names() {
        let payload = AddMoviePayload::builder(&movie())
            .quality_profile_id(4)
            .root_folder_path("/movies")
            .build()
            .unwrap();
        let json = serde_json::to_value(payload).unwrap();

        assert_eq!(json["qualityProfileId"], 4);
        assert_eq!(json["rootFolderPath"], "/movies");
        assert_eq!(json["addOptions"]["searchForMovie"], true);
        assert!(json.get("path").is_none());
    }

    #[test]
    fn payload_builder_requires_root_folder() {
        assert!(matches!(
            AddMoviePayload::builder(&movie()).build(),
            Err(Error::MissingRootFolder)
        ));
    }
}
