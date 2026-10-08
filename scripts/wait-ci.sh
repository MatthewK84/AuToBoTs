#!/bin/sh
# Block until the CI run for this commit finishes. Exit non-zero on failure.
set -eu
sha="${1:-$(git rev-parse HEAD)}"
repo="${2:-MatthewK84/AuToBoTs}"
echo "waiting for $sha"
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  id=$(gh run list --repo "$repo" --commit "$sha" --limit 1 --json databaseId,status,conclusion --jq '.[0].databaseId')
  if [ -n "$id" ] && [ "$id" != "null" ]; then
    gh run watch "$id" --repo "$repo" --exit-status
    exit $?
  fi
  sleep 5
done
echo "no run for $sha" >&2
exit 2
