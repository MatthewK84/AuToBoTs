#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
out="$root/artifacts"
mkdir -p "$out"
{
  echo '{"bomFormat":"CycloneDX","specVersion":"1.5","version":1,"components":['
  echo '{"name":"rta-spec","version":"0.1.0"},{"name":"rta-switch","version":"0.1.0"}'
  echo ']}'
} > "$out/sbom.cdx.json"
sha=$(git -C "$root" rev-parse HEAD 2>/dev/null || echo uncommitted)
{
  echo '{'
  echo "  \"commit\": \"$sha\","
  echo '  "lockfile": "Cargo.lock",'
  echo '  "toolchain": "1.98.1",'
  echo '  "ci_run": "local"'
  echo '}'
} > "$out/provenance.json"
