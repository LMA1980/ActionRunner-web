use std::collections::{BTreeMap, BTreeSet, VecDeque};

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::node_locks::NodeExclusiveLockBadge;
use crate::models::{PipelineTaskItem, StageRecord, StageStatus};

/// Tier 1 (`OnActionTrigger`): Trigger action tabs (`push`, `pull_request`, `merge`, `tag`)
/// mapping to activated stage IDs in the 3-tier `.actionrunner/ActionRunner.yml` (`version: "3"`) model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnActionTrigger {
    pub action: String,
    pub display_label: String,
    pub short_label: String,
    pub activated_stage_ids: Vec<String>,
    pub is_job_trigger: bool,
}

/// Tier 2 (`PipelineStageTreeItem`): Topological DAG stage node containing nested Tier 3 `PipelineTaskItem`s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStageTreeItem {
    pub stage_id: String,
    pub name: String,
    pub status: StageStatus,
    pub wave_depth: usize,
    pub depends_on: Vec<String>,
    pub exclusive: bool,
    pub max_concurrent_per_node: u32,
    pub node_id: Option<String>,
    pub tags: Vec<String>,
    pub threads: u32,
    pub tasks: Vec<PipelineTaskItem>,
}

impl PipelineStageTreeItem {
    pub fn status_badge(&self) -> (&'static str, &'static str) {
        match &self.status {
            StageStatus::Pending => ("⏳ Pending", "status-pill status-queued"),
            StageStatus::Assigned { .. } => ("🚀 Assigned", "status-pill status-running"),
            StageStatus::Running { .. } => ("⚙️ Running", "status-pill status-running"),
            StageStatus::Completed { exit_code: 0, .. } => {
                ("✓ Passed", "status-pill status-passing")
            }
            StageStatus::Completed { .. } | StageStatus::Failed { .. } => {
                ("✕ Failed", "status-pill status-failing")
            }
            StageStatus::Skipped { .. } => ("⊘ Skipped", "status-pill status-skipped"),
        }
    }
}

/// Synthesizes Tier 3 `PipelineTaskItem`s when a stage originates from a legacy v2 workflow
/// or when explicit task entries are omitted from the stage payload.
pub fn synthesize_tasks_for_stage(stage: &StageRecord) -> Vec<PipelineTaskItem> {
    if !stage.tasks.is_empty() {
        return stage.tasks.clone();
    }

    let task_status = match &stage.status {
        StageStatus::Completed { exit_code: 0, .. } => "passed".to_string(),
        StageStatus::Completed { .. } | StageStatus::Failed { .. } => "failed".to_string(),
        StageStatus::Running { .. } | StageStatus::Assigned { .. } => "running".to_string(),
        StageStatus::Skipped { .. } => "skipped".to_string(),
        StageStatus::Pending => "pending".to_string(),
    };

    let duration_secs = match &stage.status {
        StageStatus::Completed { duration_secs, .. } => Some(*duration_secs as f64),
        _ => None,
    };

    let id_lower = stage.id.to_ascii_lowercase();
    let name_lower = stage.name.to_ascii_lowercase();
    let has_tag = |t: &str| stage.tags.iter().any(|tag| tag.eq_ignore_ascii_case(t));

    let (tool_name, args, profile) = if id_lower.contains("fmt")
        || name_lower.contains("fmt")
        || has_tag("fmt")
    {
        (
            "cargo-fmt".to_string(),
            vec!["--all".to_string(), "--".to_string(), "--check".to_string()],
            "strict-offline".to_string(),
        )
    } else if id_lower.contains("clippy") || name_lower.contains("clippy") || has_tag("clippy") {
        (
            "cargo-clippy".to_string(),
            vec![
                "--workspace".to_string(),
                "--all-targets".to_string(),
                "--".to_string(),
                "-D".to_string(),
                "warnings".to_string(),
            ],
            "strict-offline".to_string(),
        )
    } else if id_lower.contains("audit") || name_lower.contains("audit") || has_tag("audit") {
        (
            "cargo-audit".to_string(),
            vec!["--deny".to_string(), "warnings".to_string()],
            "ci-default".to_string(),
        )
    } else if id_lower.contains("cov") || name_lower.contains("coverage") || has_tag("coverage") {
        (
            "cargo-llvm-cov".to_string(),
            vec!["--workspace".to_string(), "--summary-only".to_string()],
            "ci-default".to_string(),
        )
    } else if id_lower.contains("wasm") || name_lower.contains("wasm") {
        (
            "cargo-build".to_string(),
            vec![
                "--target".to_string(),
                "wasm32-unknown-unknown".to_string(),
                "--release".to_string(),
            ],
            "strict-offline".to_string(),
        )
    } else if id_lower.contains("bench") || name_lower.contains("bench") || has_tag("bench") {
        (
            "cargo-bench".to_string(),
            vec!["--workspace".to_string()],
            "k3-matrix-bench".to_string(),
        )
    } else if id_lower.contains("test") || name_lower.contains("test") || has_tag("test") {
        (
            "cargo-test".to_string(),
            vec!["--workspace".to_string()],
            "ci-default".to_string(),
        )
    } else {
        (
            "cargo-check".to_string(),
            vec!["--workspace".to_string(), "--all-targets".to_string()],
            "ci-default".to_string(),
        )
    };

    let sandbox_profile = stage
        .sandbox_telemetry
        .as_ref()
        .map(|s| s.profile_name.clone())
        .or(Some(profile));

    vec![PipelineTaskItem {
        task_id: format!("{}-task-1", stage.id),
        tool_name,
        args,
        sandbox_profile,
        status: task_status,
        duration_secs,
    }]
}

