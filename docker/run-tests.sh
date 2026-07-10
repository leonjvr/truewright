#!/usr/bin/env bash
# Build the ai-browser test image and run the full workspace test suite plus
# `aib doctor` inside a disposable container. The repo is mounted read-write
# (normal edit/test loop); the container's own target/ and cargo registry
# live in named volumes so they persist across runs without touching the
# host's target/.
set -euo pipefail

# Git Bash on Windows rewrites leading-/ arguments (like "-w /app") into
# Windows paths before they ever reach docker.exe; this opts out for both
# the volume-mount pwd and the args, and is a no-op on real Linux/macOS.
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL="*"

cd "$(dirname "$0")/.."

docker build -t aib-test -f docker/Dockerfile .

docker run --rm \
  -v "$(pwd)":/app \
  -v aib-cargo-registry:/usr/local/cargo/registry \
  -v aib-container-target:/app/target \
  -w /app \
  aib-test \
  bash -c "cargo test --workspace && echo '--- aib doctor ---' && cargo run --quiet --bin aib -- doctor --json"
