# alamo2radarr

`alamo2radarr` is a one-shot synchronization job for repertory programming at Alamo Drafthouse cinemas in the United States. It fetches every Alamo market, selects movies from configured specialty collections, looks up an exact title in Radarr, and adds a unique match.

The job currently includes these collections:

- Terror Tuesday
- Weird Wednesday
- Video Vortex
- Horror Show
- Film Club
- World of Animation
- Graveyard Shift
- Psycho Cinema

The application removes known Alamo event prefixes, language labels, restoration/cut labels, anniversary labels, and release-year suffixes before searching. It deduplicates titles shown in multiple markets and uses a parsed release year to disambiguate remakes. If Radarr still returns multiple different movies with the same title, it reports the ambiguity and adds none of them.

## Workspace

This repository is a self-contained Cargo workspace:

- `crates/alamo2radarr`: synchronization policy and executable
- `crates/alamo_movies`: Alamo schedule API client and Serde models
- `crates/radarr`: Radarr v3 API client, Serde models, and add-payload builder

The workspace uses Rust 1.97.1, edition 2024, blocking `reqwest` 0.12, rustls, and Serde derives. It has no Git-based Rust dependencies.

## Configuration

`RADARR_API_TOKEN` is required. Other settings have compatibility defaults.

| Variable | Default | Description |
| --- | --- | --- |
| `RADARR_API_TOKEN` | none | Required Radarr API key |
| `RADARR_API_URL` | composed below | Full Radarr base URL, such as `https://radarr.example.test` |
| `RADARR_API_PROTOCOL` | `http` | Legacy URL protocol, used when `RADARR_API_URL` is absent |
| `RADARR_API_HOSTNAME` | `localhost:7878` | Legacy hostname and port, used when `RADARR_API_URL` is absent |
| `RADARR_ROOT_FOLDER_PATH` | first Radarr root | Explicit destination root folder |
| `RADARR_QUALITY_PROFILE_ID` | `1` | Radarr quality profile ID |
| `DRY_RUN` | false | Set to `true` or `1` to search and report without adding |
| `ALAMO_API_URL` | `https://drafthouse.com` | Override used by tests or API proxies |

Use explicit root-folder and quality-profile settings on a server rather than relying on ordering or installation-specific IDs.

## Running Locally

```sh
export RADARR_API_TOKEN='replace-me'
export RADARR_API_URL='https://radarr.example.test'
export RADARR_ROOT_FOLDER_PATH='/movies'
export RADARR_QUALITY_PROFILE_ID='1'
cargo run --release
```

Run `cargo test --workspace` for tests and `cargo clippy --workspace --all-targets -- -D warnings` for linting.

### Alamo fixtures

The test suite includes a complete live market response, a reduced Austin schedule response, and a presentation-detail response containing the fields consumed by the client. Refresh them from the public, read-only Alamo endpoints with:

```sh
./scripts/update-alamo-fixtures.sh
```

Set `ALAMO_FIXTURE_MARKET` to capture a different market, `ALAMO_FIXTURE_PRESENTATION` to select a specific presentation slug, or `ALAMO_API_URL` to use an API proxy. The refresh script requires `curl` and `jq`.

## Container

The multi-stage Docker build produces a statically linked binary. The final image is `FROM scratch`, runs as UID/GID `65532`, and contains only the executable and CA bundle.

```sh
docker build -t docker.home.spike.cx/alamo2radarr:local .
docker run --rm \
  -e RADARR_API_TOKEN \
  -e RADARR_API_URL \
  -e RADARR_ROOT_FOLDER_PATH \
  -e RADARR_QUALITY_PROFILE_ID \
  docker.home.spike.cx/alamo2radarr:local
```

If Radarr uses a private certificate authority, supply a CA bundle containing both public roots and the private CA at `/etc/ssl/certs/ca-certificates.crt`.

This image is a finite batch job and has no health endpoint. Schedule it weekly with cron, a systemd timer, or a Kubernetes `CronJob`; prevent overlapping executions in the scheduler.

Example host cron entry for Sundays at 04:00 local time:

```cron
0 4 * * 0 docker run --rm --env-file /etc/alamo2radarr.env docker.home.spike.cx/alamo2radarr:latest
```

## Drone

`.drone.yml` tests every push and pull request. Pushes to `main` publish `latest` and a 12-character commit tag to `docker.home.spike.cx/alamo2radarr`.

Configure these Drone repository secrets:

- `docker_registry_username`
- `docker_registry_password`

The pipeline targets Linux AMD64. Change the Drone platform or add native architecture pipelines and a manifest list if the server is ARM64.

## Failure Behavior

The job continues after individual market, search, or add failures so one transient error does not hide other movies. It exits unsuccessfully if any such operation failed, allowing the scheduler to alert or retry. Missing configuration and an empty Radarr root-folder list also produce a nonzero exit.

## Security

The Radarr key is sent in the `X-Api-Key` header rather than the URL. Keep secrets in the runtime secret store or an ignored environment file; never bake them into the image or Drone pipeline.