/// Deterministically orders stages in topological execution order (Kahn's algorithm)
/// and calculates each stage's DAG wave depth (`wave_depth: usize`).
pub fn topological_sort_stages(stages: &[StageRecord]) -> Vec<PipelineStageTreeItem> {
    if stages.is_empty() {
        return Vec::new();
    }

    let known_ids: BTreeSet<&str> = stages.iter().map(|s| s.id.as_str()).collect();
    let mut in_degree: BTreeMap<&str, usize> = BTreeMap::new();
    let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut depth_map: BTreeMap<&str, usize> = BTreeMap::new();
    let stage_by_id: BTreeMap<&str, &StageRecord> =
        stages.iter().map(|s| (s.id.as_str(), s)).collect();

    for stage in stages {
        in_degree.entry(stage.id.as_str()).or_insert(0);
        depth_map.entry(stage.id.as_str()).or_insert(0);
        for dep in &stage.depends_on {
            if known_ids.contains(dep.as_str()) && dep != &stage.id {
                dependents
                    .entry(dep.as_str())
                    .or_default()
                    .push(stage.id.as_str());
                *in_degree.entry(stage.id.as_str()).or_insert(0) += 1;
            }
        }
    }

    let mut queue: VecDeque<&str> = VecDeque::new();
    for stage in stages {
        if in_degree.get(stage.id.as_str()).copied().unwrap_or(0) == 0 {
            queue.push_back(stage.id.as_str());
        }
    }

    let mut ordered_ids: Vec<&str> = Vec::with_capacity(stages.len());
    let mut visited: BTreeSet<&str> = BTreeSet::new();

    while let Some(curr_id) = queue.pop_front() {
        if !visited.insert(curr_id) {
            continue;
        }
        ordered_ids.push(curr_id);
        let curr_depth = depth_map.get(curr_id).copied().unwrap_or(0);

        if let Some(children) = dependents.get(curr_id) {
            for &child_id in children {
                let next_depth = curr_depth + 1;
                let entry_depth = depth_map.entry(child_id).or_insert(0);
                if next_depth > *entry_depth {
                    *entry_depth = next_depth;
                }
                if let Some(deg) = in_degree.get_mut(child_id) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        queue.push_back(child_id);
                    }
                }
            }
        }
    }

    // Fallback for any remaining stages if an unexpected cycle was present in untrusted input
    for stage in stages {
        if !visited.contains(stage.id.as_str()) {
            ordered_ids.push(stage.id.as_str());
        }
    }

    ordered_ids
        .into_iter()
        .filter_map(|id| {
            let s = stage_by_id.get(id)?;
            let wave_depth = depth_map.get(id).copied().unwrap_or(0);
            let exclusive = s.exclusive
                || s.tags.iter().any(|t| {
                    t.eq_ignore_ascii_case("exclusive") || t.eq_ignore_ascii_case("bench")
                });
            let max_concurrent_per_node = if exclusive {
                1
            } else {
                s.max_concurrent_per_node.max(1)
            };
            Some(PipelineStageTreeItem {
                stage_id: s.id.clone(),
                name: s.name.clone(),
                status: s.status.clone(),
                wave_depth,
                depends_on: s.depends_on.clone(),
                exclusive,
                max_concurrent_per_node,
                node_id: s.execution_node().map(str::to_string),
                tags: s.tags.clone(),
                threads: s.threads,
                tasks: synthesize_tasks_for_stage(s),
            })
        })
        .collect()
}

