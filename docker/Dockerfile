# Disposable environment for testing ai-browser: kill the container, every
# browser process and profile directory it touched is gone with it. See
# openspec/changes/archive/*-docker-test-harness/design.md.
FROM rust:slim-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    chromium \
    fonts-liberation \
    ca-certificates \
    build-essential \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

CMD ["cargo", "test", "--workspace"]
