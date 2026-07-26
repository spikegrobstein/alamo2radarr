use serde::{Deserialize, Serialize};

/// A scheduled presentation and its associated collection metadata.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Presentation {
    /// The URL-safe presentation identifier used by presentation-detail endpoints.
    #[serde(default)]
    pub slug: String,
    /// The movie or event being presented.
    pub show: Show,
    /// The primary Alamo collection slug assigned to the presentation.
    #[serde(default, rename = "primaryCollectionSlug")]
    pub primary_collection_slug: Option<String>,
    /// The projection and media formats advertised for the presentation.
    #[serde(default, rename = "formatSlugs")]
    pub format_slugs: Vec<String>,
}

/// A movie or event included in an Alamo presentation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Show {
    /// The URL-safe show identifier, when supplied by Alamo.
    #[serde(default)]
    pub slug: String,
    /// The display title used by Alamo.
    pub title: String,
    /// The content certification, when supplied by Alamo.
    #[serde(default)]
    pub certification: Option<String>,
    /// The movie's national release date, when supplied by Alamo.
    #[serde(default, rename = "nationalReleaseDateUtc")]
    pub national_release_date_utc: Option<String>,
    /// The movie's IMDb identifier, when supplied by Alamo.
    #[serde(default, rename = "imdbId")]
    pub imdb_id: Option<String>,
    /// The movie's runtime in minutes, when supplied by Alamo.
    #[serde(default, rename = "runtimeMinutes")]
    pub runtime_minutes: Option<u32>,
    /// Directors credited by Alamo.
    #[serde(default)]
    pub directors: Vec<String>,
}
