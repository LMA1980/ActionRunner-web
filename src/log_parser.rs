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

    // If no legacy steps were found, parse distributed runner logs:
    // Format: "2026-10-24T12:31:24.898184+00:00 [rubicon] [clippy-check] [stderr] Updating crates.io index"
    if steps.is_empty() {
        pre_lines.clear();
        use std::collections::BTreeMap;
        let mut stages_map: BTreeMap<String, (String, String, Vec<String>, StepStatus)> =
            BTreeMap::new();
        let mut stage_order: Vec<String> = Vec::new();

        for line in raw.lines() {
            let parts: Vec<&str> = line.splitn(5, ' ').collect();
            if parts.len() == 5
                && parts[1].starts_with('[')
                && parts[1].ends_with(']')
                && parts[2].starts_with('[')
                && parts[2].ends_with(']')
            {
                let timestamp = parts[0];
                let node_id = &parts[1][1..parts[1].len() - 1];
                let stage_id = &parts[2][1..parts[2].len() - 1];
                let content = parts[4];

                if stage_id == "step-0" || stage_id == "git-sync" {
                    pre_lines.push(line);
                    continue;
                }

                if !stages_map.contains_key(stage_id) {
                    stage_order.push(stage_id.to_string());
                    stages_map.insert(
                        stage_id.to_string(),
                        (
                            node_id.to_string(),
                            timestamp.to_string(),
                            Vec::new(),
                            StepStatus::Passed,
                        ),
                    );
                }

                let entry = stages_map.get_mut(stage_id).unwrap();
                if content.contains("error:")
                    || content.contains("FAILED")
                    || content.contains("Diff in")
                {
                    entry.3 = StepStatus::Failed;
                }
                entry.2.push(line.to_string());
            } else if !line.trim().is_empty() {
                pre_lines.push(line);
            }
        }

        if !stage_order.is_empty() {
            let total = stage_order.len();
            for (idx, stage_id) in stage_order.iter().enumerate() {
                if let Some((node_id, started_at, lines, status)) = stages_map.remove(stage_id) {
                    steps.push(ParsedStep {
                        number: idx + 1,
                        total,
                        name: stage_id.clone(),
                        command: format!("Host: {} (Distributed Stage)", node_id),
                        started_at,
                        status,
                        output: lines.join("\n"),
                    });
                }
            }
        }
    }

    ParsedLog {
        pre_step_output: pre_lines.join("\n").trim().to_string(),
        steps,
        footer_output: footer_lines.join("\n").trim().to_string(),
    }
}

pub fn extract_stage_logs(
    raw: &str,
) -> (Vec<String>, std::collections::HashMap<String, Vec<String>>) {
    let mut step0_lines = Vec::new();
    let mut stage_logs: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for line in raw.lines() {
        let parts: Vec<&str> = line.splitn(5, ' ').collect();
        if parts.len() == 5
            && parts[1].starts_with('[')
            && parts[1].ends_with(']')
            && parts[2].starts_with('[')
            && parts[2].ends_with(']')
        {
            let stage_id = &parts[2][1..parts[2].len() - 1];
            if stage_id == "step-0" || stage_id == "git-sync" {
                step0_lines.push(line.to_string());
            } else {
                stage_logs
                    .entry(stage_id.to_string())
                    .or_default()
                    .push(line.to_string());
            }
        } else if !line.trim().is_empty() {
            step0_lines.push(line.to_string());
        }
    }

    (step0_lines, stage_logs)
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

    #[test]
    fn test_parse_distributed_cluster_log() {
        let sample = "2026-10-24T12:31:24.898184+00:00 [rubicon] [clippy-check] [stderr] Updating crates.io index\n2026-10-24T12:31:26.491124+00:00 [rubicon] [clippy-check] [stderr] Locking packages\n2026-10-24T12:39:19.894462+00:00 [pey-wail-deu] [native-k3] [stdout] running 4 tests\n2026-10-24T12:39:19.903154+00:00 [pey-wail-deu] [native-k3] [stdout] test result: ok. 4 passed\n";
        let parsed = parse_job_log(sample);
        assert_eq!(parsed.steps.len(), 2);
        assert_eq!(parsed.steps[0].name, "clippy-check");
        assert!(parsed.steps[0].command.contains("rubicon"));
        assert_eq!(parsed.steps[1].name, "native-k3");
        assert_eq!(
            parsed.steps[1].command,
            "Host: pey-wail-deu (Distributed Stage)"
        );
        assert_eq!(parsed.steps[1].status, StepStatus::Passed);
    }

    #[test]
    fn test_extract_stage_logs_and_step0() {
        let sample = "Job log initializing\n2026-10-24T12:00:00+00:00 [pey-wail-deu] [step-0] [stdout] Git submodule updated\n2026-10-24T12:00:01+00:00 [pey-wail-deu] [step-2] [stdout] Running clippy\n";
        let (step0, stage_logs) = extract_stage_logs(sample);
        assert_eq!(step0.len(), 2);
        assert!(step0[0].contains("Job log initializing"));
        assert!(step0[1].contains("Git submodule updated"));

        assert_eq!(stage_logs.len(), 1);
        let s2 = stage_logs.get("step-2").expect("step-2 present");
        assert_eq!(s2.len(), 1);
        assert!(s2[0].contains("Running clippy"));
    }
}
