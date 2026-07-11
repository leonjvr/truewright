use crate::error::{EngineError, Result};
use crate::keys;
use crate::motion::{self, Persona};
use crate::recording::{Recording, RecordingOptions};
use crate::snapshot::{self, SnapshotResult};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const RESOLVE_JS: &str = include_str!("../assets/resolve.js");
const SEEDED_RANDOM_JS: &str = include_str!("../assets/seeded_random.js");
const VIRTUAL_CLOCK_JS: &str = include_str!("../assets/virtual_clock.js");
/// Reads this page's own on-screen window geometry (true-user-input spec:
/// "Viewport-to-screen coordinate translation" -- Chrome composites its own
/// toolbar/tab UI into one native window, so this can't be read from native
/// win32 APIs alone; the page itself already knows where it sits).
#[cfg(windows)]
const WINDOW_GEOMETRY_JS: &str = "({screen_x: window.screenX, screen_y: window.screenY, outer_width: window.outerWidth, outer_height: window.outerHeight, inner_width: window.innerWidth, inner_height: window.innerHeight, device_pixel_ratio: window.devicePixelRatio})";

const NAVIGATE_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_ACTION_TIMEOUT: Duration = Duration::from_secs(5);
const ACTIONABILITY_POLL_INTERVAL: Duration = Duration::from_millis(100);
const WAIT_FOR_POLL_INTERVAL: Duration = Duration::from_millis(250);
/// Two resolutions closer than this (px) count as "stable" (browser-actions
/// spec: "Bounded-poll actionability before acting").
const STABLE_EPSILON_PX: f64 = 0.5;

/// One browser session: one browser, one context, and one *or more* pages
/// (popup-auto-attach spec) — the Phase 1 "no daemon, no multi-session yet"
/// scope (design.md Decision #1, `2026-07-10-phase-1-agent-mvp`) was about
/// one `Session` per process, not one page per `Session`; this tracks
/// popups/new tabs that attach as a side effect of driving the page.
pub struct Session {
    launched: Option<cdp::launch::LaunchedBrowser>,
    context: cdp::ops::BrowserContext,
    /// Every page currently attached, keyed by CDP target ID.
    pages: Mutex<HashMap<String, cdp::ops::Page>>,
    /// Which page every action method (`click`/`snapshot`/etc.) currently
    /// operates against.
    active_target_id: Mutex<String>,
    /// The page `Session::launch` originally created. If the active page
    /// closes (e.g. a popup finishing an OAuth redirect), the active page
    /// falls back to this one rather than an arbitrary remaining page
    /// (popup-auto-attach spec: "Predictable fallback when the active page
    /// closes").
    primary_target_id: String,
    /// Last dispatched mouse position, so a human-like move starts from
    /// wherever the cursor actually is instead of teleporting (human-motion
    /// spec: "Curved, timed mouse movement to a target").
    mouse_pos: Mutex<(f64, f64)>,
    /// Set while a training session is active; click/type/press bracket
    /// their own CDP dispatch with the page-level suppress flag so it isn't
    /// mistaken for real human input (human-motion spec: "Synthetic
    /// dispatch is not captured as training data" — CDP-dispatched events
    /// are themselves `isTrusted`, so this flag, not `isTrusted`, is the
    /// actual guard).
    training_active: Arc<AtomicBool>,
    /// Reference to the currently-active console/action trace's entry
    /// buffer, if any (action-trace spec). Action methods check this
    /// before appending a summary entry -- a no-op when no trace is
    /// active.
    action_trace_sink: crate::console::ActionTraceSink,
    /// Whether this session was launched headless -- `true_input` rejects
    /// cleanly rather than silently falling back to CDP dispatch, since a
    /// headless renderer has no real OS window to receive `SendInput`
    /// events (true-user-input spec: "Headless and non-Windows rejection").
    /// Only read by `true_input_pid`, which is Windows-only.
    #[cfg_attr(not(windows), allow(dead_code))]
    headless: bool,
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

/// One attached page, as reported by `Session::list_pages` (popup-auto-attach
/// spec).
#[derive(Debug, Clone)]
pub struct PageInfo {
    pub page_id: String,
    pub url: String,
    pub title: String,
    pub active: bool,
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
        let target_id = page.target_id().to_string();
        let mut pages = HashMap::new();
        pages.insert(target_id.clone(), page);

