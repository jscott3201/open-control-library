#!/usr/bin/env bash
#
# The gate. This file is the single runnable source of what must pass before a change lands. Every
# other document points here instead of restating commands, because prose copies of a command list
# drift, and a copy that is weaker than CI reads as a green light.
#
#   bash .agents/gate.sh          # light — mirrors the per-PR gate (.github/workflows/pr-gate.yml)
#   bash .agents/gate.sh full     # full  — what release-gate.yml executes: the same steps, from a
#                                 #         clean build and a fresh Python environment
#
# Today the per-PR gate is the whole gate, so `full` adds no check. It removes what an earlier run
# left behind (both Rust build directories, the Python environment, the installed mdBook), which
# is the state a CI runner starts in. A check that outgrows the per-PR budget is added under
# `full` here, to release-gate.yml and removed from pr-gate.yml in one change.
#
# Commands are kept in CI's exact form, flags included: `--locked`, the pinned Python
# requirements, the checksum-pinned mdBook. Change a command by changing pr-gate.yml first, then
# this file, then the pinned list in .github/scripts/check_workflow_gates.py: the workflow rules
# step holds pr-gate.yml and the steps below to that one list and fails until all three agree. It
# reads a step as `step '<label>' <command>` (or `engine_step`): one command, with no `&&`, `||`,
# `|` or `;` on the line. A step that needs more calls a function, and the check pins that
# function's body line for line. The `set -euo pipefail` below must stay the first command and the
# only line that sets shell options. A failing gate is fixed at its cause, never by editing this
# file.
#
# What the script needs and does not install:
#   * three sibling checkouts at their pins (the engine at ENGINE_PIN, Modelica Buildings at
#     routines/g36/SOURCE_RELEASE_PIN and SOURCE_DEVELOPMENT_PIN). It checks them and fails,
#     naming each one that is missing or off its pin. `bash .github/scripts/pinned-sources.sh
#     fetch` creates them.
#   * the Rust toolchain named in the engine checkout's rust-toolchain.toml, through rustup. The
#     pin selects the compiler: the script exports RUSTUP_TOOLCHAIN for every cargo command.
#   * python3 with the venv module (CI uses 3.12; set OCL_GATE_PYTHON to choose an interpreter).
# What it installs, into the untracked target/ directory only: a Python environment holding the
# pinned requirements (target/gate-venv) and the checksum-pinned mdBook (target/mdbook-bin).
#
# Every step runs even after a failure, so one run reports every problem. The closing report
# lists each step's result and names what this script cannot cover locally.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

MODE="${1:-light}"
case "$MODE" in
  light | full) ;;
  *)
    echo "usage: bash .agents/gate.sh [light|full]" >&2
    exit 2
    ;;
esac

failures=0
declare -a results=()
declare -a failed_steps=()
gate_start=$SECONDS

record() {
  local status="$1" label="$2" seconds="$3"
  results+=("$(printf '%-4s %4ss  %s' "$status" "$seconds" "$label")")
  if [ "$status" = FAIL ]; then
    failures=$((failures + 1))
    failed_steps+=("$label")
  fi
}

# Run one gate step, echoing the command first. A step never aborts the run.
step() {
  local label="$1" started=$SECONDS
  shift
  printf '\n\033[1m── %s\033[0m\n$ %s\n' "$label" "$*"
  # A step that is a function of this script prints its body, so the log shows the real commands.
  if declare -F "$1" > /dev/null; then
    declare -f "$1" | sed 's/^/    /'
  fi
  if "$@"; then
    printf '\033[32mok\033[0m — %s\n' "$label"
    record ok "$label" "$((SECONDS - started))"
  else
    printf '\033[31mFAIL\033[0m — %s\n' "$label"
    record FAIL "$label" "$((SECONDS - started))"
  fi
}

# A step that reads the pinned sibling checkouts. When they are missing or off their pins the step
# is reported as failed without being run: its own error would only obscure the cause.
sources_ok=0
engine_step() {
  if [ "$sources_ok" -eq 1 ]; then
    step "$@"
  else
    printf '\n\033[1m── %s\033[0m\n\033[31mFAIL\033[0m — not run: the pinned sources check failed\n' "$1"
    record FAIL "$1 (not run: pinned sources)" 0
  fi
}

printf '\033[1mopen-control-library gate — mode: %s\033[0m\n' "$MODE"

# ── Environment ──────────────────────────────────────────────────────────────
VENV=target/gate-venv
MDBOOK_DIR=target/mdbook-bin
GATE_PYTHON="${OCL_GATE_PYTHON:-python3}"

clean_build_outputs() {
  rm -rf "$VENV" "$MDBOOK_DIR" tools/routine-compiler/target tools/verify/target
}

