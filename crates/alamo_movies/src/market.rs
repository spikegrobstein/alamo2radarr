use serde::{Deserialize, Serialize};

/// An Alamo Drafthouse geographic market.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Market {
    /// The internal Alamo market identifier.
    pub id: String,
    /// The human-readable market name.
    pub name: String,
    /// The URL-safe market identifier used by schedule endpoints.
    pub slug: String,
    /// Whether the market is currently open for business, when supplied by Alamo.
    #[serde(default, rename = "isOpenForBusiness", alias = "is_open_for_business")]
    pub is_open_for_business: Option<bool>,
}
