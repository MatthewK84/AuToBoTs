#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
out="$root/artifacts"
mkdir -p "$out"
if [ ! -f "$out/sbom.cdx.json" ] || [ ! -f "$out/provenance.json" ]; then
  echo "evidence job: SBOM or provenance record missing" >&2
  exit 1
fi
echo "evidence job: SBOM and provenance record present"
