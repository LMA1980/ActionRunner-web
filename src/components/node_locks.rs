use leptos::prelude::*;

use crate::models::{ClusterNodeView, JobRecord};

/// Formats the visual state and HTML representation of a node's concurrency & exclusive lock badge.
pub fn render_node_lock_badge_html(
    node_id: &str,
    exclusive_locked: bool,
    owning_stage_id: Option<&str>,
    active_slots: u32,
    max_concurrent_per_node: u32,
) -> String {
    let max_c = max_concurrent_per_node.max(1);
    let slot_pill = format!(
        r#"<span class="slot-utilization-pill mono">{}/{} slots</span>"#,
        active_slots, max_c
    );

    if exclusive_locked {
        let stage_suffix = owning_stage_id
            .filter(|s| !s.is_empty())
            .map(|s| format!(" • {}", s))
            .unwrap_or_default();
        format!(
            r#"<span class="node-lock-group"><span class="exclusive-lock-badge" data-node="{}">🔒 EXCLUSIVE LOCK ({}{})</span>{}</span>"#,
            node_id, node_id, stage_suffix, slot_pill
        )
    } else {
        format!(r#"<span class="node-lock-group">{}</span>"#, slot_pill)
    }
}

/// Real-Time Visual Indicator Badge for Exclusive Node Stage Locks (`#16`).
#[component]
pub fn NodeExclusiveLockBadge(
    node_id: String,
    exclusive_locked: bool,
    #[prop(default = None)] owning_stage_id: Option<String>,
    active_slots: u32,
    max_concurrent_per_node: u32,
) -> impl IntoView {
    let max_c = max_concurrent_per_node.max(1);
    let slot_text = format!("{}/{} slots", active_slots, max_c);
    let lock_label = {
        let stage_part = owning_stage_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| format!(" • {}", s))
            .unwrap_or_default();
        format!("🔒 EXCLUSIVE LOCK ({}{})", node_id, stage_part)
    };
    let lock_title = format!(
        "Exclusive full-node stage lock active on '{}' (all concurrent stage leases blocked until release)",
        node_id
    );

    view! {
        <span class="node-lock-group">
            {exclusive_locked.then(|| view! {
                <span class="exclusive-lock-badge mono" title=lock_title>
                    {lock_label}
                </span>
            })}
            <span class="slot-utilization-pill mono" title="Active stage slots / max_concurrent_per_node ceiling">
                {slot_text}
            </span>
        </span>
    }
}

/// Cluster-wide summary strip showing real-time exclusive node stage locks across fabric nodes (`#16`).
#[component]
pub fn ClusterNodeLocksBar(
    nodes: Signal<Vec<ClusterNodeView>>,
    jobs: Signal<Vec<JobRecord>>,
) -> impl IntoView {
    view! {
        {move || {
            let current_nodes = nodes.get();
            let current_jobs = jobs.get();
            let locked_nodes: Vec<(String, Option<String>, u32, u32)> = current_nodes
                .iter()
                .filter_map(|n| {
                    let (locked, stage_id, max_c) = n.resolve_exclusive_lock(&current_jobs);
                    if locked {
                        Some((n.node_id.clone(), stage_id, n.active_slots.max(1), max_c))
                    } else {
                        None
                    }
                })
                .collect();

            if locked_nodes.is_empty() {
                view! { <div style="display: none;"></div> }.into_any()
            } else {
                view! {
                    <div class="cluster-locks-banner">
                        <span class="locks-banner-title">"🔒 ACTIVE EXCLUSIVE NODE STAGE LOCKS:"</span>
                        <div class="locks-banner-items">
                            {locked_nodes.into_iter().map(|(nid, stage_id, active_s, max_c)| view! {
                                <NodeExclusiveLockBadge
                                    node_id=nid
                                    exclusive_locked=true
                                    owning_stage_id=stage_id
                                    active_slots=active_s
                                    max_concurrent_per_node=max_c
                                />
                            }).collect_view()}
                        </div>
                    </div>
                }.into_any()
            }
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Job, JobStatus, StageRecord, StageStatus};
    use chrono::Utc;

    #[test]
    fn test_exclusive_lock_badge_renders_when_locked_and_clears_when_released() {
        let locked_html =
            render_node_lock_badge_html("worker-node", true, Some("k3-matrix-bench"), 1, 1);
        assert!(locked_html.contains("🔒 EXCLUSIVE LOCK (worker-node • k3-matrix-bench)"));
        assert!(locked_html.contains("1/1 slots"));

        let unlocked_html = render_node_lock_badge_html("worker-node", false, None, 0, 1);
        assert!(!unlocked_html.contains("EXCLUSIVE LOCK"));
        assert!(unlocked_html.contains("0/1 slots"));
    }

    #[test]
    fn test_cluster_node_view_resolve_exclusive_lock() {
        let mut node = ClusterNodeView {
            node_id: "control-node".to_string(),
            arch: "x86_64".to_string(),
            tags: vec!["fmt".to_string(), "clippy".to_string()],
            max_slots: 1,
            active_slots: 1,
            max_concurrent_per_node: Some(1),
            exclusive_locked: true,
            exclusive_stage_id: Some("security-audit".to_string()),
            ram_total_mb: 16384,
            ram_used_mb: 4096,
            cpu_load_pct: 45.0,
            btrfs_enabled: true,
            toolchains: vec!["stable".to_string()],
            last_seen_secs_ago: 1,
            is_online: true,
        };

        let (locked, stage_id, max_c) = node.resolve_exclusive_lock(&[]);
        assert!(locked);
        assert_eq!(stage_id.as_deref(), Some("security-audit"));
        assert_eq!(max_c, 1);

        // Release lock
        node.exclusive_locked = false;
        node.exclusive_stage_id = None;
        node.active_slots = 0;
        let (locked_after, stage_after, _) = node.resolve_exclusive_lock(&[]);
        assert!(!locked_after);
        assert!(stage_after.is_none());

        // Infer lock from active exclusive stage in JobRecord
        let job_with_exclusive_stage = JobRecord {
            job: Job {
                id: "job-99".to_string(),
                event_type: "push".to_string(),
                repo_name: "LMA1980/ActionRunner-web".to_string(),
                clone_url: "https://github.com/LMA1980/ActionRunner-web.git".to_string(),
                commit_sha: "abcdef1234567890".to_string(),
                ref_name: "refs/heads/main".to_string(),
                sender: "LMA1980".to_string(),
                queued_at: Utc::now(),
            },
            status: JobStatus::Running {
                started_at: Utc::now(),
            },
            log_file: "logs/job-99.log".to_string(),
            updated_at: Utc::now(),
            metrics: None,
            queue_position: None,
            elapsed_secs: Some(5),
            stages: vec![StageRecord {
                id: "exclusive-bench".to_string(),
                name: "Exclusive Benchmark".to_string(),
                tags: vec!["bench".to_string()],
                threads: 6,
                toolchain: None,
                depends_on: vec![],
                status: StageStatus::Running {
                    node_id: "control-node".to_string(),
                    started_at: Utc::now(),
                    pid: Some(4321),
                },
                assigned_node: Some("control-node".to_string()),
                exclusive: true,
                max_concurrent_per_node: 1,
                tasks: vec![],
                sandbox_telemetry: None,
            }],
            sandbox_telemetry: vec![],
            security_violations: vec![],
            traceparent: None,
            trace_id: None,
        };

        let (inferred_locked, inferred_stage, _) =
            node.resolve_exclusive_lock(&[job_with_exclusive_stage]);
        assert!(inferred_locked);
        assert_eq!(inferred_stage.as_deref(), Some("exclusive-bench"));
    }
}
