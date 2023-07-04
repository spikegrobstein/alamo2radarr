use std::error::Error;

use alamo_movies::presentation::Presentation;
use alamo_movies::market::{Market, MarketApiResponse};

pub fn clean_title(title: &str) -> String {
    let suffixes = vec![
        " (Dubbed)",
        " (Subtitled)",
    ];

    let mut title = title;

    for suffix in suffixes {
        if let Some(stripped_title) = title.strip_suffix(suffix) {
            title = stripped_title;
        }
    }

    title.to_owned()
}

pub fn fetch_all_alamo_films() -> Result<Vec<Presentation>, Box<dyn Error>> {
    let markets = Market::list()?;

    let collections = vec![
        "terror-tuesday",
        "weird-wednesday",
        "video-vortex",
        "horror-show",
        "film-club",
        "world-of-animation",
        "graveyard-shift",
        "psycho-cinema",
    ];

    let films = markets.iter()
        .flat_map(|market| {
            eprintln!("Fetching market data for {}", market.slug);
            let data = Market::get_calendar_data(&market.id).expect("Failed to get data");
            let resp: MarketApiResponse = serde_json::from_str(&data).unwrap();

            resp.data.presentations
        })
        .filter(|pres| {
            if let Some(ref collection) = pres.primary_collection_slug {
                eprintln!("checking collection type: {collection}");
                collections.iter().any(|c| c == collection)
            } else {
                false
            }
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