        Ok(Self {
            launched: Some(launched),
            context,
            pages: Mutex::new(pages),
            active_target_id: Mutex::new(target_id.clone()),
            primary_target_id: target_id,
            mouse_pos: Mutex::new((0.0, 0.0)),
            training_active: Arc::new(AtomicBool::new(false)),
            action_trace_sink: Arc::new(Mutex::new(None)),
            headless,
        })
    }

    /// The page every action method currently operates against. Panics only
    /// if the registry and `active_target_id` have gone out of sync, which
    /// would be an engine bug, not a reachable user-facing state --
    /// `switch_page` validates before updating `active_target_id`, and
    /// `refresh_attached_pages` resets it to `primary_target_id` (always
    /// present until `close`) whenever the active page detaches.
    async fn active_page(&self) -> cdp::ops::Page {
        let active_id = self.active_target_id.lock().await.clone();
        self.pages
            .lock()
            .await
            .get(&active_id)
            .cloned()
            .expect("active_target_id always refers to a page in the registry")
    }

    /// Non-blocking drain of any newly discovered/destroyed targets noticed
    /// browser-wide since the last poll (popup-auto-attach spec: "Automatic
    /// attach to new top-level targets"). Filtered to top-level page targets
    /// belonging to this session's own context -- other browser contexts'
    /// targets and cross-origin OOPIF (iframe-type) targets are both out of
    /// scope for this slice and ignored, not tracked as pages.
    async fn refresh_attached_pages(&self) {
        let created = self.context.poll_created_targets().await;
        let destroyed = self.context.poll_destroyed_targets().await;

        for ev in created {
            let info = ev.target_info;
            if info.target_type != "page" {
                continue;
            }
            if info.browser_context_id.as_deref() != Some(self.context.context_id()) {
                continue;
            }
            if self.pages.lock().await.contains_key(&info.target_id) {
                continue; // already tracked (e.g. this session's own primary page)
            }
            match self
                .context
                .attach_existing_target(info.target_id.clone())
                .await
            {
                Ok(new_page) => {
                    self.pages.lock().await.insert(info.target_id, new_page);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to attach newly discovered target");
                }
            }
        }

        for ev in destroyed {
            self.forget_page(&ev.target_id).await;
        }
    }

    /// Removes a page from the registry and, if it was the active one,
    /// falls back to the primary page (popup-auto-attach spec: "Predictable
    /// fallback when the active page closes"). Shared by the
    /// `Target.targetDestroyed`-driven path and `list_pages`'s self-heal
    /// path -- a closing page's destroy event isn't reliably delivered
    /// before `Target.getTargetInfo` on it starts failing (confirmed via
    /// live testing: `window.close()` from within the page can race ahead
    /// of the event), so both paths need the exact same fallback, not just
    /// the "expected" one.
    async fn forget_page(&self, target_id: &str) {
        self.pages.lock().await.remove(target_id);
        let mut active = self.active_target_id.lock().await;
        if *active == target_id {
            *active = self.primary_target_id.clone();
        }
    }

    /// Every currently-attached page, refreshed first (popup-auto-attach
    /// spec: "Explicit page listing and switching"). Chrome can transiently
    /// create and almost immediately destroy an extra, unrelated target
    /// around browser-context/page setup (confirmed via live testing --
    /// see design.md addendum); if a page this method just attached to
    /// turns out to already be gone by the time its info is queried, it's
    /// dropped from the registry and the results rather than surfacing a
    /// hard error for something the agent never asked about.
    pub async fn list_pages(&self) -> Result<Vec<PageInfo>> {
        self.refresh_attached_pages().await;
        let known: Vec<cdp::ops::Page> = self.pages.lock().await.values().cloned().collect();

        let mut fetched = Vec::with_capacity(known.len());
        for page in known {
            match page.target_info().await {
                Ok(info) => fetched.push(info),
                Err(e) => {
                    tracing::warn!(error = %e, target_id = page.target_id(), "page vanished before it could be listed; dropping it");
                    self.forget_page(page.target_id()).await;
                }
            }
        }

        // Read *after* any self-heal fallback above so a page that vanished
        // while active is reflected by the correct (possibly just-reset)
        // active id, not a stale snapshot from before this method ran.
        let active_id = self.active_target_id.lock().await.clone();
        Ok(fetched
            .into_iter()
            .map(|info| PageInfo {
                active: info.target_id == active_id,
                page_id: info.target_id,
                url: info.url,
                title: info.title,
            })
            .collect())
    }

