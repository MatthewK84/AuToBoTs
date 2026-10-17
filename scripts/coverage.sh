#!/usr/bin/env bash
# Measure rta-switch. An uncovered Revert arm in decide fails.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
sysroot=$(rustc --print sysroot)
export PATH="$sysroot/lib/rustlib/x86_64-unknown-linux-gnu/bin:$PATH"
export RUSTFLAGS="-C instrument-coverage"
rm -f target/switch-test.profraw target/switch.profdata
cargo test -p rta-switch --lib --locked --no-run
bin=$(find target/debug/deps -name 'rta_switch-*' -type f -executable -printf '%T@ %p\n' | sort -n | tail -1 | cut -d' ' -f2-)
LLVM_PROFILE_FILE="target/switch-test.profraw" "$bin" --test-threads=8 >/dev/null
llvm-profdata merge -sparse target/switch-test.profraw -o target/switch.profdata
mkdir -p target/coverage
llvm-cov report --instr-profile=target/switch.profdata --ignore-filename-regex='cargo/registry|rustlib' "$bin" > target/coverage/switch.txt
llvm-cov show --instr-profile=target/switch.profdata --name-regex='6decide$' "$bin" > target/coverage/decide.txt
revert=$(grep -c 'Verdict::Revert' target/coverage/decide.txt || true)
zero=$(grep 'Verdict::Revert' target/coverage/decide.txt | grep -c '| *0|' || true)
if [ "$revert" -lt 2 ] || [ "$zero" -ne 0 ]; then
  echo "uncovered Revert arm in decide" >&2
  cat target/coverage/decide.txt >&2
  exit 1
fi
echo "decide Revert arms covered"
