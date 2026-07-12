#!/usr/bin/env bash
# Same shape as run-tests.sh, but builds the Chrome-Beta image and points
# discovery at it via TRUEWRIGHT_CHROME_PATH (baked into the image, re-asserted
# here for clarity). Meant for the weekly Chrome-beta CI job -- see
# openspec/changes/archive/*-weekly-chrome-beta-ci/design.md.
set -euo pipefail

export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL="*"

cd "$(dirname "$0")/.."

docker build -t truewright-test-chrome-beta -f docker/Dockerfile.chrome-beta .

docker run --rm \
  -e TRUEWRIGHT_CHROME_PATH=/usr/bin/google-chrome-beta \
  -v "$(pwd)":/app \
  -v truewright-cargo-registry:/usr/local/cargo/registry \
  -v truewright-container-target-chrome-beta:/app/target \
  -w /app \
  truewright-test-chrome-beta \
  bash -c "cargo test --workspace && echo '--- truewright doctor ---' && cargo run --quiet --bin truewright -- doctor --json"
