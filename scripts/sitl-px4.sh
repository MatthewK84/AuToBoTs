#!/usr/bin/env bash
# rta-host against real PX4 SITL, headless: PX4's built-in SIH quadrotor, no Gazebo.
#
#   bash scripts/sitl-px4.sh [scenario ...]   (default: gate rtl loiter land late_start probe)
#
# Pins PX4-Autopilot to one release and commit, builds px4_sitl_default, builds rta-host,
# and runs scripts/sitl_px4.py. Evidence (one JSON per scenario) goes to OUT_DIR. It is PX4
# software in the loop, not hardware (docs/sitl.md).
#
# Environment:
#   PX4_HOME   where PX4 is cloned and built (default ~/.cache/rta-px4)
#   OUT_DIR    evidence directory (default sitl-px4)
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

PX4_TAG=v1.16.2
PX4_COMMIT=54f0455ffcd755534539a7cf33a09a20bf71d29d
px4_home=${PX4_HOME:-$HOME/.cache/rta-px4}
out_dir=${OUT_DIR:-sitl-px4}
src="$px4_home/PX4-Autopilot-$PX4_TAG"
venv="$px4_home/venv"
mkdir -p "$px4_home" "$out_dir"

if [[ ! -d "$src/.git" ]]; then
  git clone -q --depth 1 --branch "$PX4_TAG" --recurse-submodules --shallow-submodules -j8 \
    https://github.com/PX4/PX4-Autopilot.git "$src"
  # The NuttX submodules are for flight boards. SITL does not use them, and their shallow
  # clones have no tags, which the version header step reads.
  git -C "$src" submodule deinit -q -f platforms/nuttx/NuttX/nuttx platforms/nuttx/NuttX/apps
fi
commit=$(git -C "$src" rev-parse HEAD)
if [[ "$commit" != "$PX4_COMMIT" ]]; then
  echo "sitl-px4: $src is at $commit, not $PX4_TAG ($PX4_COMMIT)"
  exit 1
fi

if [[ ! -x "$venv/bin/python" ]]; then
  python3 -m venv "$venv"
  "$venv/bin/pip" install -q "empy>=3.3,<4" future jinja2 jsonschema kconfiglib packaging \
    pyros-genmsg pyyaml toml numpy pymavlink psutil setuptools wheel
fi
# PX4's build calls python3 from PATH.
export PATH="$venv/bin:$PATH"
if [[ ! -x "$src/build/px4_sitl_default/bin/px4" ]]; then
  make -C "$src" px4_sitl_default >"$px4_home/build.log" 2>&1 || {
    tail -n 40 "$px4_home/build.log"
    exit 1
  }
fi

cargo build --locked -q -p rta-host --bin rta-host
echo "sitl-px4: PX4 $PX4_TAG ($PX4_COMMIT), SIH sihsim_quadx, headless"
PX4_TAG=$PX4_TAG PX4_COMMIT=$PX4_COMMIT RTA_COMMIT=$(git rev-parse --short HEAD) \
  python scripts/sitl_px4.py "$src/build/px4_sitl_default" "$root/target/debug/rta-host" "$out_dir" "$@"
