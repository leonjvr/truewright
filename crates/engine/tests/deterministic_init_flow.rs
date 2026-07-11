//! End-to-end deterministic-init tests: an init script's effect is visible
//! to a page's own first-run inline script (proving before-page-scripts
//! ordering), and seeded randomness reproduces identical Math.random()
//! sequences across navigations with the same seed while differing across
//! seeds (deterministic-init spec). Skips (not fails) when no browser is
//! installed, matching the other integration tests' convention.

use engine::Session;

fn fixture_url(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}"));
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if !normalized.starts_with('/') {
        normalized = format!("/{normalized}");
    }
    format!("file://{normalized}")
}

fn extract_result(snapshot: &str) -> Option<String> {
    for line in snapshot.lines() {
        let trimmed = line.trim_start();
        if let Some(text_start) = trimmed.strip_prefix("- text \"") {
            if let Some(end) = text_start.rfind('"') {
                return Some(text_start[..end].to_string());
            }
        }
    }
    None
}

#[tokio::test]
async fn init_script_effect_is_visible_to_the_pages_own_first_run_script() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping init_script_effect_is_visible_to_the_pages_own_first_run_script: no installed browser found"
        );
        return;
    }

    let session = Session::launch("engine-test-init-script", true)
        .await
        .expect("session launches");

    session
        .add_init_script("window.__initFlag = 'hello';")
        .await
        .expect("init script registers");

    let snapshot = session
        .navigate(&fixture_url("init_script.html"))
        .await
        .expect("navigate succeeds");

    let result = extract_result(&snapshot).expect("result text present");
    assert_eq!(
        result, "flag=hello",
        "init script's value should be visible to the page's own first-run script: {snapshot}"
    );

    session.close().await.expect("session closes");
}

#[tokio::test]
async fn seeded_randomness_is_reproducible_and_seed_dependent() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping seeded_randomness_is_reproducible_and_seed_dependent: no installed browser found"
        );
        return;
    }

    let session = Session::launch("engine-test-seeded-random", true)
        .await
        .expect("session launches");

    session
        .seed_randomness(42)
        .await
        .expect("seeding randomness registers");

    let snapshot1 = session
        .navigate(&fixture_url("random_fixture.html"))
        .await
        .expect("first navigate succeeds");
    let values1 = extract_result(&snapshot1).expect("result text present on first navigate");

    let snapshot2 = session
        .navigate(&fixture_url("random_fixture.html"))
        .await
        .expect("second navigate succeeds");
    let values2 = extract_result(&snapshot2).expect("result text present on second navigate");

    assert_eq!(
        values1, values2,
        "same seed should reproduce the same Math.random() sequence across navigations"
    );

    // A later seed_randomness call registers another init script that runs
    // after the first, so its Math.random override wins on the next load.
    session
        .seed_randomness(99)
        .await
        .expect("re-seeding randomness registers");
    let snapshot3 = session
        .navigate(&fixture_url("random_fixture.html"))
        .await
        .expect("third navigate succeeds");
    let values3 = extract_result(&snapshot3).expect("result text present on third navigate");

    assert_ne!(
        values1, values3,
        "different seeds should produce different Math.random() sequences"
    );

    session.close().await.expect("session closes");
}
