#!/bin/sh
# Close an issue only after the CI run for this commit succeeds.
set -eu
issue="${1:?issue number}"
repo="${2:-MatthewK84/AuToBoTs}"
sha=$(git rev-parse HEAD)
"$(dirname "$0")/wait-ci.sh" "$sha" "$repo"
short=$(git rev-parse --short HEAD)
gh issue close "$issue" --repo "$repo" --reason completed --comment "Closed after CI passed on ${short}."
