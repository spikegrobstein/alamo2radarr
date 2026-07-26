//! Synchronization policy for adding Alamo Drafthouse repertory movies to Radarr.
#![warn(missing_docs)]

use std::collections::{HashMap, HashSet};

use alamo_movies::{Client as AlamoClient, Presentation};
use radarr::{AddMoviePayload, Client as RadarrClient, SearchResult};
use thiserror::Error;

/// Alamo collection slugs whose presentations are eligible for synchronization.
pub const TARGET_COLLECTIONS: &[&str] = &[
    "terror-tuesday",
    "weird-wednesday",
    "video-vortex",
    "horror-show",
    "film-club",
    "world-of-animation",
    "graveyard-shift",
    "psycho-cinema",
];

const TITLE_SUFFIXES: &[&str] = &[
    " (Dubbed)",
    " (Subtitled)",
    " (4K Restoration)",
    " - 4K RESTORATION",
    " (Director's Cut)",
    ": The Final Cut",
    " 35th Anniversary",
];
const TITLE_PREFIXES: &[&str] = &[
    "TERROR TUESDAY: ",
    "WEIRD WEDNESDAY: ",
    "VIDEO VORTEX: ",
    "HORROR SHOW: ",
    "FILM CLUB: ",
    "WORLD OF ANIMATION: ",
    "GRAVEYARD SHIFT: ",
    "PSYCHO CINEMA: ",
];

/// A normalized movie title and optional release year parsed from an Alamo title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovieTitle {
    /// The normalized title used for Radarr searches and comparisons.
    pub title: String,
    /// The release year parsed from the Alamo title, when present.
    pub year: Option<u32>,
}

/// Runtime settings controlling how movies are added to Radarr.
#[derive(Clone, Debug)]
pub struct SyncOptions {
    /// The Radarr quality profile assigned to newly added movies.
    pub quality_profile_id: u32,
    /// The destination root folder, or `None` to use Radarr's first configured root.
    pub root_folder_path: Option<String>,
    /// Whether to report additions without sending add-movie requests.
    pub dry_run: bool,
}

/// Counts the outcomes of a completed synchronization run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// The number of Alamo markets discovered.
    pub markets: usize,
    /// The total presentations returned by successfully fetched market schedules.
    pub presentations: usize,
    /// The number of distinct eligible movie titles processed.
    pub unique_titles: usize,
    /// The number of movies successfully added to Radarr.
    pub added: usize,
    /// The number of matching movies already present in Radarr.
    pub already_added: usize,
    /// The number of titles without an exact Radarr match.
    pub no_match: usize,
    /// The number of titles skipped because multiple Radarr movies matched.
    pub ambiguous: usize,
    /// The number of market, search, or add operations that failed.
    pub failed: usize,
}

/// An error that prevents a synchronization from completing successfully.
#[derive(Debug, Error)]
pub enum Error {
    /// An Alamo API operation failed.
    #[error("Alamo API error: {0}")]
    Alamo(#[from] alamo_movies::Error),
    /// A Radarr API operation failed.
    #[error("Radarr API error: {0}")]
    Radarr(#[from] radarr::Error),
    /// No explicit or Radarr-configured root folder was available.
    #[error(
        "Radarr has no root folder configured; set RADARR_ROOT_FOLDER_PATH or configure one in Radarr"
    )]
    NoRootFolder,
    /// One or more independent operations failed during an otherwise completed run.
    #[error("synchronization completed with {0} failed operation(s)")]
    PartialFailure(usize),
}

/// Removes known Alamo decorations and a trailing release year from `title`.
pub fn clean_title(title: &str) -> String {
    parse_title(title).title
}

