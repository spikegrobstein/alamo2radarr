use alamo2radarr::*;

fn main() {
    let config = radarr::Config::new_from_env();
    let client = radarr::Client::new(config).unwrap();

    let _health = client.health().expect("Failed to do radarr healthcheck");

    let root_folder_path = client.root_folder()
        .expect("Failed to get root folder")
        .data;

    let root_folder_path = &root_folder_path[0].path;

    eprintln!("Radarr online! (root folder: {})", root_folder_path);

    // now let's fetch all of the alamo movies
    let presentations = fetch_all_alamo_films().expect("Failed to get films from ADC");

    eprintln!("Got back {} films", presentations.len());

    // now search radarr
    for pres in presentations {
        if let Ok(results) = client.search(&pres.show.title) {
            let results = results.data;
            let results_count = results.len();
            let best_results = match best_matches(&pres.show.title, *results) {
                Some(r) => r,
                None => {
                    eprintln!("Found no results for {}", pres.show.title);
                    continue;
                },
            };

            eprintln!("Got {}/{} results for {}", best_results.len(), results_count, pres.show.title);

            for result in best_results {
                let mut payload = match radarr::AddMoviePayload::from_movie_response(&result) {
                    Some(payload) => payload,
                    None => {
                        eprintln!("Cannot create movie payload.");
                        continue;
                    }
                };
                payload.set_search_for_movie(true);
                payload.set_monitored(true);
                payload.set_root_folder_path(root_folder_path);

                match client.add_movie(&payload) {
                    Ok(_) => eprintln!("Added movie: {}", pres.show.title),
                    Err(error) => {
                        eprintln!("Failed to add movie: {}: {}", pres.show.title, error);
                        continue;
                    }
                }
            }
        } else {
            eprintln!("Failed to search for {}", pres.show.title);
        }
    }
}