/// Builds the Tier 1 `OnActionTrigger` definitions (`push`, `pull_request`, `merge`, `tag`)
/// and resolves the transitive stage subgraph activated by each trigger.
pub fn build_on_action_triggers(
    stages: &[PipelineStageTreeItem],
    job_event_type: &str,
) -> Vec<OnActionTrigger> {
    let normalized_event = match job_event_type.trim().to_ascii_lowercase().as_str() {
        "pullrequest" | "pull_request" | "pr" => "pull_request",
        "merge" | "post-merge" => "merge",
        "tag" | "release" => "tag",
        _ => "push",
    };

    let all_ids: Vec<String> = stages.iter().map(|s| s.stage_id.clone()).collect();
    let pr_ids: Vec<String> = stages
        .iter()
        .filter(|s| !s.stage_id.contains("audit") && !s.stage_id.contains("release"))
        .map(|s| s.stage_id.clone())
        .collect();
    let tag_ids: Vec<String> = stages
        .iter()
        .filter(|s| {
            s.stage_id.contains("fmt")
                || s.stage_id.contains("clippy")
                || s.stage_id.contains("build")
                || s.stage_id.contains("wasm")
                || s.stage_id.contains("release")
        })
        .map(|s| s.stage_id.clone())
        .collect();

    let resolve_non_empty = |subset: Vec<String>| -> Vec<String> {
        if subset.is_empty() {
            all_ids.clone()
        } else {
            subset
        }
    };

    vec![
        OnActionTrigger {
            action: "push".to_string(),
            display_label: "on: push".to_string(),
            short_label: "push".to_string(),
            activated_stage_ids: all_ids.clone(),
            is_job_trigger: normalized_event == "push",
        },
        OnActionTrigger {
            action: "pull_request".to_string(),
            display_label: "on: pull_request".to_string(),
            short_label: "PR".to_string(),
            activated_stage_ids: if normalized_event == "pull_request" {
                all_ids.clone()
            } else {
                resolve_non_empty(pr_ids)
            },
            is_job_trigger: normalized_event == "pull_request",
        },
        OnActionTrigger {
            action: "merge".to_string(),
            display_label: "on: merge".to_string(),
            short_label: "merge".to_string(),
            activated_stage_ids: all_ids.clone(),
            is_job_trigger: normalized_event == "merge",
        },
        OnActionTrigger {
            action: "tag".to_string(),
            display_label: "on: tag".to_string(),
            short_label: "tag".to_string(),
            activated_stage_ids: resolve_non_empty(tag_ids),
            is_job_trigger: normalized_event == "tag",
        },
    ]
}

