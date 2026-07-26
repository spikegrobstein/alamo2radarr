#!/bin/sh

set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
fixture_dir="$repository_root/crates/alamo_movies/tests/fixtures"
base_url=${ALAMO_API_URL:-https://drafthouse.com}
market_slug=${ALAMO_FIXTURE_MARKET:-austin}
schedule_file=$(mktemp)
presentation_file=$(mktemp)

trap 'rm -f "$schedule_file" "$presentation_file"' EXIT
mkdir -p "$fixture_dir"

curl --fail --silent --show-error --location \
    "$base_url/s/mother/v1/page/cclamp?useUnifiedSchedule=true" \
    | jq '.' > "$fixture_dir/markets.json"

curl --fail --silent --show-error --location \
    "$base_url/s/mother/v2/schedule/market/$market_slug" \
    > "$schedule_file"

jq '{data: {presentations: [.data.presentations[] | {
    slug,
    show: (.show | {slug, title, certification}),
    primaryCollectionSlug,
    formatSlugs
}]}}' "$schedule_file" > "$fixture_dir/$market_slug-schedule.json"

presentation_slug=${ALAMO_FIXTURE_PRESENTATION:-$(jq -r '
    [.data.presentations[] | select(.primaryCollectionSlug == "terror-tuesday"
        or .primaryCollectionSlug == "weird-wednesday"
        or .primaryCollectionSlug == "video-vortex"
        or .primaryCollectionSlug == "horror-show"
        or .primaryCollectionSlug == "film-club"
        or .primaryCollectionSlug == "world-of-animation"
        or .primaryCollectionSlug == "graveyard-shift"
        or .primaryCollectionSlug == "psycho-cinema")][0].slug
' "$schedule_file")}

curl --fail --silent --show-error --location \
    "$base_url/s/mother/v2/schedule/presentation/$market_slug/$presentation_slug" \
    > "$presentation_file"

jq '{data: {presentation: {
    slug: .data.presentation.slug,
    show: (.data.presentation.show | {
        slug,
        title,
        certification,
        nationalReleaseDateUtc,
        imdbId,
        runtimeMinutes,
        directors
    }),
    primaryCollectionSlug: .data.presentation.primaryCollectionSlug,
    formatSlugs: .data.presentation.formatSlugs
}}}' "$presentation_file" > "$fixture_dir/$market_slug-presentation.json"
