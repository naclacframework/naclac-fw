#!/usr/bin/env bash
# Downloads the IDLs and clients that a run of the Suites workflow kept for each
# suite (the `artifacts-<suite>` artifacts) into tests/<suite>/artifacts/idl and
# tests/<suite>/artifacts/clients. Only those two folders are written or
# replaced; tests/<suite>/artifacts/deploy is never touched.
#
# Usage: bash scripts/pull-suite-artifacts.sh [run-id]
# Without a run id, the latest completed Suites run of the current branch is used.
# Needs the GitHub CLI (`gh`), logged in.

set -euo pipefail
cd "$(dirname "$0")/.."

run="${1:-}"
if [ -z "$run" ]; then
  branch="$(git branch --show-current)"
  run="$(gh run list --workflow Suites --branch "$branch" --status completed --limit 1 \
    --json databaseId --jq '.[0].databaseId')"
fi
if [ -z "$run" ] || [ "$run" = "null" ]; then
  echo "No completed Suites run found. Pass a run id: bash scripts/pull-suite-artifacts.sh <run-id>" >&2
  exit 1
fi
echo "Suites run $run"

names="$(gh api "repos/{owner}/{repo}/actions/runs/$run/artifacts" --paginate \
  --jq '.artifacts[] | select(.expired | not) | .name | select(startswith("artifacts-"))')"
if [ -z "$names" ]; then
  echo "Run $run has no artifacts-<suite> artifacts (expired, or the run predates them)." >&2
  exit 1
fi

for name in $names; do
  suite="${name#artifacts-}"
  if [ ! -d "tests/$suite" ]; then
    echo "Error: artifact $name has no matching folder tests/$suite" >&2
    exit 1
  fi
  dest="tests/$suite/artifacts"
  mkdir -p "$dest"
  rm -rf "$dest/idl" "$dest/clients"
  gh run download "$run" --name "$name" --dir "$dest"
  echo "  $suite -> $dest"
done
