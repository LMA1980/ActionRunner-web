use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Passed,
    Failed,
    Running,
    TimedOut,
}

impl StepStatus {
    pub fn label(&self) -> &'static str {
        match self {
            StepStatus::Passed => "✓ PASSED",
            StepStatus::Failed => "✗ FAILED",
            StepStatus::Running => "⏳ RUNNING",
            StepStatus::TimedOut => "⏱ TIMED OUT",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            StepStatus::Passed => "step-badge step-passed",
            StepStatus::Failed => "step-badge step-failed",
            StepStatus::Running => "step-badge step-running",
            StepStatus::TimedOut => "step-badge step-failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedStep {
    pub number: usize,
    pub total: usize,
    pub name: String,
    pub command: String,
    pub started_at: String,
    pub status: StepStatus,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ParsedLog {
    pub pre_step_output: String,
    pub steps: Vec<ParsedStep>,
    pub footer_output: String,
}

pub fn parse_job_log(raw: &str) -> ParsedLog {
    let mut pre_lines: Vec<&str> = Vec::new();
    let mut steps: Vec<ParsedStep> = Vec::new();
    let mut footer_lines: Vec<&str> = Vec::new();

    let lines: Vec<&str> = raw.lines().collect();
    let mut i = 0;
    let mut in_footer = false;

    while i < lines.len() {
        let line = lines[i];

        // Detect step header block:
        // --------------------------------------------------------------------------------
        // Step [1/5]: Verify Rust Formatting (cargo fmt)
        // Command:    cargo fmt --check
        // Started At: ...
        // --------------------------------------------------------------------------------
        if line.starts_with("Step [") && line.contains("]: ") {
            if let Some((num_part, name_part)) = line
                .strip_prefix("Step [")
                .and_then(|s| s.split_once("]: "))
            {
                let (number, total) = num_part
                    .split_once('/')
                    .and_then(|(n, t)| Some((n.parse::<usize>().ok()?, t.parse::<usize>().ok()?)))
                    .unwrap_or((steps.len() + 1, steps.len() + 1));

                let mut command = String::new();
                let mut started_at = String::new();
                i += 1;
                while i < lines.len() {
                    let hline = lines[i];
                    if let Some(cmd) = hline.strip_prefix("Command:") {
                        command = cmd.trim().to_string();
                    } else if let Some(st) = hline.strip_prefix("Started At:") {
                        started_at = st.trim().to_string();
                    } else if hline.starts_with("--------") {
                        i += 1;
                        break;
                    }
                    i += 1;
                }

                // Collect step body until next step divider or final summary divider
                let mut step_lines: Vec<&str> = Vec::new();
                let mut status = StepStatus::Running;

                while i < lines.len() {
                    let bline = lines[i];
                    if bline.starts_with("--------")
                        && i + 1 < lines.len()
                        && lines[i + 1].starts_with("Step [")
                    {
                        i += 1; // skip the opening divider of next step
                        break;
                    }
                    if bline.starts_with("========")
                        && i + 1 < lines.len()
                        && lines[i + 1].starts_with("All steps completed")
                    {
                        in_footer = true;
                        break;
                    }

                    if bline.contains("[ActionRunner-rs] Step '")
                        && bline.contains("completed successfully.")
                    {
                        status = StepStatus::Passed;
                    } else if bline.contains("[ActionRunner-rs ERROR] Step '") {
                        if bline.contains("timed out") {
                            status = StepStatus::TimedOut;
                        } else {
                            status = StepStatus::Failed;
                        }
                    }

                    step_lines.push(bline);
                    i += 1;
                }

                steps.push(ParsedStep {
                    number,
                    total,
                    name: name_part.trim().to_string(),
                    command,
                    started_at,
                    status,
                    output: step_lines.join("\n").trim().to_string(),
                });
                continue;
            }
        }

        if in_footer {
            footer_lines.push(line);
        } else if steps.is_empty() {
            // Skip the trailing step divider right before Step [1/N]
            if !(line.starts_with("--------")
                && i + 1 < lines.len()
                && lines[i + 1].starts_with("Step ["))
            {
                pre_lines.push(line);
            }
        } else {
            footer_lines.push(line);
        }

        i += 1;
    }

    ParsedLog {
        pre_step_output: pre_lines.join("\n").trim().to_string(),
        steps,
        footer_output: footer_lines.join("\n").trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_job_log_steps_and_statuses() {
        let sample = r#"================================================================================
ActionRunner-rs Job Execution: job-123-abcd1234
Event:      push
Repository: LMA1980/model-runner
================================================================================

[ActionRunner-rs] Loaded workflow file: .actionrunner/on-push.yml

Workflow Name: K3 Factory Pipeline
Total Steps:   2

--------------------------------------------------------------------------------
Step [1/2]: Verify Rust Formatting (cargo fmt)
Command:    cargo fmt --check
Started At: 2026-10-21T18:00:00Z
--------------------------------------------------------------------------------
[ActionRunner-rs] Step 'Verify Rust Formatting (cargo fmt)' completed successfully.
--------------------------------------------------------------------------------
Step [2/2]: Verify Rust Linter & Clippy Rules (cargo clippy)
Command:    cargo clippy --workspace --all-targets -- -D warnings
Started At: 2026-10-21T18:00:02Z
--------------------------------------------------------------------------------
Checking k3-core v0.1.0
[ActionRunner-rs ERROR] Step 'Verify Rust Linter & Clippy Rules (cargo clippy)' failed with exit code: 101
"#;

        let parsed = parse_job_log(sample);
        assert!(parsed
            .pre_step_output
            .contains("ActionRunner-rs Job Execution: job-123-abcd1234"));
        assert_eq!(parsed.steps.len(), 2);
        assert_eq!(parsed.steps[0].number, 1);
        assert_eq!(parsed.steps[0].total, 2);
        assert_eq!(parsed.steps[0].name, "Verify Rust Formatting (cargo fmt)");
        assert_eq!(parsed.steps[0].command, "cargo fmt --check");
        assert_eq!(parsed.steps[0].status, StepStatus::Passed);

        assert_eq!(parsed.steps[1].number, 2);
        assert_eq!(parsed.steps[1].status, StepStatus::Failed);
        assert!(parsed.steps[1].output.contains("Checking k3-core v0.1.0"));
    }
}
