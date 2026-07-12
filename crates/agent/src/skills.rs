//! Reusable prompt files (agent-harness spec: "Skills"). Plain Markdown,
//! name = file stem. Resolution order: project-local `./.truewright/skills/`
//! (so a repo can pin its own house style/known-gotchas for whoever runs
//! `truewright agent` against it) -> the per-user `<data-dir>/truewright/skills/` ->
//! each configured extra directory, in order -- first hit wins.

use crate::error::{AgentError, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub body: String,
}

/// Resolves every name in `names`, in order, erroring clearly (not
/// silently skipping) on the first one not found anywhere -- matches
/// this project's convention elsewhere (e.g. an unknown trained-motion
/// profile) of failing loud rather than silently proceeding with less
/// than what was asked for.
#[allow(clippy::result_large_err)]
pub fn resolve(names: &[String], search_dirs: &[PathBuf]) -> Result<Vec<Skill>> {
    names
        .iter()
        .map(|name| resolve_one(name, search_dirs))
        .collect()
}

#[allow(clippy::result_large_err)]
fn resolve_one(name: &str, search_dirs: &[PathBuf]) -> Result<Skill> {
    for dir in search_dirs {
        let path = dir.join(format!("{name}.md"));
        if path.is_file() {
            let raw = std::fs::read_to_string(&path)?;
            return Ok(Skill {
                name: name.to_string(),
                body: strip_front_matter(&raw),
            });
        }
    }
    Err(AgentError::UnknownSkill(name.to_string()))
}

/// The default search path, in priority order: project-local, then the
/// per-user data dir, then any extra configured dirs.
pub fn default_search_dirs(truewright_data_dir: &Path, extra: &[String]) -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("./.truewright/skills"),
        truewright_data_dir.join("skills"),
    ];
    dirs.extend(extra.iter().map(PathBuf::from));
    dirs
}

/// Strips a leading `---\n...\n---\n` front-matter block, if present.
/// Nothing currently reads individual front-matter keys (e.g. a
/// `description:`) -- this only exists so a skill file *can* carry one
/// (for a human skimming the file, or a future use) without it leaking
/// into the prompt text verbatim.
fn strip_front_matter(raw: &str) -> String {
    let trimmed = raw.trim_start();
    if let Some(rest) = trimmed.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            return rest[end + 5..].trim_start().to_string();
        }
        if let Some(end) = rest.find("\n---") {
            if rest[end + 4..].trim().is_empty() {
                return String::new();
            }
        }
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_skills_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "truewright-agent-skills-test-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolves_a_skill_from_the_first_matching_directory() {
        let dir = temp_skills_dir("resolve");
        std::fs::write(
            dir.join("checkout-flow.md"),
            "Always verify the total before submitting.",
        )
        .unwrap();

        let skills =
            resolve(&["checkout-flow".to_string()], std::slice::from_ref(&dir)).expect("resolves");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "checkout-flow");
        assert!(skills[0].body.contains("verify the total"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_skill_errors_clearly_rather_than_silently_skipping() {
        let dir = temp_skills_dir("missing");
        let err = resolve(&["does-not-exist".to_string()], std::slice::from_ref(&dir))
            .expect_err("must error");
        assert!(matches!(err, AgentError::UnknownSkill(name) if name == "does-not-exist"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn front_matter_block_is_stripped() {
        let raw = "---\ndescription: test skill\n---\nActual body text here.";
        assert_eq!(strip_front_matter(raw), "Actual body text here.");
    }

    #[test]
    fn body_without_front_matter_is_returned_unchanged() {
        let raw = "Just a plain skill body, no front matter.";
        assert_eq!(strip_front_matter(raw), raw);
    }

    #[test]
    fn first_directory_in_search_order_wins_on_a_name_collision() {
        let first = temp_skills_dir("collision-first");
        let second = temp_skills_dir("collision-second");
        std::fs::write(first.join("dup.md"), "from first dir").unwrap();
        std::fs::write(second.join("dup.md"), "from second dir").unwrap();

        let skills =
            resolve(&["dup".to_string()], &[first.clone(), second.clone()]).expect("resolves");
        assert_eq!(skills[0].body, "from first dir");

        std::fs::remove_dir_all(&first).ok();
        std::fs::remove_dir_all(&second).ok();
    }
}