/// Parses an Alamo display title into a normalized title and optional release year.
pub fn parse_title(title: &str) -> MovieTitle {
    let mut title = title.trim();
    if let Some(stripped) = TITLE_PREFIXES
        .iter()
        .find_map(|prefix| title.strip_prefix(prefix))
    {
        title = stripped.trim_start();
    }
    while let Some(stripped) = TITLE_SUFFIXES
        .iter()
        .find_map(|suffix| title.strip_suffix(suffix))
    {
        title = stripped.trim_end();
    }

    let (title, year) = title
        .strip_suffix(')')
        .and_then(|without_paren| without_paren.rsplit_once(" ("))
        .and_then(|(title, year)| {
            (year.len() == 4 && year.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| (title, year.parse().expect("four ASCII digits fit in u32")))
        })
        .map_or((title, None), |(title, year)| {
            (title.trim_end(), Some(year))
        });

    MovieTitle {
        title: title.to_owned(),
        year,
    }
}

/// Returns whether a presentation belongs to one of [`TARGET_COLLECTIONS`].
pub fn is_target_presentation(presentation: &Presentation) -> bool {
    presentation
        .primary_collection_slug
        .as_deref()
        .is_some_and(|slug| TARGET_COLLECTIONS.contains(&slug))
}

/// Filters eligible presentations and returns normalized, deduplicated movie titles.
pub fn unique_titles(presentations: impl IntoIterator<Item = Presentation>) -> Vec<MovieTitle> {
    let mut seen = HashSet::new();
    presentations
        .into_iter()
        .filter(is_target_presentation)
        .map(|presentation| parse_title(&presentation.show.title))
        .filter(|movie| {
            !movie.title.is_empty() && seen.insert((movie.title.to_lowercase(), movie.year))
        })
        .collect()
}

/// The exact-match outcome for a normalized title and Radarr lookup response.
#[derive(Debug, PartialEq)]
pub enum Match<'a> {
    /// No Radarr result had an exact primary or alternative title match.
    None,
    /// Exactly one distinct TMDB movie matched.
    Unique(&'a SearchResult),
    /// Multiple distinct TMDB movies matched and require manual disambiguation.
    Ambiguous(Vec<&'a SearchResult>),
}

/// Selects exact primary or alternative title matches, constrained by year when known.
pub fn best_match<'a>(term: &MovieTitle, results: &'a [SearchResult]) -> Match<'a> {
    let normalized_term = term.title.to_lowercase();
    let mut by_tmdb_id = HashMap::new();
    for result in results.iter().filter(|result| {
        (result.title.to_lowercase() == normalized_term
            || result
                .alternate_titles
                .iter()
                .any(|title| title.title.to_lowercase() == normalized_term))
            && term.year.is_none_or(|year| result.year == year)
    }) {
        by_tmdb_id.entry(result.tmdb_id).or_insert(result);
    }

    let mut matches: Vec<_> = by_tmdb_id.into_values().collect();
    matches.sort_by_key(|movie| (movie.year, movie.tmdb_id));
    match matches.as_slice() {
        [] => Match::None,
        [result] => Match::Unique(result),
        _ => Match::Ambiguous(matches),
    }
}

