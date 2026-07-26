#!/bin/sh

set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
fixture_dir="$repository_root/crates/alamo_movies/tests/fixtures"
base_url=${ALAMO_API_URL:-https://drafthouse.com}
market_slug=${ALAMO_FIXTURE_MARKET:-austin}
schedule_file=$(mktemp)

trap 'rm -f "$schedule_file"' EXIT
mkdir -p "$fixture_dir"

curl --fail --silent --show-error --location \
    "$base_url/s/mother/v1/page/cclamp?useUnifiedSchedule=true" \
    | jq '.' > "$fixture_dir/markets.json"

curl --fail --silent --show-error --location \
    "$base_url/s/mother/v2/schedule/market/$market_slug" \
    > "$schedule_file"

jq '{data: {presentations: [.data.presentations[] | {
    show: (.show | {slug, title, certification}),
    primaryCollectionSlug
}]}}' "$schedule_file" > "$fixture_dir/$market_slug-schedule.json"