python_environment() {
  if [ ! -x "$VENV/bin/python3" ]; then
    "$GATE_PYTHON" -m venv "$VENV" || return 1
  fi
  "$VENV/bin/python3" -m pip install --quiet --disable-pip-version-check \
    --requirement tools/lint/requirements-routine-schemas.txt PyYAML==6.0.3
}

# Reports from an earlier run must not satisfy this one.
rm -f target/g36-reference-results.json target/dataset-harness/ci-lbl-fpu-results.json
rm -rf target/gate
mkdir -p target/gate

if [ "$MODE" = full ]; then
  step 'clean build outputs, Python environment and mdBook (full)' clean_build_outputs
fi
step 'Python environment with the pinned requirements' python_environment
# Every `python3` below is the environment's, as it is the runner's interpreter in CI.
PATH="$PWD/$VENV/bin:$PATH"
export PATH

# ── pr-gate.yml job `workflow-rules` ─────────────────────────────────────────
step 'workflow rules' python3 .github/scripts/check_workflow_gates.py
step 'gate-script fixtures' python3 -m unittest discover -s .github/scripts -p 'test_*.py' -v

# ── pr-gate.yml job `no-secret-scan` ─────────────────────────────────────────
step 'tracked-file scan (home paths, UUID shapes, secrets)' bash .github/scripts/check-no-secrets.sh

# ── pr-gate.yml job `lint` ───────────────────────────────────────────────────
step 'svg fitment lint' python3 tools/lint/svg_fitment.py
step 'registry lint' python3 tools/lint/registry.py
step 'point semantic lint' python3 tools/lint/point_semantics.py
step 'routine catalog lint: unit tests' python3 -m unittest discover -s tools/lint/tests -v
step 'routine catalog lint: schemas' python3 tools/lint/routine_schemas.py
step 'routine catalog lint: semantics' python3 tools/lint/routine_semantics.py
step 'routine catalog lint: catalog' python3 tools/lint/routines.py
step 'authored reference graphs match the graph author' python3 tools/authoring/g36_reference.py --check
step 'reference routine tests' python3 -m unittest discover -s tools/routines/tests -v
step 'book generator tests' python3 -m unittest discover -s tools/book/tests -v
step 'external dataset harness tests' python3 -m unittest discover -s tools/dataset_harness/tests -v

# ── pr-gate.yml job `verify` ─────────────────────────────────────────────────
select_engine_toolchain() {
  local channel
  channel="$(bash .github/scripts/pinned-sources.sh toolchain)" || return 1
  if ! rustup run "$channel" rustc --version; then
    echo "the toolchain the pinned engine names is not installed: rustup toolchain install $channel" >&2
    return 1
  fi
  export RUSTUP_TOOLCHAIN="$channel"
}

pinned_sources() {
  bash .github/scripts/pinned-sources.sh check && select_engine_toolchain && sources_ok=1
}

fault_library() {
  cargo run --locked --manifest-path tools/verify/Cargo.toml -- --all | tee target/gate/verify-all.log
}

dataset_fixture_replay() {
  python3 tools/dataset_harness/harness.py replay \
    --adapter lbl-fpu \
    --dataset tools/dataset_harness/tests/fixtures/lbl_fpu_tiny \
    --rules FPB-0001,FPB-0002,FPB-0003,FPB-0004,FPB-0005,FPB-0006 \
    --verifier tools/verify/target/debug/cxf-verify \
    --output target/dataset-harness/ci-lbl-fpu-results.json &&
    python3 -c 'import json; p="target/dataset-harness/ci-lbl-fpu-results.json"; d=json.load(open(p)); assert d["schema"] == "cxf-library/dataset-validation/v1"; assert d["summary"]["fault_free_alarm_samples"] == 0; assert d["summary"]["target_declared_rule_case_pairs"] == 1; assert d["summary"]["target_detected_rule_case_pairs"] == 1'
}

# `step` calls the function in this shell, so the toolchain it exports reaches every cargo command
# below, and `sources_ok` tells `engine_step` whether to run.
step 'pinned sources at their pins, and the toolchain the pinned engine names' pinned_sources

engine_step 'G36 source inventory' \
  python3 tools/lint/g36_source.py --check \
  --release-root ../modelica-buildings-release \
  --development-root ../modelica-buildings-development
engine_step 'G36 declaration package tests' \
  cargo test --locked --manifest-path tools/routine-compiler/Cargo.toml
engine_step 'G36 TrimAndRespond compiler integration' \
  env OCL_G36_RELEASE_ROOT=../../../modelica-buildings-release \
  OCL_G36_DEVELOPMENT_ROOT=../../../modelica-buildings-development \
  cargo test --locked --manifest-path tools/routine-compiler/Cargo.toml \
  --test g36_trim_and_respond -- --include-ignored
engine_step 'G36 release declarations' \
  cargo run --locked --manifest-path tools/verify/Cargo.toml -- \
  --g36-declarations ../modelica-buildings-release
