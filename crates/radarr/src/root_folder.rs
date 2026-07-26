use serde::Deserialize;

/// A root folder configured in Radarr.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RootFolder {
    /// The filesystem path where Radarr stores movies.
    pub path: String,
}
