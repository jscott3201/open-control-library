#!/usr/bin/env bash
# The three pinned source checkouts the verification steps read, as siblings of this repository:
#
#   ../open-control                    jscott3201/open-control-engine at ENGINE_PIN
#   ../modelica-buildings-release      lbl-srg/modelica-buildings at routines/g36/SOURCE_RELEASE_PIN
#   ../modelica-buildings-development  lbl-srg/modelica-buildings at routines/g36/SOURCE_DEVELOPMENT_PIN
#
# Each pin file holds one 40-hex commit and is the only copy of that pin. This script is the one
# "fetch pinned sources" step: both gate workflows and .agents/gate.sh call it, so a later consumer
# of the same checkouts (a tool built from the engine checkout) is fed by the same fetch.
#
#   bash .github/scripts/pinned-sources.sh fetch [parent]      fetch each checkout at its pin
#   bash .github/scripts/pinned-sources.sh check [parent]      fail, naming each checkout that is
#                                                              missing or off its pin
#   bash .github/scripts/pinned-sources.sh toolchain [parent]  print the Rust channel named by the
#                                                              engine checkout's rust-toolchain.toml
#
# `parent` defaults to the directory that holds this repository. The sources are fetched by exact
# commit with plain git: tools/lint/g36_source.py requires the Modelica checkouts' origin to be
# github.com/lbl-srg/modelica-buildings, and tools/verify/build.rs requires the engine checkout's
# HEAD to equal ENGINE_PIN with no tracked modification.

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
mode=${1:-}
parent=${2:-"$repo_root/.."}

usage() {
  echo "usage: bash .github/scripts/pinned-sources.sh fetch|check|toolchain [parent-directory]" >&2
  exit 2
}

case "$mode" in
  fetch | check | toolchain) ;;
  *) usage ;;
esac

# directory | repository | pin file
sources=(
  "open-control|jscott3201/open-control-engine|ENGINE_PIN"
  "modelica-buildings-release|lbl-srg/modelica-buildings|routines/g36/SOURCE_RELEASE_PIN"
  "modelica-buildings-development|lbl-srg/modelica-buildings|routines/g36/SOURCE_DEVELOPMENT_PIN"
)

read_pin() {
  local pin_file=$1 pin
  pin=$(tr -d '[:space:]' < "$repo_root/$pin_file")
  if ! [[ $pin =~ ^[0-9a-f]{40}$ ]]; then
    echo "pinned sources: $pin_file does not hold one 40-hex commit" >&2
    return 1
  fi
  printf '%s\n' "$pin"
}

head_of() {
  git -C "$1" rev-parse --verify --quiet 'HEAD^{commit}' 2>/dev/null || true
}

is_checkout() {
  [ -d "$1" ] && [ "$(git -C "$1" rev-parse --show-prefix 2>/dev/null || echo no)" = "" ]
}

tracked_changes() {
  git -C "$1" status --porcelain --untracked-files=no
}

fetch_one() {
  local name=$1 slug=$2 pin_file=$3 dir pin url
  dir="$parent/$name"
  pin=$(read_pin "$pin_file") || return 1
  url="https://github.com/$slug.git"
  if [ -e "$dir" ] && ! is_checkout "$dir"; then
    echo "pinned sources: $dir exists and is not a git checkout; move it away" >&2
    return 1
  fi
  if [ ! -e "$dir" ]; then
    git init --quiet "$dir"
    git -C "$dir" remote add origin "$url"
  elif [ "$(head_of "$dir")" = "$pin" ]; then
    echo "pinned sources: $name already at $pin_file ($pin)"
    return 0
  elif [ -n "$(tracked_changes "$dir")" ]; then
    echo "pinned sources: $dir is off $pin_file and has tracked modifications; not moving it" >&2
    return 1
  fi
  git -C "$dir" fetch --quiet --depth 1 "$url" "$pin"
  git -C "$dir" checkout --quiet --detach FETCH_HEAD
  echo "pinned sources: $name fetched at $pin_file ($pin)"
}

check_one() {
  local name=$1 pin_file=$3 dir pin head
  dir="$parent/$name"
  pin=$(read_pin "$pin_file") || return 1
  if ! is_checkout "$dir"; then
    echo "pinned sources: MISSING ../$name (no git checkout there); $pin_file names $pin"
    return 1
  fi
  head=$(head_of "$dir")
  if [ "$head" != "$pin" ]; then
    echo "pinned sources: OFF PIN ../$name is at ${head:-no commit}, but $pin_file names $pin"
    return 1
  fi
  if [ "$name" = open-control ] && [ -n "$(tracked_changes "$dir")" ]; then
    echo "pinned sources: MODIFIED ../$name is at ENGINE_PIN but has tracked modifications"
    return 1
  fi
  echo "pinned sources: ok ../$name at $pin_file ($pin)"
}

engine_channel() {
  local file="$parent/open-control/rust-toolchain.toml" channel
  if [ ! -f "$file" ]; then
    echo "pinned sources: ../open-control/rust-toolchain.toml is missing; fetch the engine first" >&2
    return 1
  fi
  channel=$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*$/\1/p' "$file")
  if ! [[ $channel =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "pinned sources: ../open-control/rust-toolchain.toml names no exact release channel" >&2
    return 1
  fi
  printf '%s\n' "$channel"
}

if [ "$mode" = toolchain ]; then
  engine_channel
  exit 0
fi

bad=0
for row in "${sources[@]}"; do
  IFS='|' read -r name slug pin_file <<< "$row"
  if [ "$mode" = fetch ]; then
    fetch_one "$name" "$slug" "$pin_file" || bad=$((bad + 1))
  else
    check_one "$name" "$slug" "$pin_file" || bad=$((bad + 1))
  fi
done

if [ "$bad" -gt 0 ]; then
  if [ "$mode" = check ]; then
    echo "pinned sources: $bad of ${#sources[@]} checkouts are not usable." \
      "Run: bash .github/scripts/pinned-sources.sh fetch"
  fi
  exit 1
fi
