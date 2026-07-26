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

/// Normalized Alamo movie metadata used to identify a Radarr result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovieTitle {
    /// The normalized title used for Radarr searches and comparisons.
    pub title: String,
    /// The release year parsed from the Alamo title, when present.
    pub year: Option<u32>,
    /// The IMDb identifier supplied by Alamo, when present.
    pub imdb_id: Option<String>,
    /// The runtime supplied by Alamo, when present.
    pub runtime_minutes: Option<u32>,
    /// Directors credited by Alamo.
    pub directors: Vec<String>,
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
        imdb_id: None,
        runtime_minutes: None,
        directors: Vec::new(),
    }
}

fn movie_metadata(presentation: Presentation) -> MovieTitle {
    let mut movie = parse_title(&presentation.show.title);
    if let Some(year) = presentation
        .show
        .national_release_date_utc
        .as_deref()
        .and_then(release_year)
    {
        movie.year = Some(year);
    }
    movie.imdb_id = presentation
        .show
        .imdb_id
        .filter(|imdb_id| !imdb_id.trim().is_empty());
    movie.runtime_minutes = presentation
        .show
        .runtime_minutes
        .filter(|runtime| *runtime > 0);
    movie.directors = presentation.show.directors;
    movie
}

fn release_year(date: &str) -> Option<u32> {
    let year = date.get(..4)?.parse().ok()?;
    (year >= 1888 && year != 1900).then_some(year)
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
        .map(movie_metadata)
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

/// Selects a Radarr result by IMDb ID or by normalized title and release year.
///
/// Runtime is used only to resolve multiple title-and-year matches. A candidate without
/// either an IMDb ID or a usable release year is not selected automatically.
pub fn best_match<'a>(term: &MovieTitle, results: &'a [SearchResult]) -> Match<'a> {
    if let Some(imdb_id) = term.imdb_id.as_deref() {
        let matches = distinct_results(results.iter().filter(|result| {
            result
                .imdb_id
                .as_deref()
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(imdb_id))
        }));
        if !matches.is_empty() {
            return classify(matches);
        }
    }

    let Some(year) = term.year else {
        return Match::None;
    };
    let title_variants = title_variants(&term.title);
    let mut by_tmdb_id = HashMap::new();
    for result in results.iter().filter(|result| {
        (title_variants.contains(&result.title.to_lowercase())
            || result
                .alternate_titles
                .iter()
                .any(|title| title_variants.contains(&title.title.to_lowercase())))
            && result.year == year
    }) {
        by_tmdb_id.entry(result.tmdb_id).or_insert(result);
    }

    let mut matches: Vec<_> = by_tmdb_id.into_values().collect();
    matches.sort_by_key(|movie| (movie.year, movie.tmdb_id));
    if matches.len() > 1
        && let Some(runtime) = term.runtime_minutes
    {
        let smallest_difference = matches
            .iter()
            .filter_map(|movie| movie.runtime.map(|value| value.abs_diff(runtime)))
            .min();
        if let Some(difference) = smallest_difference.filter(|difference| *difference <= 15) {
            let closest: Vec<_> = matches
                .iter()
                .copied()
                .filter(|movie| {
                    movie
                        .runtime
                        .is_some_and(|value| value.abs_diff(runtime) == difference)
                })
                .collect();
            if closest.len() == 1 {
                return Match::Unique(closest[0]);
            }
        }
    }
    classify(matches)
}

fn distinct_results<'a>(results: impl Iterator<Item = &'a SearchResult>) -> Vec<&'a SearchResult> {
    let mut by_tmdb_id = HashMap::new();
    for result in results {
        by_tmdb_id.entry(result.tmdb_id).or_insert(result);
    }
    let mut results: Vec<_> = by_tmdb_id.into_values().collect();
    results.sort_by_key(|movie| (movie.year, movie.tmdb_id));
    results
}

