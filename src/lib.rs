use std::error::Error;

use alamo_movies::film::Film;
use alamo_movies::cinema::Cinema;

pub fn fetch_all_alamo_films() -> Result<Vec<Film>, Box<dyn Error>> {
    let cinemas = Cinema::list();

    let films = cinemas.iter()
        .flat_map(|cinema| {
            eprintln!("Fetching cinema data for {}", cinema.slug);
            let data = Cinema::get_calendar_data(&cinema.id).expect("Failed to get data");
            let (_cinema, films) = Cinema::from_calendar_data(&data).expect("Failed to get films");

            films
        })
        .filter(|film| {
            film.show_type.to_lowercase() == "terror tuesday"
                || film.show_type.to_lowercase() == "weird wednesday"
                || film.show_type.to_lowercase() == "video vortex"
                || film.show_type.to_lowercase() == "horror show"
                || film.show_type.to_lowercase() == "film club"
        })
        .collect();

    Ok(films)
}

pub fn best_matches(term: &str, results: Vec<radarr::SearchResult>) -> Option<Vec<radarr::SearchResult>> {
    let matches: Vec<radarr::SearchResult> = results.into_iter()
        .filter(|result| {
            result.title.to_lowercase() == term.to_lowercase() 
                || result.alternative_titles.iter()
                    .any(|title| title.to_lowercase() == term.to_lowercase())
        })
        .collect();

    if matches.is_empty() {
        return None;
    }

    Some(matches)
}

