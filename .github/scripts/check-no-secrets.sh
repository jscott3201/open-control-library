#!/usr/bin/env bash
# Tracked-file scan: absolute developer home paths, UUID-shaped strings and common secret shapes.
# It reads the tracked files of the repository it is run in (`git grep`), so gitignored working
# directories are out of scope by construction. Each pattern matches a shape, never a literal
# value, so this file does not match itself. The rule is the engine repository's.

set -euo pipefail

violations=0
errors=0

# `git grep` exits 0 on a match, 1 on no match, and above 1 when it could not search at all: not
# a repository, a pattern it rejects, a pattern it read as an option. Only 1 is a clean result.
# The pattern is passed with `-e` because one of them begins with a dash, and git's own message
# goes to stderr unhidden.
scan() {
  local pattern="$1"
  local label="$2"
  local matches status=0
  matches="$(git grep -nE -e "$pattern")" || status=$?
  case "$status" in
    0)
      echo "FAIL: matched $label pattern in tracked files:"
      printf '%s\n' "$matches"
      violations=$((violations + 1))
      ;;
    1) ;;
    *)
      echo "FAIL: git grep exited $status on the $label pattern; nothing was scanned for it."
      errors=$((errors + 1))
      ;;
  esac
}

scan 'AKIA[0-9A-Z]{16}' 'AWS access key id'
scan '-----BEGIN (RSA|EC|OPENSSH|PGP) PRIVATE KEY-----' 'private key block'
scan 'xox[abpr]-[A-Za-z0-9-]{10,}' 'Slack token'
scan 'gh[pousr]_[A-Za-z0-9]{36,}' 'GitHub token'
scan 'sk-[A-Za-z0-9_-]{20,}' 'API token (sk- prefix)'
scan '[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}' 'UUID-shaped string (possible agent or namespace identity)'
scan '/(Users|home)/[A-Za-z0-9._-]+/' 'absolute developer home-directory path'
scan 'AIONFORGE_[A-Z][A-Z_]+' 'Aionforge environment variable name'

if [ "$violations" -gt 0 ]; then
  echo
  echo "Remove the match from the working tree. A real secret is also rotated, and history rewritten."
fi

if [ "$errors" -gt 0 ]; then
  echo
  echo "The scan did not complete: $errors pattern(s) could not be searched. That is not clean."
  exit 2
fi

if [ "$violations" -gt 0 ]; then
  exit 1
fi

echo "OK: tracked-file scan clean (home paths, UUID-shaped strings, secret shapes)."
