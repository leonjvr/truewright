//! System prompt construction (agent-harness spec: "System prompt").
//! Adapted from the MCP server's own `instructions` string
//! (`crates/mcp/src/lib.rs`) -- the ref conventions and actionability
//! rules an agent needs to know are identical regardless of whether the
//! model driving them is an outer MCP host or `truewright`'s own harness; this
//! is restricted to the tool subset `tools::tool_defs()` actually
//! exposes and adds the harness's own termination contract.

const BASE_INSTRUCTIONS: &str = "You are driving a real Chrome/Edge browser to complete a task. \
Refs come from the snapshot text, e.g. `[e6]` -> ref \"e6\". Actions do not auto-return a new \
snapshot; call snapshot again after an action that may have changed the page. wait_for polls \
until text appears (or times out) -- prefer it over guessing a fixed delay. assert checks \
immediately (no polling) and is how you confirm a specific outcome actually happened; treat a \
failed assertion as a real problem to solve, not something to ignore. A popup or new tab opened \
as a side effect of an action (e.g. \"Sign in with Google\") attaches automatically but does NOT \
become active on its own -- call list_pages to see it and switch_page to start driving it. \
run_yaml runs a whole script of steps in one call when you already know the exact sequence \
needed, stopping at the first failing step.\n\n\
You MUST end every task by calling exactly one of task_complete or task_failed -- do not just \
stop responding. Call task_complete only once you have verified the outcome (e.g. via assert or \
by reading the snapshot), not merely after taking an action that you expect worked.";

/// Builds the full system prompt: base instructions, the task itself,
/// then any attached skills, then optional freeform caller guidance (MCP's
/// `browser_run_task.guidance` param, or a CLI-equivalent) -- in that
/// order, so skills read as established practice and guidance reads as
/// this-specific-call context layered on top.
pub fn system_prompt(
    task: &str,
    skills: &[crate::skills::Skill],
    guidance: Option<&str>,
) -> String {
    let mut prompt = String::new();
    prompt.push_str(BASE_INSTRUCTIONS);
    prompt.push_str("\n\n## Task\n");
    prompt.push_str(task);

    for skill in skills {
        prompt.push_str(&format!("\n\n## Skill: {}\n", skill.name));
        prompt.push_str(&skill.body);
    }

    if let Some(guidance) = guidance {
        if !guidance.is_empty() {
            prompt.push_str("\n\n## Caller guidance\n");
            prompt.push_str(guidance);
        }
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::Skill;

    #[test]
    fn prompt_includes_task_skills_and_guidance_in_order() {
        let skills = vec![Skill {
            name: "checkout-flow".to_string(),
            body: "Always verify the cart total before submitting.".to_string(),
        }];
        let prompt = system_prompt(
            "Buy the cheapest item",
            &skills,
            Some("Use the test credit card."),
        );

        let task_pos = prompt.find("Buy the cheapest item").expect("task present");
        let skill_pos = prompt
            .find("Always verify the cart total")
            .expect("skill present");
        let guidance_pos = prompt
            .find("Use the test credit card")
            .expect("guidance present");
        assert!(
            task_pos < skill_pos && skill_pos < guidance_pos,
            "expected task, then skills, then guidance"
        );
        assert!(
            prompt.contains("task_complete") && prompt.contains("task_failed"),
            "termination contract must be present"
        );
    }

    #[test]
    fn empty_guidance_is_omitted_not_rendered_as_an_empty_section() {
        let prompt = system_prompt("do something", &[], Some(""));
        assert!(!prompt.contains("## Caller guidance"));
    }
}
