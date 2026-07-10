//! Integration test: the full attach→navigate→evaluate→screenshot→teardown
//! cycle against a real, installed Chromium browser (tasks.md 4.2). Skips
//! rather than fails when no browser is installed, so CI without Chrome/Edge
//! stays green.

use cdp::launch::discover_browsers;
use cdp::ops::run_full_cycle;

#[tokio::test]
async fn full_cycle_against_installed_browser() {
    let browsers = match discover_browsers() {
        Ok(list) => list,
        Err(e) => {
            eprintln!("skipping full_cycle_against_installed_browser: {e}");
            return;
        }
    };

    let browser = &browsers[0];
    let report = run_full_cycle(browser, "integration-test", true)
        .await
        .expect("full cycle should succeed against a real browser");

    assert_eq!(report.title, serde_json::json!("Example Domain"));
    assert!(
        report.screenshot_bytes > 0,
        "screenshot should not be empty"
    );
}
