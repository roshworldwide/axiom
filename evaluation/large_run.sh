#!/usr/bin/env bash
# Large-scale TLC exploration: model-check widened CRDT instances to completion,
# cumulatively exploring >= 1e8 distinct states. This is NOT part of per-commit
# CI (it is slow + resource-heavy); run it manually or via the nightly workflow
# (.github/workflows/large-tlc.yml). The committed evaluation/large_run.md is the
# reference run on the author's machine.
#
# Env knobs:
#   JAVA  - java binary (default: java; on macOS keg-only Temurin pass the path)
#   JAR   - path to tla2tools.jar (downloaded if absent)
#   XMX   - JVM max heap (default 10g)
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TLA="$ROOT/tla"
CFG="$ROOT/evaluation/configs"
JAVA="${JAVA:-java}"
XMX="${XMX:-10g}"
VER="${TLA2TOOLS_VERSION:-v1.7.4}"
JAR="${JAR:-$ROOT/tla2tools.jar}"
[ -f "$JAR" ] || curl -fLsS -o "$JAR" \
  "https://github.com/tlaplus/tlaplus/releases/download/$VER/tla2tools.jar"

mem_gib() {
  if [ "$(uname)" = Darwin ]; then echo $(( $(sysctl -n hw.memsize) / 1073741824 ))
  else echo $(( $(awk '/MemTotal/{print $2}' /proc/meminfo) / 1048576 )); fi
}
cores() { sysctl -n hw.ncpu 2>/dev/null || nproc; }

echo "# Large-scale TLC run"
echo
echo "- generated: $(date -u +%Y-%m-%dT%H:%M:%SZ) UTC"
echo "- machine: $(uname -s) $(uname -m), $(cores) cores, $(mem_gib) GiB RAM, JVM -Xmx${XMX}"
echo "- TLC: $("$JAVA" -cp "$JAR" tlc2.TLC 2>&1 | grep -i version | head -1)"
echo "- JVM: $("$JAVA" -version 2>&1 | head -1)"
echo
echo "Each spec below is model-checked to completion (no error) at widened"
echo "CONSTANTS; symmetry reduction over replicas keeps it sound for invariants."
echo

cd "$TLA"
total=0
run() {
  local title="$1" spec="$2" cfg="$3"
  echo "## $title"
  echo '```'
  local log
  log=$("$JAVA" -Xmx"$XMX" -cp "$JAR" tlc2.TLC -workers auto -config "$cfg" "$spec" 2>&1)
  echo "$log" | grep -E 'states generated|distinct states found|Finished in|Model checking completed|is violated|Error' | tail -6
  echo '```'
  echo
  local d
  d=$(echo "$log" | grep -oE '[0-9]+ distinct states found' | tail -1 | grep -oE '^[0-9]+')
  total=$(( total + ${d:-0} ))
}

# GCounter is the headline: a single model that alone exceeds 1e8 distinct
# states. PNCounter and ORSet add harder (concurrent inc/dec, add/remove)
# coverage to the cumulative total.
run "GCounter — 3 replicas, MaxIncrements=13, symmetry  (headline, >1e8)" GCounter.tla "$CFG/GCounter.large.cfg"
run "PNCounter — 3 replicas, MaxOps=3, symmetry" PNCounter.tla "$CFG/PNCounter.large.cfg"
run "ORSet — 3 replicas, 2 elements, MaxAdds=2, symmetry" ORSet.tla "$CFG/ORSet.large.cfg"

echo "## Cumulative"
echo
echo "Distinct states explored across the run: **${total}**"
