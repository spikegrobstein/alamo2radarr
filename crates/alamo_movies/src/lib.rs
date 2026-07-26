//! Models and blocking API access for Alamo Drafthouse schedules.
#![warn(missing_docs)]

mod client;
mod error;
mod market;
mod presentation;

pub use client::{Client, ClientBuilder};
pub use error::Error;
pub use market::Market;
pub use presentation::{Presentation, Show};
