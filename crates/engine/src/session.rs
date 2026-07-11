use crate::error::{EngineError, Result};
use crate::keys;
use crate::motion::{self, Persona};
use crate::recording::{Recording, RecordingOptions};
use crate::snapshot::{self, SnapshotResult};
use serde::Deserialize;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const RESOLVE_JS: &str = include_str!("../assets/resolve.js");

const NAVIGATE_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_ACTION_TIMEOUT: Duration = Duration::from_secs(5);
const ACTIONABILITY_POLL_INTERVAL: Duration = Duration::from_millis(100);
const WAIT_FOR_POLL_INTERVAL: Duration = Duration::from_millis(250);
/// Two resolutions closer than this (px) count as "stable" (browser-actions
/// spec: "Bounded-poll actionability before acting").
const STABLE_EPSILON_PX: f64 = 0.5;

/// One browser session: one browser, one context, one page — the Phase 1
/// scope (design.md Decision #1: no daemon, no multi-session yet).
pub struct Session {
    launched: Option<cdp::launch::LaunchedBrowser>,
    context: cdp::ops::BrowserContext,
    page: cdp::ops::Page,
    /// Last dispatched mouse position, so a human-like move starts from
    /// wherever the cursor actually is instead of teleporting (human-motion
    /// spec: "Curved, timed mouse movement to a target").
    mouse_pos: Mutex<(f64, f64)>,
}

/// Requests a human-like (curved mouse path / paced typing cadence) variant
/// of an action instead of the default instant dispatch (human-motion spec).
pub struct HumanLike {
    pub persona: Persona,
    /// Fixed for reproducibility; a fresh random seed is drawn if `None`.
    pub seed: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ResolveResult {
    ok: bool,
    #[serde(default)]
    visible: bool,
    #[serde(default)]
    x: f64,
    #[serde(default)]
    y: f64,
    #[serde(default)]
    width: f64,
    #[serde(default)]
    height: f64,
}

struct Coordinates {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Session {
    /// Launches a browser and opens one context and one blank page
    /// (page-snapshot / browser-actions specs' prerequisite state).
    /// Headless sessions resolve the browser per `pref` (managed
    /// headless-shell by default, installed browser as fallback/opt-out);
    /// headed sessions always use the installed browser.
    pub async fn launch(profile_name: &str, headless: bool) -> Result<Self> {
        Self::launch_with(profile_name, headless, cdp::launch::BrowserPreference::Auto).await
    }

    pub async fn launch_with(
        profile_name: &str,
        headless: bool,
        pref: cdp::launch::BrowserPreference,
    ) -> Result<Self> {
        let discovered = if headless {
            cdp::launch::resolve_headless_browser(pref).await?
        } else {
            let mut found = cdp::launch::discover_browsers()?;
            found.remove(0)
        };
        let launched = cdp::launch::launch(&discovered, profile_name, headless).await?;

        let browser = cdp::ops::Browser::connect(&launched.ws_url).await?;
        let context = browser.new_context().await?;
        let page = context.new_page("about:blank").await?;

        Ok(Self {
            launched: Some(launched),
            context,
            page,
            mouse_pos: Mutex::new((0.0, 0.0)),
        })
    }

    /// Looks up a built-in persona by name, for callers (the MCP layer)
    /// turning a request's `persona` string into a typed engine error rather
    /// than silently falling back to a default (human-motion spec: "Persona
    /// presets").
    // EngineError is kept as one flat enum (matches cdp::CdpError's
    // rationale); see the identical allow in cdp/src/launch.rs.
    #[allow(clippy::result_large_err)]
    pub fn persona(name: &str) -> Result<Persona> {
        Persona::by_name(name).ok_or_else(|| EngineError::UnknownPersona(name.to_string()))
    }

    pub async fn navigate(&self, url: &str) -> Result<String> {
        self.page.navigate_and_wait(url, NAVIGATE_TIMEOUT).await?;
        self.snapshot().await
    }

