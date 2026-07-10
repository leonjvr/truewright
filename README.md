# ai-browser (`aib`)

An LLM-first browser-testing engine: a single Rust binary that drives your installed Chrome/Edge over CDP, with human-realistic (and seed-reproducible) mouse/keyboard input, a true OS-level input mode, token-efficient accessibility snapshots for agents, Playwright-style deterministic injection (network mocks, virtual clock, init scripts), and a native MCP server.

**Why:** Playwright/Puppeteer are resource-heavy (Node driver + ~1 GB bundled browsers + browser-per-worker) and their input doesn't mimic a real user (teleporting mouse, instant clicks). `aib` fixes both while keeping deterministic testing.

📄 **Read the full design: [PROPOSAL.md](./PROPOSAL.md)** — architecture, MCP tool surface, strengths/weaknesses, risks, and the phased roadmap.

## Status

Proposal stage. Next step: Phase 0 CDP spike (attach to installed Chrome/Edge, navigate, evaluate, screenshot).
