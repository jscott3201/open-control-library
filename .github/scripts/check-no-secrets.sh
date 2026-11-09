#!/usr/bin/env bash
# Tracked-file scan: absolute developer home paths, UUID-shaped strings and common secret shapes.
# It reads the tracked files of the repository it is run in (`git grep`), so gitignored working
# directories are out of scope by construction. Each pattern matches a shape, never a literal
# value, so this file does not match itself. The rule is the engine repository's.

set -euo pipefail

violations=0

scan() {
  local pattern="$1"
  local label="$2"
  if git grep -nE -e "$pattern" >/dev/null 2>&1; then
    echo "FAIL: matched $label pattern in tracked files:"
    git grep -nE -e "$pattern" || true
    violations=$((violations + 1))
  fi
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
  exit 1
fi

echo "OK: tracked-file scan clean (home paths, UUID-shaped strings, secret shapes)."