    /// Evaluates the injected walker and renders its tree as text
    /// (page-snapshot spec: "Injected accessibility-style walker").
    pub async fn snapshot(&self) -> Result<String> {
        let raw = self.page.evaluate(snapshot::WALKER_JS).await?;
        let parsed: SnapshotResult = serde_json::from_value(raw)?;
        Ok(snapshot::render(&parsed))
    }

    pub async fn click(&self, r#ref: &str) -> Result<()> {
        self.click_with(r#ref, None).await?;
        Ok(())
    }

    /// Human-like variant of `click`: with `human` set, moves the mouse
    /// along a synthesized curved path (starting from the last known
    /// position) before pressing, instead of teleporting straight to the
    /// target (human-motion spec: "Curved, timed mouse movement to a
    /// target"). Returns the seed used, if human-like mode was requested, so
    /// the run can be reproduced.
    pub async fn click_with(&self, r#ref: &str, human: Option<HumanLike>) -> Result<Option<u64>> {
        let coords = self
            .resolve_actionable(r#ref, DEFAULT_ACTION_TIMEOUT)
            .await?;

        let seed = if let Some(HumanLike { persona, seed }) = human {
            let seed = seed.unwrap_or_else(rand_seed);
            let mut rng = motion::seeded_rng(seed);
            let from = *self.mouse_pos.lock().await;
            let target_size = coords.width.max(coords.height).max(1.0);
            let path = motion::mouse_path(from, (coords.x, coords.y), target_size, &persona, &mut rng);
            self.walk_mouse_path(&path).await?;
            Some(seed)
        } else {
            None
        };

        self.page.click_at(coords.x, coords.y).await?;
        *self.mouse_pos.lock().await = (coords.x, coords.y);
        Ok(seed)
    }

    /// Clicks to focus, then inserts text (browser-actions spec: "Type by
    /// ref"). `submit` additionally presses Enter afterward.
    pub async fn type_text(&self, r#ref: &str, text: &str, submit: bool) -> Result<()> {
        self.type_text_with(r#ref, text, submit, None).await?;
        Ok(())
    }

    /// Human-like variant of `type_text`: with `human` set, clicks to focus
    /// via a human-like mouse path, then dispatches one `char` event per
    /// character with a persona-derived, non-uniform delay between them
    /// (human-motion spec: "Per-character typing cadence"). Returns the
    /// seed used, if human-like mode was requested.
    pub async fn type_text_with(
        &self,
        r#ref: &str,
        text: &str,
        submit: bool,
        human: Option<HumanLike>,
    ) -> Result<Option<u64>> {
        let seed = match human {
            Some(HumanLike { persona, seed }) => {
                let seed = seed.unwrap_or_else(rand_seed);
                self.click_with(
                    r#ref,
                    Some(HumanLike {
                        persona,
                        seed: Some(seed),
                    }),
                )
                .await?;

                let mut rng = motion::seeded_rng(seed);
                let timeline = motion::typing_timeline(text, &persona, &mut rng);
                self.dispatch_typing(&timeline).await?;
                Some(seed)
            }
            None => {
                self.click(r#ref).await?;
                self.page.insert_text(text).await?;
                None
            }
        };

        if submit {
            self.press("Enter").await?;
        }
        Ok(seed)
    }

    pub async fn press(&self, key: &str) -> Result<()> {
        let spec = keys::lookup(key).ok_or_else(|| EngineError::UnknownKey(key.to_string()))?;
        self.page
            .dispatch_key(spec.key, spec.code, spec.windows_virtual_key_code)
            .await?;
        Ok(())
    }

    /// Polls the rendered snapshot for a substring (browser-actions spec:
    /// "Wait for text").
    pub async fn wait_for(&self, text: &str, timeout: Duration) -> Result<String> {
        let deadline = Instant::now() + timeout;
        loop {
            let snap = self.snapshot().await?;
            if snap.contains(text) {
                return Ok(snap);
            }
            if Instant::now() >= deadline {
                return Err(EngineError::WaitTimeout {
                    text: text.to_string(),
                    timeout,
                });
            }
            tokio::time::sleep(WAIT_FOR_POLL_INTERVAL).await;
        }
    }

    pub async fn screenshot(&self) -> Result<Vec<u8>> {
        Ok(self.page.screenshot().await?)
    }

    /// Starts a screencast recording (browser-recording spec). Artifacts
    /// land under `<data-dir>/aib/recordings/<id>/` once `Recording::stop`
    /// is called.
    pub async fn start_recording(&self, options: RecordingOptions) -> Result<Recording> {
        let recordings_base = cdp::launch::profile_base_dir()?
            .join("aib")
            .join("recordings");
        Recording::start(&self.page, options, recordings_base).await
    }

    /// Tears the session down: page, context, and (if we launched it) the
    /// browser process.
    pub async fn close(mut self) -> Result<()> {
        let _ = self.page.close().await;
        let _ = self.context.dispose().await;
        if let Some(launched) = self.launched.take() {
            launched.shutdown().await?;
        }
        Ok(())
    }

    /// Dispatches a synthesized mouse path with real pacing: sleeps between
    /// points for the delta in their `at_ms` timestamps rather than firing
    /// them back to back (human-motion spec: "timelines are computed up
    /// front, then dispatched with real pacing").
    async fn walk_mouse_path(&self, path: &[motion::TimedPoint]) -> Result<()> {
        let mut last_ms = 0.0;
        for point in path {
            let delta = (point.at_ms - last_ms).max(0.0);
            if delta > 0.0 {
                tokio::time::sleep(Duration::from_secs_f64(delta / 1000.0)).await;
            }
            self.page.move_mouse_to(point.x, point.y).await?;
            last_ms = point.at_ms;
        }
        Ok(())
    }

    /// Dispatches a synthesized typing timeline with real pacing.
    async fn dispatch_typing(&self, timeline: &[motion::TimedKey]) -> Result<()> {
        let mut last_ms = 0.0;
        for key in timeline {
            let delta = (key.at_ms - last_ms).max(0.0);
            if delta > 0.0 {
                tokio::time::sleep(Duration::from_secs_f64(delta / 1000.0)).await;
            }
            self.page.dispatch_char(key.ch).await?;
            last_ms = key.at_ms;
        }
        Ok(())
    }

    async fn resolve_ref(&self, r#ref: &str) -> Result<ResolveResult> {
        let script = format!("({RESOLVE_JS})({})", serde_json::to_string(r#ref)?);
        let raw = self.page.evaluate(&script).await?;
        Ok(serde_json::from_value(raw)?)
    }

    /// Bounded-poll actionability gate: resolve until visible and stable
    /// across two consecutive reads, or time out (browser-actions spec).
    async fn resolve_actionable(&self, r#ref: &str, timeout: Duration) -> Result<Coordinates> {
        let deadline = Instant::now() + timeout;
        let mut last: Option<(f64, f64, f64, f64)> = None;

        loop {
            let resolved = self.resolve_ref(r#ref).await?;
            if !resolved.ok {
                return Err(EngineError::StaleRef(r#ref.to_string()));
            }

            if resolved.visible {
                let current = (resolved.x, resolved.y, resolved.width, resolved.height);
                if let Some(prev) = last {
                    if rects_close(prev, current) {
                        return Ok(Coordinates {
                            x: resolved.x,
                            y: resolved.y,
                            width: resolved.width,
                            height: resolved.height,
                        });
                    }
                }
                last = Some(current);
            } else {
                last = None;
            }

            if Instant::now() >= deadline {
                return Err(EngineError::ActionTimeout {
                    r#ref: r#ref.to_string(),
                    timeout,
                    last_visible: last.is_some(),
                });
            }
            tokio::time::sleep(ACTIONABILITY_POLL_INTERVAL).await;
        }
    }
}

fn rand_seed() -> u64 {
    use rand::Rng;
    rand::thread_rng().gen()
}

fn rects_close(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    (a.0 - b.0).abs() < STABLE_EPSILON_PX
        && (a.1 - b.1).abs() < STABLE_EPSILON_PX
        && (a.2 - b.2).abs() < STABLE_EPSILON_PX
        && (a.3 - b.3).abs() < STABLE_EPSILON_PX
}
