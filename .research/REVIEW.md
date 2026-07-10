# Review: "High Performance Browser Automation" research

**Date:** 2026-07-11
**Question:** Should ai-browser continue, or does something better already exist?
**Goal being evaluated against:** a more cost-effective (CPU + memory) Playwright-type application that agents use to test applications. Screenshots are a hard requirement; short motion videos of moving parts are desired.

## Claim-by-claim verification

| Claim in research doc | Verdict |
|---|---|
| Lightpanda: 9–16× lighter than Chrome | **True, but disqualified for our goal.** It has no rendering engine — no CSS layout, no paint pipeline — and therefore fundamentally cannot take screenshots (confirmed on lightpanda.io's own blog and lightpanda-io/browser issue #2343), so no videos either. It's a scraping/text-extraction tool, not a testing tool. |
| OxyBlink: embedded Blink engine in Rust with zero-copy Skia screenshots, Nano/Standard/Full tiers | **Not verifiable — likely fabricated.** No such crate exists on crates.io. The research doc also cites a polyethylene-pipe manufacturing article as a source (citation #80), a hallmark of LLM-generated citation noise. Discarded. |
| A Rust driver rewrite yields only ~5–10% throughput gain; the browser binary dominates cost | **Directionally correct**, and consistent with our own measured benchmark: renderer memory is identical either way and wall-clock navigation time was similar. However, the *driver-side* footprint difference is much larger than the doc implies and it matters for our use case: measured on this machine, aib's driver is 8.0 MB RSS / 0.03 s CPU per session vs Playwright's Node driver at 120.3 MB / 0.80 s — ~15× memory and ~27× CPU per agent session, plus 5.7 MB on disk vs 400 MB+ browser downloads and a Node runtime. For fleets of concurrent agent sessions, that compounds. |
| chrome-headless-shell is meaningfully lighter than `--headless=new` | **Real.** Distributed with Chrome for Testing; it's what Puppeteer's `headless: 'shell'` uses. The doc's exact memory numbers are unverified, but the direction is well-established, and it **does** support screenshots. It is a separate ~100 MB download. |
| CDP `Page.startScreencast` for video capture | **Confirmed.** The standard mechanism (Cypress uses it), ~30 fps in practice, works headless, frames encodable to WebM/GIF. Directly enables the motion-video requirement — and is exactly what screenshot-less engines like Lightpanda can never do. |
| `--remote-debugging-pipe` and memory-reduction launch flags | Real, low-risk optimizations. Pipe transport fits our existing `Transport` trait seam; the flags are a one-line addition to `cdp::launch`. |

## Recommendation: continue building ai-browser

No existing tool covers the goal:

- **Lightpanda** cannot screenshot — hard disqualifier.
- **OxyBlink** does not appear to exist.
- **Playwright + optimizations** (the research's own low-risk recommendation) keeps the ~120 MB Node driver per agent session, playwright-mcp's overhead, and the download-heavy distribution model. It narrows the browser-side gap but not the driver-side one, and it isn't MCP-native.

ai-browser's niche — an MCP-native, ~6 MB single-binary driver with token-efficient snapshots, real screenshots, and (now planned) video capture — remains unserved.

**But the research's central insight is correct and changes our roadmap:** the browser binary, not the driver, is the dominant cost. The driver war is already won; the next wins are browser-side.

## Roadmap adjustment (decided with the user, 2026-07-11)

1. **`browser-efficiency`** (next): memory-reduction launch flags; **auto-downloaded chrome-headless-shell** as the default headless binary (cached, installed-Chrome fallback; `--headed` keeps using installed Chrome) — a deliberate relaxation of the original zero-download principle in exchange for out-of-the-box memory savings; measured process-tree memory evidence via the doctor/bench tooling.
2. **`screencast-capture`** (next): `Page.startScreencast`-based recording, JPEG frame sequence + animated GIF (pure Rust), optional WebM via ffmpeg when present, exposed as `browser_record_start`/`browser_record_stop` MCP tools.
3. **Human-motion engine** (original Phase 2): moved to after the two changes above.
