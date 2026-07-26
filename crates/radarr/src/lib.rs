//! Focused models, payload builders, and blocking access to the Radarr v3 API.
#![warn(missing_docs)]

mod add_movie_payload;
mod client;
mod error;
mod root_folder;
mod search_result;

pub use add_movie_payload::{AddMoviePayload, AddMoviePayloadBuilder, AddOptions};
pub use client::{Client, ClientBuilder};
pub use error::Error;
pub use root_folder::RootFolder;
pub use search_result::{AlternativeTitle, SearchResult};