engine_step 'routine library (generated deployments)' \
  cargo run --locked --manifest-path tools/verify/Cargo.toml -- --routines
engine_step 'G36 reference subsequences replayed (not deployment qualification)' \
  python3 tools/routines/reference.py \
  --verifier tools/verify/target/debug/cxf-verify \
  --report target/g36-reference-results.json
engine_step 'fault verifier unit tests' \
  cargo test --locked --manifest-path tools/verify/Cargo.toml
engine_step 'fault library linted and replayed' fault_library
engine_step 'original tiny FPU fixture replayed through the pinned engine' dataset_fixture_replay

# ── pr-gate.yml job `book` ───────────────────────────────────────────────────
step 'checksum-pinned mdBook' bash tools/book/install-mdbook.sh "$MDBOOK_DIR"
step 'book source generated from the repository' python3 tools/book/generate.py
step 'book build' "$MDBOOK_DIR/mdbook" build book

# ── Report ───────────────────────────────────────────────────────────────────
observed_totals() {
  if [ -s target/gate/verify-all.log ]; then
    printf '  fault verifier: %s; %s scenarios PASS, %s FAIL\n' \
      "$(grep -m1 '^discovered ' target/gate/verify-all.log || echo 'no discovery line')" \
      "$(grep -c '^  PASS ' target/gate/verify-all.log)" \
      "$(grep -c '^  FAIL ' target/gate/verify-all.log)"
  else
    echo '  fault verifier: not run'
  fi
  if [ -s target/g36-reference-results.json ]; then
    python3 - <<'PY' || echo '  reference replay: report unreadable'
import json
for routine in json.load(open("target/g36-reference-results.json"))["routines"]:
    print(
        f"  reference replay: {routine['id']}: engine_replay {routine['engine_replay']};"
        f" {routine['authored']['scenarios']} authored and {routine['matrix']['scenarios']}"
        " matrix scenarios"
    )
PY
  else
    echo '  reference replay: not run'
  fi
}

echo
echo '────────────────────────────────────────────────────────────────────────────'
echo "STEPS — mode $MODE, $((SECONDS - gate_start))s, at $(git rev-parse HEAD 2> /dev/null || echo 'no commit')$([ -z "$(git status --porcelain 2> /dev/null)" ] || echo ' with uncommitted changes')"
printf '  %s\n' "${results[@]}"
echo
echo 'OBSERVED TOTALS — reported, not asserted (compare them with the pull request):'
observed_totals
echo
cat <<COVERAGE
NOT COVERED BY THIS SCRIPT — a green run here does not prove these pass:
  · the CI platform. CI runs on ubuntu-latest (x86_64) with Python 3.12. This
    run used $(uname -sm) with $(python3 --version 2>&1 || echo 'no python3'). The Rust toolchain is the
    same in both: the channel the pinned engine names.
  · this script's control flow, and this script as CI executes it. The
    workflow rules step proves that the first command is set -euo pipefail and
    no later line changes the shell options; that every step line holds one
    command and no control operator; that each of those commands is a gate
    command pr-gate.yml runs or a call of a pinned function; that each
    pinned function matches its pin line for line and operator for operator;
    and that between them they run every gate command pr-gate.yml runs. It
    does not prove that a step is reached or that its result counts: it
    reads the script and does not follow its control flow, so a final
    exit 0, a redefined step or record, an if false around a step, a trap,
    a heredoc that swallows a step, and a function written in another form
    that shadows a command all pass. CI executes this script only in
    release-gate.yml, after a merge and daily: for a pull request that
    changes this script, a script under .github/scripts or a workflow, this
    run is the only one before the merge.
  · the workflows themselves: triggers, concurrency, the required CI OK
    status and branch protection are GitHub's to evaluate. The workflow
    rules step reads the files; it does not run them.
  · publication of the book to GitHub Pages (book-pages.yml, on main).
  · tools/simharness/test_harness.py. No gate runs it, here or in CI.
COVERAGE

if [ "$MODE" != full ]; then
  cat <<'LIGHT'
  · a build from nothing. This was not a `full` run: it reused whatever
    build output, Python environment and mdBook an earlier run left.
    Run `bash .agents/gate.sh full` on the final head before a pull request.
LIGHT
fi

echo '────────────────────────────────────────────────────────────────────────────'

if [ "$failures" -gt 0 ]; then
  printf '\n\033[31mGATE FAILED\033[0m (%s) — %d step(s):\n' "$MODE" "$failures"
  printf '  · %s\n' "${failed_steps[@]}"
  echo
  echo 'Fix the cause. Never weaken a gate to reach green.'
  exit 1
fi

printf '\n\033[32mGATE PASSED\033[0m (%s) — all %d steps green.\n' "$MODE" "${#results[@]}"