/// Fetches eligible Alamo presentations and synchronizes unique matches to Radarr.
///
/// Independent market, search, and add failures are recorded while later work continues.
/// The function returns [`Error::PartialFailure`] after processing when any such failures
/// occurred.
pub fn synchronize(
    alamo: &AlamoClient,
    radarr: &RadarrClient,
    options: &SyncOptions,
) -> Result<SyncReport, Error> {
    let markets = alamo.markets()?;
    let mut report = SyncReport {
        markets: markets.len(),
        ..SyncReport::default()
    };
    let mut presentations = Vec::new();

    for market in markets {
        eprintln!("Fetching Alamo schedule for {}", market.slug);
        match alamo.presentations(&market.slug) {
            Ok(mut market_presentations) => {
                report.presentations += market_presentations.len();
                presentations.append(&mut market_presentations);
            }
            Err(error) => {
                report.failed += 1;
                eprintln!("Failed to fetch {}: {error}", market.slug);
            }
        }
    }

    let titles = unique_titles(presentations);
    report.unique_titles = titles.len();
    let mut root_folder = options
        .root_folder_path
        .as_ref()
        .filter(|path| !path.trim().is_empty())
        .cloned();

    for movie_title in titles {
        let title = &movie_title.title;
        let results = match radarr.search(title) {
            Ok(results) => results,
            Err(error) => {
                report.failed += 1;
                eprintln!("Failed to search Radarr for {title}: {error}");
                continue;
            }
        };
        let movie = match best_match(&movie_title, &results) {
            Match::None => {
                report.no_match += 1;
                eprintln!("No exact Radarr match for {title}");
                continue;
            }
            Match::Ambiguous(matches) => {
                report.ambiguous += 1;
                let candidates = matches
                    .iter()
                    .map(|movie| format!("{} ({})", movie.title, movie.year))
                    .collect::<Vec<_>>()
                    .join(", ");
                eprintln!("Skipping ambiguous match for {title}: {candidates}");
                continue;
            }
            Match::Unique(movie) if movie.is_already_added() => {
                report.already_added += 1;
                continue;
            }
            Match::Unique(movie) => movie,
        };

        if options.dry_run {
            eprintln!("Would add {} ({})", movie.title, movie.year);
            continue;
        }

        if root_folder.is_none() {
            root_folder = Some(
                radarr
                    .root_folders()?
                    .into_iter()
                    .next()
                    .ok_or(Error::NoRootFolder)?
                    .path,
            );
        }
        let payload = AddMoviePayload::builder(movie)
            .quality_profile_id(options.quality_profile_id)
            .root_folder_path(root_folder.as_deref().expect("root folder was resolved"))
            .build()?;
        match radarr.add_movie(&payload) {
            Ok(()) => {
                report.added += 1;
                eprintln!("Added {} ({})", movie.title, movie.year);
            }
            Err(error) => {
                report.failed += 1;
                eprintln!("Failed to add {title}: {error}");
            }
        }
    }

    if report.failed > 0 {
        return Err(Error::PartialFailure(report.failed));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use super::*;
    use alamo_movies::Show;
    use radarr::AlternativeTitle;

    fn presentation(title: &str, collection: Option<&str>) -> Presentation {
        Presentation {
            show: Show {
                slug: String::new(),
                title: title.into(),
                certification: None,
            },
            primary_collection_slug: collection.map(Into::into),
        }
    }

    fn result(title: &str, year: u32, tmdb_id: u32) -> SearchResult {
        SearchResult {
            id: 0,
            title: title.into(),
            alternate_titles: vec![],
            year,
            tmdb_id,
            title_slug: format!("{title}-{year}"),
            images: vec![],
            monitored: false,
            movie_file: None,
        }
    }

    fn http_server(responses: Vec<&'static str>) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            responses
                .into_iter()
                .map(|body| {
                    let (mut stream, _) = listener.accept().unwrap();
                    let mut bytes = [0; 8192];
                    let length = stream.read(&mut bytes).unwrap();
                    let request = String::from_utf8_lossy(&bytes[..length]).into_owned();
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .unwrap();
                    request
                })
                .collect()
        });
        (format!("http://{address}"), handle)
    }

    #[test]
    fn cleans_repeated_known_suffixes_and_whitespace() {
        assert_eq!(clean_title("  Akira (Dubbed) (Subtitled)  "), "Akira");
        assert_eq!(clean_title("Alien (Director's Cut)"), "Alien");
        assert_eq!(clean_title("RINGU - 4K RESTORATION"), "RINGU");
        assert_eq!(
            clean_title("Terminator 2: Judgement Day 35th Anniversary"),
            "Terminator 2: Judgement Day"
        );
    }

    #[test]
    fn parses_event_prefix_and_release_year() {
        assert_eq!(
            parse_title("WEIRD WEDNESDAY: What Ever Happened to Baby Jane? (1962)"),
            MovieTitle {
                title: "What Ever Happened to Baby Jane?".into(),
                year: Some(1962),
            }
        );
    }

    #[test]
    fn filters_and_deduplicates_titles_across_markets() {
        let titles = unique_titles([
            presentation("The Thing", Some("terror-tuesday")),
            presentation("the thing", Some("weird-wednesday")),
            presentation("Up", Some("family-party")),
        ]);
        assert_eq!(
            titles,
            [MovieTitle {
                title: "The Thing".into(),
                year: None,
            }]
        );
    }

    #[test]
    fn finds_primary_and_alternative_titles() {
        let mut alternate = result("Seven Samurai", 1954, 346);
        alternate.alternate_titles = vec![AlternativeTitle {
            title: "Shichinin no Samurai".into(),
        }];
        assert!(matches!(
            best_match(
                &MovieTitle {
                    title: "shichinin no samurai".into(),
                    year: None,
                },
                &[alternate]
            ),
            Match::Unique(_)
        ));
    }

    #[test]
    fn rejects_different_movies_with_the_same_title() {
        let results = [
            result("The Thing", 1982, 1091),
            result("The Thing", 2011, 60935),
        ];
        assert!(matches!(
            best_match(
                &MovieTitle {
                    title: "The Thing".into(),
                    year: None,
                },
                &results
            ),
            Match::Ambiguous(_)
        ));
    }

    #[test]
    fn release_year_disambiguates_remakes() {
        let results = [
            result("The Thing", 1982, 1091),
            result("The Thing", 2011, 60935),
        ];
        let title = MovieTitle {
            title: "The Thing".into(),
            year: Some(1982),
        };
        assert!(matches!(best_match(&title, &results), Match::Unique(movie) if movie.year == 1982));
    }

    #[test]
    fn collapses_duplicate_results_for_the_same_movie() {
        let results = [result("Alien", 1979, 348), result("Alien", 1979, 348)];
        assert!(matches!(
            best_match(
                &MovieTitle {
                    title: "Alien".into(),
                    year: None,
                },
                &results
            ),
            Match::Unique(_)
        ));
    }

    #[test]
    fn matches_unicode_case_insensitively() {
        let results = [result("Amélie", 2001, 194)];
        let title = MovieTitle {
            title: "AMÉLIE".into(),
            year: None,
        };
        assert!(matches!(best_match(&title, &results), Match::Unique(_)));
    }

    #[test]
    fn dry_run_searches_without_fetching_root_folders_or_adding() {
        let (alamo_url, alamo_requests) = http_server(vec![
            r#"{"data":{"marketSummaries":[{"id":"market-id","name":"Austin","slug":"austin"}]}}"#,
            r#"{"data":{"presentations":[{"show":{"title":"WEIRD WEDNESDAY: The Thing (1982)"},"primaryCollectionSlug":"weird-wednesday"}]}}"#,
        ]);
        let (radarr_url, radarr_requests) = http_server(vec![
            r#"[{"title":"The Thing","year":1982,"tmdbId":1091,"titleSlug":"the-thing-1982"}]"#,
        ]);
        let alamo = AlamoClient::builder().base_url(alamo_url).build().unwrap();
        let radarr = RadarrClient::builder()
            .base_url(radarr_url)
            .api_token("secret")
            .build()
            .unwrap();

        let report = synchronize(
            &alamo,
            &radarr,
            &SyncOptions {
                quality_profile_id: 1,
                root_folder_path: None,
                dry_run: true,
            },
        )
        .unwrap();

        assert_eq!(report.unique_titles, 1);
        let requests = alamo_requests.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /s/mother/v1/page/cclamp?useUnifiedSchedule=true "));
        assert!(requests[1].starts_with("GET /s/mother/v2/schedule/market/austin "));
        let requests = radarr_requests.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("GET /api/v3/movie/lookup?term=The+Thing "));
        assert!(requests[0].to_lowercase().contains("x-api-key: secret"));
        assert!(!requests[0].lines().next().unwrap().contains("secret"));
    }

    #[test]
    fn extracts_target_movies_from_live_austin_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../alamo_movies/tests/fixtures/austin-schedule.json"
        ))
        .unwrap();
        let presentations: Vec<Presentation> =
            serde_json::from_value(fixture["data"]["presentations"].clone()).unwrap();

        let titles = unique_titles(presentations);

        assert!(
            titles
                .iter()
                .any(|movie| movie.title.eq_ignore_ascii_case("Kitten with a Whip"))
        );
        assert!(titles.contains(&MovieTitle {
            title: "What Ever Happened to Baby Jane?".into(),
            year: Some(1962),
        }));
        assert!(titles.iter().any(|movie| movie.title == "The Dead Pit"));
    }
}