/// Filters `PipelineStageTreeItem`s by selected `OnAction` trigger tab (`"all"` or trigger action)
/// and search query matching stage ID, stage name, dependency, or `ToolAdapter` task name/args.
pub fn filter_pipeline_stages(
    stages: &[PipelineStageTreeItem],
    triggers: &[OnActionTrigger],
    selected_trigger: &str,
    filter_query: &str,
) -> Vec<PipelineStageTreeItem> {
    let trig = selected_trigger.trim().to_ascii_lowercase();
    let query = filter_query.trim().to_ascii_lowercase();

    let allowed_stage_ids: Option<BTreeSet<&str>> = if trig.is_empty() || trig == "all" {
        None
    } else {
        triggers
            .iter()
            .find(|t| t.action.eq_ignore_ascii_case(&trig))
            .map(|t| t.activated_stage_ids.iter().map(String::as_str).collect())
    };

    stages
        .iter()
        .filter(|stage| {
            if let Some(ref allowed) = allowed_stage_ids {
                if !allowed.contains(stage.stage_id.as_str()) {
                    return false;
                }
            }
            if !query.is_empty() {
                let in_stage_id = stage.stage_id.to_ascii_lowercase().contains(&query);
                let in_stage_name = stage.name.to_ascii_lowercase().contains(&query);
                let in_deps = stage
                    .depends_on
                    .iter()
                    .any(|d| d.to_ascii_lowercase().contains(&query));
                let in_tasks = stage.tasks.iter().any(|t| {
                    t.task_id.to_ascii_lowercase().contains(&query)
                        || t.tool_name.to_ascii_lowercase().contains(&query)
                        || t.args
                            .iter()
                            .any(|a| a.to_ascii_lowercase().contains(&query))
                });
                if !(in_stage_id || in_stage_name || in_deps || in_tasks) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

/// Renders a deterministic HTML snapshot of the 3-tier `OnAction -> Stages -> Tasks` hierarchy
/// for structural and mobile-viewport invariant verification (`AGENTS.md §8`).
pub fn render_pipeline_tree_html(
    stages: &[PipelineStageTreeItem],
    triggers: &[OnActionTrigger],
    selected_trigger: &str,
    filter_query: &str,
) -> String {
    let visible = filter_pipeline_stages(stages, triggers, selected_trigger, filter_query);
    let mut html = String::new();
    html.push_str(
        r#"<div class="pipeline-tree-container"><div class="status-tabs pipeline-trigger-tabs" style="overflow-x: auto; max-width: 100%;">"#,
    );
    for trig in triggers {
        let active_cls = if trig.action == selected_trigger {
            "tab-btn tab-active"
        } else {
            "tab-btn"
        };
        html.push_str(&format!(
            r#"<button type="button" class="{}"><span class="tab-label-full">{} ({})</span><span class="tab-label-short">{}</span></button>"#,
            active_cls,
            trig.display_label,
            trig.activated_stage_ids.len(),
            trig.short_label
        ));
    }
    html.push_str("</div>");
    html.push_str(&format!(
        r#"<input type="text" class="search-input pipeline-filter-input" style="font-size: 16px;" value="{}" />"#,
        filter_query
    ));

    for stage in &visible {
        let deps_str = if stage.depends_on.is_empty() {
            "root (wave 0)".to_string()
        } else {
            format!("depends_on: {}", stage.depends_on.join(", "))
        };
        html.push_str(&format!(
            r#"<div class="pipeline-stage-node" data-stage-id="{}" data-wave="{}"><span class="stage-wave-badge">Wave {}</span><span class="stage-name">{}</span><span class="stage-dag-deps">{}</span><span class="stage-concurrency">max_concurrent_per_node: {}</span>"#,
            stage.stage_id,
            stage.wave_depth,
            stage.wave_depth,
            stage.name,
            deps_str,
            stage.max_concurrent_per_node
        ));
        if stage.exclusive {
            html.push_str(r#"<span class="exclusive-lock-badge">🔒 EXCLUSIVE LOCK</span>"#);
        }
        html.push_str(r#"<ul class="pipeline-tasks-list">"#);
        for task in &stage.tasks {
            let argv = if task.args.is_empty() {
                task.tool_name.clone()
            } else {
                format!("{} {}", task.tool_name, task.args.join(" "))
            };
            html.push_str(&format!(
                r#"<li class="pipeline-task-item" data-task-id="{}"><code class="task-tool-argv">{}</code><span class="task-status">{}</span></li>"#,
                task.task_id, argv, task.status
            ));
        }
        html.push_str("</ul></div>");
    }
    html.push_str("</div>");
    html
}

/// 3-Tier Declarative `OnAction -> Stages (DAG) -> Tasks (ToolAdapter)` Execution Tree Component (`#15`).
#[component]
pub fn PipelineTreeView(stages: Vec<StageRecord>, active_event: String) -> impl IntoView {
    let sorted_stages = topological_sort_stages(&stages);
    let triggers = build_on_action_triggers(&sorted_stages, &active_event);

    let selected_trigger = RwSignal::new(String::from("all"));
    let stage_filter_query = RwSignal::new(String::new());

    let triggers_for_tabs = triggers.clone();
    let stages_memo = StoredValue::new(sorted_stages);
    let triggers_memo = StoredValue::new(triggers);

    view! {
        <section class="panel pipeline-tree-panel">
            <div class="panel-header">
                <div>
                    <h2 class="panel-title">"3-Tier Declarative Pipeline Tree (OnAction → Stages → Tasks)"</h2>
                    <div class="muted-text" style="font-size: 11px; margin-top: 2px;">
                        "ActionRunner.yml v3 topological DAG execution hierarchy & typed ToolAdapter dispatch"
                    </div>
                </div>

                <div class="filter-bar">
                    <div
                        class="status-tabs pipeline-trigger-tabs"
                        role="tablist"
                        aria-label="Filter pipeline stages by OnAction trigger"
                        style="overflow-x: auto; max-width: 100%; box-sizing: border-box;"
                    >
                        <button
                            type="button"
                            role="tab"
                            class=move || if selected_trigger.get() == "all" { "tab-btn tab-active" } else { "tab-btn" }
                            on:click=move |_| selected_trigger.set("all".to_string())
                        >
                            <span class="tab-label-full">"All Triggers"</span>
                            <span class="tab-label-short">"All"</span>
                        </button>
                        {triggers_for_tabs.into_iter().map(|trig| {
                            let act = trig.action.clone();
                            let act_click = trig.action.clone();
                            let full_lbl = if trig.is_job_trigger {
                                format!("● {} ({})", trig.display_label, trig.activated_stage_ids.len())
                            } else {
                                format!("{} ({})", trig.display_label, trig.activated_stage_ids.len())
                            };
                            let short_lbl = if trig.is_job_trigger {
                                format!("● {}", trig.short_label)
                            } else {
                                trig.short_label.clone()
                            };
                            view! {
                                <button
                                    type="button"
                                    role="tab"
                                    class=move || if selected_trigger.get() == act { "tab-btn tab-active" } else { "tab-btn" }
                                    on:click=move |_| selected_trigger.set(act_click.clone())
                                >
                                    <span class="tab-label-full">{full_lbl}</span>
                                    <span class="tab-label-short">{short_lbl}</span>
                                </button>
                            }
                        }).collect_view()}
                    </div>

                    <input
                        type="text"
                        class="search-input pipeline-filter-input"
                        style="font-size: 16px;"
                        aria-label="Filter stages or ToolAdapter tasks"
                        placeholder="Filter stage, depends_on, or ToolAdapter..."
                        prop:value=move || stage_filter_query.get()
                        on:input=move |ev| stage_filter_query.set(event_target_value(&ev))
                    />
                </div>
            </div>

            <div class="pipeline-tree-body">
                {move || {
                    let all_s = stages_memo.get_value();
                    let all_t = triggers_memo.get_value();
                    let sel_t = selected_trigger.get();
                    let q = stage_filter_query.get();
                    let filtered = filter_pipeline_stages(&all_s, &all_t, &sel_t, &q);

                    if filtered.is_empty() {
                        view! {
                            <div class="queue-empty-list">
                                "No declarative pipeline stages match the selected OnAction trigger or filter."
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="pipeline-stages-tree">
                                {filtered.into_iter().map(|stage| {
                                    let (status_lbl, status_cls) = stage.status_badge();
                                    let wave_lbl = format!("Wave {}", stage.wave_depth);
                                    let node_name = stage.node_id.clone().unwrap_or_else(|| "awaiting-node".to_string());
                                    let deps = stage.depends_on.clone();
                                    let tasks = stage.tasks.clone();
                                    let is_exclusive = stage.exclusive;
                                    let stage_id_for_lock = stage.stage_id.clone();
                                    let node_for_lock = stage.node_id.clone().unwrap_or_else(|| "worker-node".to_string());
                                    let active_slots = u32::from(matches!(stage.status, StageStatus::Running { .. } | StageStatus::Assigned { .. }));
                                    let max_conc = stage.max_concurrent_per_node;

                                    view! {
                                        <details class="pipeline-stage-card" open=true>
                                            <summary class="pipeline-stage-summary">
                                                <div class="pipeline-stage-left">
                                                    <span class="stage-wave-pill mono">{wave_lbl}</span>
                                                    <span class=status_cls>{status_lbl}</span>
                                                    <span class="pipeline-stage-title">{stage.name.clone()}</span>
                                                    <span class="mono-chip mono">{stage.stage_id.clone()}</span>
                                                </div>
                                                <div class="pipeline-stage-right">
                                                    <NodeExclusiveLockBadge
                                                        node_id=node_for_lock
                                                        exclusive_locked=is_exclusive
                                                        owning_stage_id=Some(stage_id_for_lock)
                                                        active_slots=active_slots
                                                        max_concurrent_per_node=max_conc
                                                    />
                                                    <span class="mono-chip mono" title="Assigned cluster worker host">
                                                        {format!("Host: {}", node_name)}
                                                    </span>
                                                </div>
                                            </summary>

                                            <div class="pipeline-stage-details">
                                                // DAG Dependencies Row
                                                <div class="pipeline-dag-row">
                                                    <span class="sub-row-label">"DAG EDGES (depends_on)"</span>
                                                    {if deps.is_empty() {
                                                        view! {
                                                            <span class="dag-edge-badge dag-edge-root mono">"Root Stage (no upstream dependencies)"</span>
                                                        }.into_any()
                                                    } else {
                                                        view! {
                                                            <div class="dag-edges-list">
                                                                {deps.into_iter().map(|dep| view! {
                                                                    <span class="dag-edge-badge mono">{format!("← {}", dep)}</span>
                                                                }).collect_view()}
                                                            </div>
                                                        }.into_any()
                                                    }}
                                                </div>

                                                // Tier 3 Tasks (ToolAdapter invocations)
                                                <div class="pipeline-tasks-section">
                                                    <div class="sub-row-label" style="margin-bottom: 6px;">
                                                        {format!("TIER 3 TASKS — TOOLADAPTER REGISTRY ({})", tasks.len())}
                                                    </div>
                                                    <div class="pipeline-tasks-grid">
                                                        {tasks.into_iter().map(|task| {
                                                            let tool = task.tool_name.clone();
                                                            let argv_str = if task.args.is_empty() {
                                                                tool.clone()
                                                            } else {
                                                                format!("{} {}", tool, task.args.join(" "))
                                                            };
                                                            let prof = task.sandbox_profile.clone().unwrap_or_else(|| "ci-default".to_string());
                                                            let dur_txt = task.duration_secs.map(|d| format!("{:.1}s", d)).unwrap_or_else(|| "—".to_string());
                                                            let task_badge_cls = match task.status.as_str() {
                                                                "passed" => "step-badge step-passed",
                                                                "failed" => "step-badge step-failed",
                                                                "running" => "step-badge step-running",
                                                                _ => "step-badge",
                                                            };
                                                            view! {
                                                                <div class="pipeline-task-row">
                                                                    <div class="pipeline-task-main">
                                                                        <span class=task_badge_cls>{task.status.to_ascii_uppercase()}</span>
                                                                        <span class="mono-chip mono">{task.task_id}</span>
                                                                        <code class="step-cmd mono">{argv_str}</code>
                                                                    </div>
                                                                    <div class="pipeline-task-meta">
                                                                        <span class="sandbox-profile-chip mono">{format!("profile: {}", prof)}</span>
                                                                        <span class="muted-text mono">{dur_txt}</span>
                                                                    </div>
                                                                </div>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                </div>
                                            </div>
                                        </details>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_stage(
        id: &str,
        name: &str,
        depends_on: Vec<&str>,
        exclusive: bool,
        status: StageStatus,
    ) -> StageRecord {
        StageRecord {
            id: id.to_string(),
            name: name.to_string(),
            tags: vec![id.to_string()],
            threads: 2,
            toolchain: None,
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            status,
            assigned_node: Some("worker-node".to_string()),
            exclusive,
            max_concurrent_per_node: if exclusive { 1 } else { 2 },
            tasks: Vec::new(),
            sandbox_telemetry: None,
        }
    }

    #[test]
    fn test_topological_sort_stages_and_wave_depths() {
        // Provide stages out of topological order to verify Kahn's algorithm sorting
        let stages = vec![
            sample_stage(
                "wasm-build",
                "Build WASM Release",
                vec!["fmt-check", "clippy-check"],
                false,
                StageStatus::Completed {
                    duration_secs: 18,
                    exit_code: 0,
                },
            ),
            sample_stage(
                "k3-bench",
                "Hardware Benchmark Suite",
                vec!["wasm-build"],
                true,
                StageStatus::Pending,
            ),
            sample_stage(
                "fmt-check",
                "Verify Rust Formatting",
                vec![],
                false,
                StageStatus::Completed {
                    duration_secs: 2,
                    exit_code: 0,
                },
            ),
            sample_stage(
                "clippy-check",
                "Verify Clippy Lints",
                vec![],
                false,
                StageStatus::Completed {
                    duration_secs: 9,
                    exit_code: 0,
                },
            ),
        ];

        let sorted = topological_sort_stages(&stages);
        assert_eq!(sorted.len(), 4);

        // Wave 0 roots come first
        assert_eq!(sorted[0].stage_id, "fmt-check");
        assert_eq!(sorted[0].wave_depth, 0);
        assert_eq!(sorted[1].stage_id, "clippy-check");
        assert_eq!(sorted[1].wave_depth, 0);

        // Wave 1 depends on Wave 0
        assert_eq!(sorted[2].stage_id, "wasm-build");
        assert_eq!(sorted[2].wave_depth, 1);
        assert_eq!(sorted[2].depends_on, vec!["fmt-check", "clippy-check"]);

        // Wave 2 depends on Wave 1 and has exclusive lock enabled
        assert_eq!(sorted[3].stage_id, "k3-bench");
        assert_eq!(sorted[3].wave_depth, 2);
        assert!(sorted[3].exclusive);
        assert_eq!(sorted[3].max_concurrent_per_node, 1);
    }

    #[test]
    fn test_hierarchy_construction_and_mobile_viewport_invariants() {
        let stages = vec![
            sample_stage(
                "fmt-check",
                "Verify Formatting",
                vec![],
                false,
                StageStatus::Completed {
                    duration_secs: 1,
                    exit_code: 0,
                },
            ),
            sample_stage(
                "coverage-test",
                "Run Coverage Suite",
                vec!["fmt-check"],
                true,
                StageStatus::Completed {
                    duration_secs: 14,
                    exit_code: 0,
                },
            ),
        ];

        let tree_items = topological_sort_stages(&stages);
        let triggers = build_on_action_triggers(&tree_items, "push");
        assert_eq!(triggers.len(), 4);
        assert!(triggers
            .iter()
            .any(|t| t.action == "push" && t.is_job_trigger));

        let html = render_pipeline_tree_html(&tree_items, &triggers, "push", "");
        // Verify mobile viewport invariants (AGENTS.md §8)
        assert!(html.contains("overflow-x: auto"));
        assert!(html.contains("font-size: 16px"));
        // Verify 3-tier hierarchy (OnAction -> Stages -> Tasks) & DAG badges
        assert!(html.contains("on: push"));
        assert!(html.contains("depends_on: fmt-check"));
        assert!(html.contains("cargo-fmt --all -- --check"));
        assert!(html.contains("cargo-llvm-cov --workspace --summary-only"));
        assert!(html.contains("🔒 EXCLUSIVE LOCK"));
    }
}