    /// Changes which page subsequent action methods operate against
    /// (popup-auto-attach spec). Errors clearly if `page_id` doesn't match
    /// any currently-attached page, rather than silently doing nothing.
    // EngineError is kept as one flat enum (matches cdp::CdpError's
    // rationale); see the identical allow in cdp/src/launch.rs.
    #[allow(clippy::result_large_err)]
    pub async fn switch_page(&self, page_id: &str) -> Result<()> {
        self.refresh_attached_pages().await;
        if !self.pages.lock().await.contains_key(page_id) {
            return Err(EngineError::UnknownPage(page_id.to_string()));
        }
        *self.active_target_id.lock().await = page_id.to_string();
        Ok(())
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

    /// Resolves a `browser_click`/`browser_type` request's `persona`/
    /// `trained_profile` fields into one `Persona` (human-motion spec:
    /// "Untrained profile fails clearly"). Specifying both is ambiguous;
    /// specifying neither falls back to `Persona::average()`, same as
    /// omitting `persona` alone already did.
    // EngineError is kept as one flat enum (matches cdp::CdpError's
    // rationale); see the identical allow in cdp/src/launch.rs.
    #[allow(clippy::result_large_err)]
    pub fn persona_or_trained(
        persona: Option<&str>,
        trained_profile: Option<&str>,
    ) -> Result<Persona> {
        match (persona, trained_profile) {
            (Some(_), Some(_)) => Err(EngineError::AmbiguousPersona),
            (Some(name), None) => Self::persona(name),
            (None, Some(name)) => motion::profile_store::load(name),
            (None, None) => Ok(Persona::average()),
        }
    }

    /// Starts capturing genuinely trusted input for a training session
    /// (human-motion spec: "Training capture from real trusted input").
    /// `Training::stop` finishes and persists the fitted profile under
    /// `name`.
    pub async fn train_start(&self, name: &str) -> Result<motion::Training> {
        self.training_active.store(true, Ordering::SeqCst);
        let page = self.active_page().await;
        match motion::Training::start(&page, name, self.training_active.clone()).await {
            Ok(training) => Ok(training),
            Err(e) => {
                self.training_active.store(false, Ordering::SeqCst);
                Err(e)
            }
        }
    }

    /// Starts passively recording network traffic to a named cassette
    /// (network-mocking spec: "Passive network recording to a named
    /// cassette"). `NetworkRecording::stop` finishes and persists it.
    pub async fn network_record_start(
        &self,
        name: &str,
    ) -> Result<crate::network::NetworkRecording> {
        let page = self.active_page().await;
        crate::network::NetworkRecording::start(&page, name).await
    }

    /// Starts intercepting every request and fulfilling it from the named
    /// cassette (network-mocking spec: "Replay from a cassette with no
    /// live-network dependency"). `NetworkReplay::stop` disables
    /// interception.
    pub async fn network_replay_start(&self, name: &str) -> Result<crate::network::NetworkReplay> {
        let page = self.active_page().await;
        crate::network::NetworkReplay::start(&page, name).await
    }

    /// Registers JS that runs before any of a page's own scripts, on every
    /// subsequent navigation (deterministic-init spec: "Init scripts run
    /// before a page's own scripts"). Register before navigating -- an
    /// init script only affects loads that happen after it's registered.
    pub async fn add_init_script(&self, source: &str) -> Result<()> {
        self.active_page().await.add_init_script(source).await?;
        Ok(())
    }

    /// Overrides `Math.random` with a deterministic PRNG seeded from
    /// `seed`, via `add_init_script` (deterministic-init spec: "Seeded,
    /// reproducible Math.random").
    pub async fn seed_randomness(&self, seed: u64) -> Result<()> {
        let script = SEEDED_RANDOM_JS.replace("%SEED%", &(seed as u32).to_string());
        self.add_init_script(&script).await
    }

    /// Installs a virtual clock frozen at `time_ms` (epoch milliseconds),
    /// via `add_init_script` -- overrides `Date`/`performance.now`/timers
    /// to read from it (virtual-clock spec: "Agent-controlled virtual
    /// clock"). Register before navigating, same as any init script.
    pub async fn set_clock(&self, time_ms: u64) -> Result<()> {
        let script = VIRTUAL_CLOCK_JS.replace("%START_TIME_MS%", &time_ms.to_string());
        self.add_init_script(&script).await
    }

    /// Advances the installed virtual clock by `ms`, synchronously firing
    /// every due `setTimeout`/`setInterval`/`requestAnimationFrame`
    /// callback in chronological order (virtual-clock spec: "Explicit
    /// clock advancement fires due timers in order"). Requires a clock to
    /// already be installed via `set_clock` on the current page.
    pub async fn advance_clock(&self, ms: u64) -> Result<()> {
        let raw = self
            .active_page()
            .await
            .evaluate(&format!("typeof window.__aibAdvanceClock === 'function' ? (window.__aibAdvanceClock({ms}), true) : false"))
            .await?;
        if raw.as_bool() != Some(true) {
            return Err(EngineError::Clock(
                "no virtual clock is installed on the current page; call browser_set_clock (and browser_navigate) first".to_string(),
            ));
        }
        Ok(())
    }

    /// Starts capturing console output and uncaught exceptions on the
    /// current page (console-capture spec). `ConsoleCapture::stop` finishes
    /// and persists the JSONL trace under `name`.
    pub async fn console_capture_start(
        &self,
        name: &str,
    ) -> Result<crate::console::ConsoleCapture> {
        let page = self.active_page().await;
        crate::console::ConsoleCapture::start(&page, name, self.action_trace_sink.clone()).await
    }

    /// Appends a one-line summary to the active trace, if any (action-trace
    /// spec: "Action entries interleaved into the active trace"). A no-op
    /// when no trace is active.
    async fn log_action(&self, text: String) {
        if let Some(active) = self.action_trace_sink.lock().await.as_ref() {
            active.entries.lock().await.push(crate::console::TraceEntry::Action {
                text,
                timestamp_ms: crate::console::now_ms(),
            });
        }
    }

    pub async fn navigate(&self, url: &str) -> Result<String> {
        self.log_action(format!("navigate {url}")).await;
        self.active_page()
            .await
            .navigate_and_wait(url, NAVIGATE_TIMEOUT)
            .await?;
        self.snapshot().await
    }

    /// Evaluates the injected walker and renders its tree as text
    /// (page-snapshot spec: "Injected accessibility-style walker").
    pub async fn snapshot(&self) -> Result<String> {
        let raw = self
            .active_page()
            .await
            .evaluate(snapshot::WALKER_JS)
            .await?;
        let parsed: SnapshotResult = serde_json::from_value(raw)?;
        Ok(snapshot::render(&parsed))
    }

    pub async fn click(&self, r#ref: &str) -> Result<()> {
        self.click_with(r#ref, None, false).await?;
        Ok(())
    }

    /// Human-like variant of `click`: with `human` set, moves the mouse
    /// along a synthesized curved path (starting from the last known
    /// position) before pressing, instead of teleporting straight to the
    /// target (human-motion spec: "Curved, timed mouse movement to a
    /// target"). Returns the seed used, if human-like mode was requested, so
    /// the run can be reproduced. `true_input` dispatches via real Windows
    /// `SendInput` instead of CDP (true-user-input spec); rejected cleanly
    /// for headless sessions or on non-Windows platforms.
    pub async fn click_with(
        &self,
        r#ref: &str,
        human: Option<HumanLike>,
        true_input: bool,
    ) -> Result<Option<u64>> {
        self.log_action(format!("click {ref}")).await;
        let suppress = self.begin_training_suppression().await;
        let result = self.click_dispatch(r#ref, human, true_input).await;
        self.end_training_suppression(suppress).await;
        result
    }

    /// Clicks to focus, then inserts text (browser-actions spec: "Type by
    /// ref"). `submit` additionally presses Enter afterward.
    pub async fn type_text(&self, r#ref: &str, text: &str, submit: bool) -> Result<()> {
        self.type_text_with(r#ref, text, submit, None, false)
            .await?;
        Ok(())
    }

    /// Human-like variant of `type_text`: with `human` set, clicks to focus
    /// via a human-like mouse path, then dispatches one `char` event per
    /// character with a persona-derived, non-uniform delay between them
    /// (human-motion spec: "Per-character typing cadence"). Returns the
    /// seed used, if human-like mode was requested. `true_input` dispatches
    /// via real Windows `SendInput` instead of CDP (true-user-input spec);
    /// rejected cleanly for headless sessions or on non-Windows platforms.
    pub async fn type_text_with(
        &self,
        r#ref: &str,
        text: &str,
        submit: bool,
        human: Option<HumanLike>,
        true_input: bool,
    ) -> Result<Option<u64>> {
        self.log_action(format!("type {ref} {text:?}")).await;
        // Suppressed once for the whole action (focus-click + typing +
        // optional submit), not per sub-step -- sub-steps below call the
        // unwrapped `_dispatch` variants so the suppress flag isn't
        // cleared partway through by a nested toggle.
        let suppress = self.begin_training_suppression().await;
        let result = self
            .type_text_dispatch(r#ref, text, submit, human, true_input)
            .await;
        self.end_training_suppression(suppress).await;
        result
    }

    pub async fn press(&self, key: &str) -> Result<()> {
        self.log_action(format!("press {key}")).await;
        let suppress = self.begin_training_suppression().await;
        let result = self.press_dispatch(key, false).await;
        self.end_training_suppression(suppress).await;
        result
    }

    async fn click_dispatch(
        &self,
        r#ref: &str,
        human: Option<HumanLike>,
        true_input: bool,
    ) -> Result<Option<u64>> {
        let coords = self
            .resolve_actionable(r#ref, DEFAULT_ACTION_TIMEOUT)
            .await?;

        if true_input {
            return self.click_true_input(coords, human).await;
        }

        let seed = if let Some(HumanLike { persona, seed }) = human {
            let seed = seed.unwrap_or_else(rand_seed);
            let mut rng = motion::seeded_rng(seed);
            let from = *self.mouse_pos.lock().await;
            let target_size = coords.width.max(coords.height).max(1.0);
            let path =
                motion::mouse_path(from, (coords.x, coords.y), target_size, &persona, &mut rng);
            self.walk_mouse_path(&path).await?;
            Some(seed)
        } else {
            None
        };

        self.active_page()
            .await
            .click_at(coords.x, coords.y)
            .await?;
        *self.mouse_pos.lock().await = (coords.x, coords.y);
        Ok(seed)
    }

    async fn type_text_dispatch(
        &self,
        r#ref: &str,
        text: &str,
        submit: bool,
        human: Option<HumanLike>,
        true_input: bool,
    ) -> Result<Option<u64>> {
        let seed = match human {
            Some(HumanLike { persona, seed }) => {
                let seed = seed.unwrap_or_else(rand_seed);
                self.click_dispatch(
                    r#ref,
                    Some(HumanLike {
                        persona,
                        seed: Some(seed),
                    }),
                    true_input,
                )
                .await?;

                let mut rng = motion::seeded_rng(seed);
                let timeline = motion::typing_timeline(text, &persona, &mut rng);
                if true_input {
                    self.dispatch_typing_true_input(&timeline).await?;
                } else {
                    self.dispatch_typing(&timeline).await?;
                }
                Some(seed)
            }
            None => {
                self.click_dispatch(r#ref, None, true_input).await?;
                if true_input {
                    // SendInput has no bulk-insert primitive -- every
                    // character is its own real keystroke either way, so
                    // true_input without an explicit persona falls back to
                    // a fast synthetic cadence rather than an unrealistic
                    // zero-delay burst (true-user-input spec).
                    let mut rng = motion::seeded_rng(rand_seed());
                    let timeline = motion::typing_timeline(text, &Persona::fast(), &mut rng);
                    self.dispatch_typing_true_input(&timeline).await?;
                } else {
                    self.active_page().await.insert_text(text).await?;
                }
                None
            }
        };

        if submit {
            self.press_dispatch("Enter", true_input).await?;
        }
        Ok(seed)
    }

    async fn press_dispatch(&self, key: &str, true_input: bool) -> Result<()> {
        let spec = keys::lookup(key).ok_or_else(|| EngineError::UnknownKey(key.to_string()))?;
        if true_input {
            return self
                .press_true_input(spec.windows_virtual_key_code as u16)
                .await;
        }
        self.active_page()
            .await
            .dispatch_key(spec.key, spec.code, spec.windows_virtual_key_code)
            .await?;
        Ok(())
    }

    /// If a training session is active, flags this page's recorder to
    /// ignore events for the duration of the caller's action -- CDP
    /// dispatch is itself `isTrusted`, so `isTrusted` alone can't tell this
    /// engine's own synthetic input apart from a real human's (human-motion
    /// spec: "Synthetic dispatch is not captured as training data"). A
    /// no-op (and no extra round trip) when no training session is active.
    async fn begin_training_suppression(&self) -> bool {
        let suppress = self.training_active.load(Ordering::SeqCst);
        if suppress {
            let _ = self
                .active_page()
                .await
                .evaluate("window.__aibSuppressTraining = true")
                .await;
        }
        suppress
    }

    async fn end_training_suppression(&self, was_suppressing: bool) {
        if was_suppressing {
            let _ = self
                .active_page()
                .await
                .evaluate("window.__aibSuppressTraining = false")
                .await;
        }
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

    /// Checks the current snapshot for `text`'s presence (or absence),
    /// immediately -- no polling, unlike `wait_for` (browser-assert spec:
    /// "Immediate text-presence assertion"). Logs the outcome (pass or
    /// fail) into the active trace, if any.
    pub async fn assert_text(&self, text: &str, present: bool) -> Result<()> {
        let snap = self.snapshot().await?;
        let holds = snap.contains(text) == present;

        self.log_action(format!(
            "assert text={text:?} present={present} => {}",
            if holds { "pass" } else { "fail" }
        ))
        .await;

        if holds {
            return Ok(());
        }
        Err(EngineError::AssertionFailed {
            text: text.to_string(),
            present,
            snapshot_excerpt: snap.chars().take(500).collect(),
        })
    }

    pub async fn screenshot(&self) -> Result<Vec<u8>> {
        let bytes = self.active_page().await.screenshot().await?;

        // Best-effort: a screenshot-tracing hiccup must never fail the
        // screenshot call itself (html-trace-viewer spec: "Screenshot
        // logging never fails the screenshot call").
        if let Some(active) = self.action_trace_sink.lock().await.as_ref() {
            if let Ok(path) = crate::console::save_screenshot(&active.name, &bytes) {
                active.entries.lock().await.push(crate::console::TraceEntry::Screenshot {
                    path: path.display().to_string(),
                    timestamp_ms: crate::console::now_ms(),
                });
            }
        }

        Ok(bytes)
    }

    /// Parses and executes a YAML script's steps in order against this
    /// session, fail-fast on the first failing step (yaml-runner spec:
    /// "Declarative YAML step execution").
    pub async fn run_yaml(&self, source: &str) -> Result<crate::yaml_runner::RunSummary> {
        crate::yaml_runner::run(self, source).await
    }

    /// Loads a saved console/action trace by name and converts its action
    /// entries into a runnable YAML script (yaml-runner spec: "Trace
    /// export to a runnable YAML script").
    // EngineError is kept as one flat enum (matches cdp::CdpError's
    // rationale); see the identical allow in cdp/src/launch.rs.
    #[allow(clippy::result_large_err)]
    pub fn export_yaml(name: &str) -> Result<String> {
        let entries = crate::console::load_trace(name)?;
        crate::yaml_runner::export(&entries)
    }

    /// Starts a screencast recording (browser-recording spec). Artifacts
    /// land under `<data-dir>/aib/recordings/<id>/` once `Recording::stop`
    /// is called.
    pub async fn start_recording(&self, options: RecordingOptions) -> Result<Recording> {
        let recordings_base = cdp::launch::profile_base_dir()?
            .join("aib")
            .join("recordings");
        let page = self.active_page().await;
        Recording::start(&page, options, recordings_base).await
    }

    /// Tears the session down: every attached page, the context, and (if we
    /// launched it) the browser process.
    pub async fn close(mut self) -> Result<()> {
        for page in self.pages.lock().await.values() {
            let _ = page.close().await;
        }
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
        let page = self.active_page().await;
        let mut last_ms = 0.0;
        for point in path {
            let delta = (point.at_ms - last_ms).max(0.0);
            if delta > 0.0 {
                tokio::time::sleep(Duration::from_secs_f64(delta / 1000.0)).await;
            }
            page.move_mouse_to(point.x, point.y).await?;
            last_ms = point.at_ms;
        }
        Ok(())
    }

    /// Dispatches a synthesized typing timeline with real pacing.
    async fn dispatch_typing(&self, timeline: &[motion::TimedKey]) -> Result<()> {
        let page = self.active_page().await;
        let mut last_ms = 0.0;
        for key in timeline {
            let delta = (key.at_ms - last_ms).max(0.0);
            if delta > 0.0 {
                tokio::time::sleep(Duration::from_secs_f64(delta / 1000.0)).await;
            }
            page.dispatch_char(key.ch).await?;
            last_ms = key.at_ms;
        }
        Ok(())
    }

    /// Resolves `true_input`'s preconditions (true-user-input spec:
    /// "Headless and non-Windows rejection") into the OS process id real
    /// dispatch needs to locate the browser's window. A clean typed error,
    /// never a silent CDP fallback.
    // EngineError is kept as one flat enum (matches cdp::CdpError's
    // rationale); see the identical allow in cdp/src/launch.rs.
    #[cfg(windows)]
    #[allow(clippy::result_large_err)]
    fn true_input_pid(&self) -> Result<u32> {
        if self.headless {
            return Err(EngineError::TrueInputUnsupported(
                "session is headless; true_input requires a real, visible window".to_string(),
            ));
        }
        self.launched.as_ref().and_then(|l| l.pid()).ok_or_else(|| {
            EngineError::TrueInputUnsupported(
                "no OS process id available for this session".to_string(),
            )
        })
    }

    /// Reads this page's own on-screen geometry, for translating viewport
    /// coordinates to OS screen coordinates (true-user-input spec:
    /// "Viewport-to-screen coordinate translation").
    #[cfg(windows)]
    async fn window_geometry(&self) -> Result<cdp::os_input::WindowGeometry> {
        #[derive(Deserialize)]
        struct Geom {
            screen_x: f64,
            screen_y: f64,
            outer_width: f64,
            outer_height: f64,
            inner_width: f64,
            inner_height: f64,
            device_pixel_ratio: f64,
        }
        let raw = self
            .active_page()
            .await
            .evaluate(WINDOW_GEOMETRY_JS)
            .await?;
        let g: Geom = serde_json::from_value(raw)?;
        Ok(cdp::os_input::WindowGeometry {
            screen_x: g.screen_x,
            screen_y: g.screen_y,
            outer_width: g.outer_width,
            outer_height: g.outer_height,
            inner_width: g.inner_width,
            inner_height: g.inner_height,
            device_pixel_ratio: g.device_pixel_ratio,
        })
    }

    /// Reads this page's on-screen window bounds from CDP, for
    /// disambiguating between a browser process's several OS windows.
    /// Every headed session turns out to own at least two: the window
    /// opened by the initial browser launch, plus a separate native window
    /// for the isolated `BrowserContext` the engine actually drives (see
    /// design.md addendum) -- PID filtering alone can't tell them apart, so
    /// `os_input::find_browser_window` picks whichever candidate's bounds
    /// most closely match this hint.
    #[cfg(windows)]
    async fn window_hint(&self) -> Result<cdp::os_input::WindowHint> {
        let bounds = self.active_page().await.window_bounds().await?.bounds;
        let (Some(left), Some(top), Some(width), Some(height)) =
            (bounds.left, bounds.top, bounds.width, bounds.height)
        else {
            return Err(EngineError::TrueInput(
                "browser reported incomplete window bounds (window minimized?)".to_string(),
            ));
        };
        Ok(cdp::os_input::WindowHint {
            left: left as i32,
            top: top as i32,
            width: width as i32,
            height: height as i32,
        })
    }

    /// Runs a blocking `os_input` call on a dedicated thread so the
    /// non-`Send` `HWND` it works with never needs to cross an `.await`
    /// point (design.md: "all Win32 calls are fast, local, and
    /// non-blocking" -- `spawn_blocking` here is about `Send`-safety, not
    /// avoiding runtime stalls).
    #[cfg(windows)]
    async fn run_true_input<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce() -> cdp::Result<()> + Send + 'static,
    {
        tokio::task::spawn_blocking(f)
            .await
            .map_err(|e| EngineError::TrueInput(format!("blocking task panicked: {e}")))?
            .map_err(|e| EngineError::TrueInput(e.to_string()))
    }

    #[cfg(windows)]
    #[allow(clippy::result_large_err)]
    async fn click_true_input(
        &self,
        coords: Coordinates,
        human: Option<HumanLike>,
    ) -> Result<Option<u64>> {
        let pid = self.true_input_pid()?;
        let geom = self.window_geometry().await?;
        let hint = self.window_hint().await?;

        let seed = match human {
            Some(HumanLike { persona, seed }) => {
                let seed = seed.unwrap_or_else(rand_seed);
                let mut rng = motion::seeded_rng(seed);
                let from = *self.mouse_pos.lock().await;
                let target_size = coords.width.max(coords.height).max(1.0);
                let path =
                    motion::mouse_path(from, (coords.x, coords.y), target_size, &persona, &mut rng);
                let screen_path: Vec<cdp::os_input::ScreenPoint> = path
                    .iter()
                    .map(|p| {
                        let (x, y) = cdp::os_input::viewport_to_screen(geom, p.x, p.y);
                        cdp::os_input::ScreenPoint {
                            at_ms: p.at_ms,
                            x,
                            y,
                        }
                    })
                    .collect();
                self.run_true_input(move || cdp::os_input::walk_and_click(pid, hint, &screen_path))
                    .await?;
                Some(seed)
            }
            None => {
                let (x, y) = cdp::os_input::viewport_to_screen(geom, coords.x, coords.y);
                self.run_true_input(move || cdp::os_input::click_at(pid, hint, x, y))
                    .await?;
                None
            }
        };

        *self.mouse_pos.lock().await = (coords.x, coords.y);
        Ok(seed)
    }

    #[cfg(not(windows))]
    async fn click_true_input(
        &self,
        _coords: Coordinates,
        _human: Option<HumanLike>,
    ) -> Result<Option<u64>> {
        Err(EngineError::TrueInputUnsupported(
            "true_input is only supported on Windows".to_string(),
        ))
    }

    #[cfg(windows)]
    #[allow(clippy::result_large_err)]
    async fn dispatch_typing_true_input(&self, timeline: &[motion::TimedKey]) -> Result<()> {
        let pid = self.true_input_pid()?;
        let hint = self.window_hint().await?;
        let timed: Vec<cdp::os_input::TimedChar> = timeline
            .iter()
            .map(|k| cdp::os_input::TimedChar {
                at_ms: k.at_ms,
                ch: k.ch,
            })
            .collect();
        self.run_true_input(move || cdp::os_input::dispatch_typing(pid, hint, &timed))
            .await
    }

    #[cfg(not(windows))]
    async fn dispatch_typing_true_input(&self, _timeline: &[motion::TimedKey]) -> Result<()> {
        Err(EngineError::TrueInputUnsupported(
            "true_input is only supported on Windows".to_string(),
        ))
    }

    #[cfg(windows)]
    #[allow(clippy::result_large_err)]
    async fn press_true_input(&self, virtual_key_code: u16) -> Result<()> {
        let pid = self.true_input_pid()?;
        let hint = self.window_hint().await?;
        self.run_true_input(move || cdp::os_input::press_key(pid, hint, virtual_key_code))
            .await
    }

    #[cfg(not(windows))]
    async fn press_true_input(&self, _virtual_key_code: u16) -> Result<()> {
        Err(EngineError::TrueInputUnsupported(
            "true_input is only supported on Windows".to_string(),
        ))
    }

    async fn resolve_ref(&self, r#ref: &str) -> Result<ResolveResult> {
        let script = format!("({RESOLVE_JS})({})", serde_json::to_string(r#ref)?);
        let raw = self.active_page().await.evaluate(&script).await?;
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
