use std::error::Error;

use alamo_movies::presentation::Presentation;
use alamo_movies::market::{Market, MarketApiResponse};

pub fn fetch_all_alamo_films() -> Result<Vec<Presentation>, Box<dyn Error>> {
    let markets = Market::list()?;

    let films = markets.iter()
        .flat_map(|market| {
            eprintln!("Fetching market data for {}", market.slug);
            let data = Market::get_calendar_data(&market.id).expect("Failed to get data");
            let resp: MarketApiResponse = serde_json::from_str(&data).unwrap();

            resp.data.presentations
        })
        .filter(|pres| {
            pres.primary_collection_slug == Some("terror-tuesday".into())
                || pres.primary_collection_slug == Some("weird-wednesday".into())
                || pres.primary_collection_slug == Some("video-vortex".into())
                || pres.primary_collection_slug == Some("horror-show".into())
                || pres.primary_collection_slug == Some("film-club".into())
        })
        .collect();

    Ok(films)
}

pub fn best_matches(term: &str, results: Vec<radarr::SearchResult>) -> Option<Vec<radarr::SearchResult>> {
    let matches: Vec<radarr::SearchResult> = results.into_iter()
        .filter(|result| {
            result.title.to_lowercase() == term.to_lowercase() 
                || result.alternate_titles.iter()
                    .any(|title| title.title.to_lowercase() == term.to_lowercase())
        })
        .collect();

    if matches.is_empty() {
        return None;
    }

    Some(matches)
}