fn classify(matches: Vec<&SearchResult>) -> Match<'_> {
    match matches.as_slice() {
        [] => Match::None,
        [result] => Match::Unique(result),
        _ => Match::Ambiguous(matches),
    }
}

fn title_variants(title: &str) -> HashSet<String> {
    let mut variants = HashSet::from([title.to_lowercase()]);
    if let Some((presenter, movie_title)) = title.split_once("'s ")
        && presenter.contains(' ')
    {
        variants.insert(movie_title.to_lowercase());
    }
    variants
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
    let mut scheduled_presentations = Vec::new();

    for market in markets {
        eprintln!("Fetching Alamo schedule for {}", market.slug);
        match alamo.presentations(&market.slug) {
            Ok(market_presentations) => {
                report.presentations += market_presentations.len();
                scheduled_presentations.extend(
                    market_presentations
                        .into_iter()
                        .filter(is_target_presentation)
                        .map(|presentation| (market.slug.clone(), presentation)),
                );
            }
            Err(error) => {
                report.failed += 1;
                eprintln!("Failed to fetch {}: {error}", market.slug);
            }
        }
    }

    let mut seen_presentations = HashSet::new();
    let mut presentations = Vec::new();
    for (market_slug, summary) in scheduled_presentations {
        let identity = if summary.show.slug.is_empty() {
            summary.show.title.to_lowercase()
        } else {
            summary.show.slug.to_lowercase()
        };
        if !seen_presentations.insert(identity) {
            continue;
        }
        if summary.slug.is_empty() {
            presentations.push(summary);
            continue;
        }
        match alamo.presentation(&market_slug, &summary.slug) {
            Ok(presentation) => presentations.push(presentation),
            Err(error) => {
                report.failed += 1;
                eprintln!(
                    "Failed to fetch details for {}: {error}",
                    summary.show.title
                );
                presentations.push(summary);
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
            slug: String::new(),
            show: Show {
                slug: String::new(),
                title: title.into(),
                certification: None,
                national_release_date_utc: None,
                imdb_id: None,
                runtime_minutes: None,
                directors: vec![],
            },
            primary_collection_slug: collection.map(Into::into),
            format_slugs: vec![],
        }
    }

    fn movie_title(title: &str, year: Option<u32>) -> MovieTitle {
        MovieTitle {
            title: title.into(),
            year,
            imdb_id: None,
            runtime_minutes: None,
            directors: vec![],
        }
    }

    fn result(title: &str, year: u32, tmdb_id: u32) -> SearchResult {
        SearchResult {
            id: 0,
            title: title.into(),
            alternate_titles: vec![],
            year,
            tmdb_id,
            imdb_id: None,
            runtime: None,
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
            movie_title("What Ever Happened to Baby Jane?", Some(1962))
        );
    }

    #[test]
    fn filters_and_deduplicates_titles_across_markets() {
        let titles = unique_titles([
            presentation("The Thing", Some("terror-tuesday")),
            presentation("the thing", Some("weird-wednesday")),
            presentation("Up", Some("family-party")),
        ]);
        assert_eq!(titles, [movie_title("The Thing", None)]);
    }

    #[test]
    fn finds_primary_and_alternative_titles() {
        let mut alternate = result("Seven Samurai", 1954, 346);
        alternate.alternate_titles = vec![AlternativeTitle {
            title: "Shichinin no Samurai".into(),
        }];
        assert!(matches!(
            best_match(
                &movie_title("shichinin no samurai", Some(1954)),
                &[alternate]
            ),
            Match::Unique(_)
        ));
    }

    #[test]
    fn refuses_title_only_match_without_year() {
        let results = [
            result("The Thing", 1982, 1091),
            result("The Thing", 2011, 60935),
        ];
        assert!(matches!(
            best_match(&movie_title("The Thing", None), &results),
            Match::None
        ));
    }

    #[test]
    fn release_year_disambiguates_remakes() {
        let results = [
            result("The Thing", 1982, 1091),
            result("The Thing", 2011, 60935),
        ];
        let title = movie_title("The Thing", Some(1982));
        assert!(matches!(best_match(&title, &results), Match::Unique(movie) if movie.year == 1982));
    }

    #[test]
    fn collapses_duplicate_results_for_the_same_movie() {
        let results = [result("Alien", 1979, 348), result("Alien", 1979, 348)];
        assert!(matches!(
            best_match(&movie_title("Alien", Some(1979)), &results),
            Match::Unique(_)
        ));
    }

    #[test]
    fn matches_unicode_case_insensitively() {
        let results = [result("Amélie", 2001, 194)];
        let title = movie_title("AMÉLIE", Some(2001));
        assert!(matches!(best_match(&title, &results), Match::Unique(_)));
    }

    #[test]
    fn imdb_id_takes_precedence_over_title() {
        let mut result = result("Manhunter", 1986, 11454);
        result.imdb_id = Some("tt0091474".into());
        let mut title = movie_title("A Decorated Event Title", None);
        title.imdb_id = Some("tt0091474".into());

        assert!(matches!(best_match(&title, &[result]), Match::Unique(_)));
    }

    #[test]
    fn strips_filmmaker_prefix_when_year_matches() {
        let results = [result("Manhunter", 1986, 11454)];
        let title = movie_title("Michael Mann's Manhunter", Some(1986));

        assert!(matches!(best_match(&title, &results), Match::Unique(_)));
    }

    #[test]
    fn runtime_breaks_title_and_year_tie() {
        let mut shorter = result("Crash", 1996, 123);
        shorter.runtime = Some(100);
        let mut longer = result("Crash", 1996, 456);
        longer.runtime = Some(130);
        let mut title = movie_title("Crash", Some(1996));
        title.runtime_minutes = Some(128);

        assert!(matches!(
            best_match(&title, &[shorter, longer]),
            Match::Unique(movie) if movie.tmdb_id == 456
        ));
    }

    #[test]
    fn dry_run_searches_without_fetching_root_folders_or_adding() {
        let (alamo_url, alamo_requests) = http_server(vec![
            r#"{"data":{"marketSummaries":[{"id":"market-id","name":"Austin","slug":"austin"}]}}"#,
            r#"{"data":{"presentations":[{"slug":"weird-wednesday-the-thing","show":{"slug":"the-thing","title":"WEIRD WEDNESDAY: The Thing"},"primaryCollectionSlug":"weird-wednesday"}]}}"#,
            r#"{"data":{"presentation":{"slug":"weird-wednesday-the-thing","show":{"slug":"the-thing","title":"WEIRD WEDNESDAY: The Thing","nationalReleaseDateUtc":"1982-06-25","runtimeMinutes":109},"primaryCollectionSlug":"weird-wednesday"}}}"#,
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
        assert_eq!(requests.len(), 3);
        assert!(requests[0].starts_with("GET /s/mother/v1/page/cclamp?useUnifiedSchedule=true "));
        assert!(requests[1].starts_with("GET /s/mother/v2/schedule/market/austin "));
        assert!(requests[2].starts_with(
            "GET /s/mother/v2/schedule/presentation/austin/weird-wednesday-the-thing "
        ));
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
        assert!(titles.contains(&movie_title("What Ever Happened to Baby Jane?", Some(1962))));
        assert!(titles.iter().any(|movie| movie.title == "The Dead Pit"));
    }

    #[test]
    fn matches_movie_from_live_presentation_detail_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../alamo_movies/tests/fixtures/austin-presentation.json"
        ))
        .unwrap();
        let presentation: Presentation =
            serde_json::from_value(fixture["data"]["presentation"].clone()).unwrap();
        let movie = movie_metadata(presentation);
        let year = movie.year.expect("fixture presentation has a release year");
        let results = [result(&movie.title, year, 1)];

        assert!(matches!(best_match(&movie, &results), Match::Unique(_)));
    }
}
