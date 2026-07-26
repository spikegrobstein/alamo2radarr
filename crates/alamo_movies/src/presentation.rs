use serde::{Deserialize, Serialize};

/// A scheduled presentation and its associated collection metadata.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Presentation {
    /// The movie or event being presented.
    pub show: Show,
    /// The primary Alamo collection slug assigned to the presentation.
    #[serde(default, rename = "primaryCollectionSlug")]
    pub primary_collection_slug: Option<String>,
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
}
