#!/usr/bin/env bash
# Same shape as run-tests.sh, but builds the Chrome-Beta image and points
# discovery at it via AIB_CHROME_PATH (baked into the image, re-asserted
# here for clarity). Meant for the weekly Chrome-beta CI job -- see
# openspec/changes/archive/*-weekly-chrome-beta-ci/design.md.
set -euo pipefail

export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL="*"

cd "$(dirname "$0")/.."

docker build -t aib-test-chrome-beta -f docker/Dockerfile.chrome-beta .

docker run --rm \
  -e AIB_CHROME_PATH=/usr/bin/google-chrome-beta \
  -v "$(pwd)":/app \
  -v aib-cargo-registry:/usr/local/cargo/registry \
  -v aib-container-target-chrome-beta:/app/target \
  -w /app \
  aib-test-chrome-beta \
  bash -c "cargo test --workspace && echo '--- aib doctor ---' && cargo run --quiet --bin aib -- doctor --json"
