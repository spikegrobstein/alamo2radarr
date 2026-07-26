use std::{env, process::ExitCode};

use alamo2radarr::{SyncOptions, synchronize};
use thiserror::Error;

#[derive(Debug, Error)]
enum Error {
    #[error("RADARR_API_TOKEN must be set")]
    MissingApiToken,
    #[error("RADARR_QUALITY_PROFILE_ID must be a positive integer: {0}")]
    InvalidQualityProfile(String),
    #[error(transparent)]
    Alamo(#[from] alamo_movies::Error),
    #[error(transparent)]
    Radarr(#[from] radarr::Error),
    #[error(transparent)]
    Sync(#[from] alamo2radarr::Error),
}

fn run() -> Result<(), Error> {
    let api_token = env::var("RADARR_API_TOKEN").map_err(|_| Error::MissingApiToken)?;
    let radarr_url = env::var("RADARR_API_URL").unwrap_or_else(|_| {
        let protocol = env::var("RADARR_API_PROTOCOL").unwrap_or_else(|_| "http".into());
        let hostname = env::var("RADARR_API_HOSTNAME").unwrap_or_else(|_| "localhost:7878".into());
        format!("{protocol}://{hostname}")
    });
    let quality_profile = env::var("RADARR_QUALITY_PROFILE_ID").unwrap_or_else(|_| "1".into());
    let quality_profile_id = quality_profile
        .parse::<u32>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or(Error::InvalidQualityProfile(quality_profile))?;
    let root_folder_path = env::var("RADARR_ROOT_FOLDER_PATH")
        .ok()
        .filter(|path| !path.trim().is_empty());
    let dry_run =
        env::var("DRY_RUN").is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));

    let alamo = alamo_movies::Client::builder()
        .base_url(env::var("ALAMO_API_URL").unwrap_or_else(|_| "https://drafthouse.com".into()))
        .build()?;
    let radarr = radarr::Client::builder()
        .api_token(api_token)
        .base_url(radarr_url)
        .build()?;
    let report = synchronize(
        &alamo,
        &radarr,
        &SyncOptions {
            quality_profile_id,
            root_folder_path,
            dry_run,
        },
    )?;

    eprintln!(
        "Completed: {} markets, {} presentations, {} unique titles, {} added, {} already present, {} unmatched, {} ambiguous",
        report.markets,
        report.presentations,
        report.unique_titles,
        report.added,
        report.already_added,
        report.no_match,
        report.ambiguous,
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}
