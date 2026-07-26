use std::time::Duration;

use reqwest::blocking::Client as HttpClient;
use serde::Deserialize;
use url::Url;

use crate::{Error, Market, Presentation};

const DEFAULT_BASE_URL: &str = "https://drafthouse.com/";

#[derive(Debug, Deserialize)]
struct MarketListResponse {
    data: MarketListData,
}

#[derive(Debug, Deserialize)]
struct MarketListData {
    #[serde(rename = "marketSummaries")]
    market_summaries: Vec<Market>,
}

#[derive(Debug, Deserialize)]
struct ScheduleResponse {
    data: ScheduleData,
}

#[derive(Debug, Deserialize)]
struct ScheduleData {
    presentations: Vec<Presentation>,
}

#[derive(Debug, Deserialize)]
struct PresentationResponse {
    data: PresentationData,
}

#[derive(Debug, Deserialize)]
struct PresentationData {
    presentation: Presentation,
}

/// A blocking client for Alamo Drafthouse schedule endpoints.
#[derive(Clone, Debug)]
pub struct Client {
    http: HttpClient,
    base_url: Url,
}

impl Client {
    /// Creates a builder with the production Alamo URL and a 30-second timeout.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Fetches every Alamo geographic market.
    pub fn markets(&self) -> Result<Vec<Market>, Error> {
        let url = self
            .base_url
            .join("s/mother/v1/page/cclamp?useUnifiedSchedule=true")?;
        let response = self
            .http
            .get(url)
            .send()?
            .error_for_status()?
            .json::<MarketListResponse>()?;
        Ok(response.data.market_summaries)
    }

    /// Fetches all presentations scheduled in the market identified by `market_slug`.
    pub fn presentations(&self, market_slug: &str) -> Result<Vec<Presentation>, Error> {
        let mut url = self.base_url.join("s/mother/v2/schedule/market/")?;
        url.path_segments_mut()
            .expect("HTTP URLs support path segments")
            .pop_if_empty()
            .push(market_slug);
        let response = self
            .http
            .get(url)
            .send()?
            .error_for_status()?
            .json::<ScheduleResponse>()?;
        Ok(response.data.presentations)
    }

    /// Fetches full movie metadata for a presentation in a market.
    pub fn presentation(
        &self,
        market_slug: &str,
        presentation_slug: &str,
    ) -> Result<Presentation, Error> {
        let mut url = self.base_url.join("s/mother/v2/schedule/presentation/")?;
        url.path_segments_mut()
            .expect("HTTP URLs support path segments")
            .pop_if_empty()
            .push(market_slug)
            .push(presentation_slug);
        let response = self
            .http
            .get(url)
            .send()?
            .error_for_status()?
            .json::<PresentationResponse>()?;
        Ok(response.data.presentation)
    }
}

/// Configures and constructs an Alamo [`Client`].
#[derive(Clone, Debug)]
pub struct ClientBuilder {
    base_url: String,
    timeout: Duration,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            timeout: Duration::from_secs(30),
        }
    }
}

impl ClientBuilder {
    /// Overrides the Alamo API base URL.
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
        let mut base_url = Url::parse(&self.base_url)?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let http = HttpClient::builder().timeout(self.timeout).build()?;
        Ok(Client { http, base_url })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_current_market_shape() {
        let response: MarketListResponse = serde_json::from_str(
            r#"{"data":{"marketSummaries":[{"id":"1","name":"Austin","slug":"austin","isOpenForBusiness":true}]}}"#,
        )
        .unwrap();

        assert_eq!(response.data.market_summaries[0].slug, "austin");
        assert_eq!(
            response.data.market_summaries[0].is_open_for_business,
            Some(true)
        );
    }

    #[test]
    fn deserializes_minimal_schedule() {
        let response: ScheduleResponse = serde_json::from_str(
            r#"{"data":{"presentations":[{"show":{"title":"The Thing"},"primaryCollectionSlug":"terror-tuesday"}]}}"#,
        )
        .unwrap();

        assert_eq!(response.data.presentations[0].show.title, "The Thing");
    }

    #[test]
    fn deserializes_detailed_presentation_metadata() {
        let response: PresentationResponse = serde_json::from_str(
            r#"{"data":{"presentation":{"slug":"the-thing","show":{"title":"The Thing","nationalReleaseDateUtc":"1982-06-25","imdbId":"tt0084787","runtimeMinutes":109,"directors":["John Carpenter"]},"formatSlugs":["35mm"]}}}"#,
        )
        .unwrap();

        let show = response.data.presentation.show;
        assert_eq!(
            show.national_release_date_utc.as_deref(),
            Some("1982-06-25")
        );
        assert_eq!(show.imdb_id.as_deref(), Some("tt0084787"));
        assert_eq!(show.runtime_minutes, Some(109));
    }

    #[test]
    fn rejects_schedule_without_presentations() {
        assert!(serde_json::from_str::<ScheduleResponse>(r#"{"data":{}}"#).is_err());
    }

    #[test]
    fn deserializes_live_market_fixture() {
        let response: MarketListResponse =
            serde_json::from_str(include_str!("../tests/fixtures/markets.json")).unwrap();

        assert!(response.data.market_summaries.len() > 20);
        assert!(
            response
                .data
                .market_summaries
                .iter()
                .any(|market| market.slug == "austin" && market.id == "0000")
        );
    }

    #[test]
    fn deserializes_live_austin_schedule_fixture() {
        let response: ScheduleResponse =
            serde_json::from_str(include_str!("../tests/fixtures/austin-schedule.json")).unwrap();

        assert!(response.data.presentations.len() > 50);
        assert!(response.data.presentations.iter().any(|presentation| {
            presentation.primary_collection_slug.as_deref() == Some("weird-wednesday")
        }));
    }

    #[test]
    fn deserializes_live_austin_presentation_fixture() {
        let response: PresentationResponse =
            serde_json::from_str(include_str!("../tests/fixtures/austin-presentation.json"))
                .unwrap();

        assert!(!response.data.presentation.slug.is_empty());
        assert!(
            response
                .data
                .presentation
                .show
                .national_release_date_utc
                .is_some()
        );
    }
}
