use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub fn default_max_concurrent_per_node() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub event_type: String,
    pub repo_name: String,
    pub clone_url: String,
    pub commit_sha: String,
    pub ref_name: String,
    pub sender: String,
    pub queued_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details")]
pub enum JobStatus {
    Queued,
    Running { started_at: DateTime<Utc> },
    Completed { duration_secs: u64, exit_code: i32 },
    Failed { error: String },
    Skipped { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmoduleMetrics {
    pub name: String,
    #[serde(default)]
    pub path: String,
    pub quality_score: u32,
    pub letter_grade: String,
    #[serde(default)]
    pub tests_passed: u32,
    #[serde(default)]
    pub tests_total: u32,
    #[serde(default)]
    pub coverage_pct: Option<f64>,
    #[serde(default)]
    pub benchmark_summary: Option<String>,
    #[serde(default)]
    pub clippy_warnings: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobMetrics {
    pub quality_score: u32,
    pub letter_grade: String,
    #[serde(default)]
    pub coverage_pct: Option<f64>,
    #[serde(default)]
    pub benchmark_summary: Option<String>,
    #[serde(default)]
    pub tests_passed: u32,
    #[serde(default)]
    pub tests_total: u32,
    #[serde(default)]
    pub advisories_count: u32,
    #[serde(default)]
    pub clippy_warnings: u32,
    #[serde(default)]
    pub submodules: BTreeMap<String, SubmoduleMetrics>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details")]
pub enum StageStatus {
    Pending,
    Assigned {
        node_id: String,
        started_at: DateTime<Utc>,
    },
    Running {
        node_id: String,
        started_at: DateTime<Utc>,
        #[serde(default)]
        pid: Option<u32>,
    },
    Completed {
        duration_secs: u64,
        exit_code: i32,
    },
    Failed {
        error: String,
    },
    Skipped {
        reason: String,
    },
}

/// Tier 3 (`PipelineTaskItem`): Typed `ToolAdapter` invocation inside a pipeline stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineTaskItem {
    pub task_id: String,
    pub tool_name: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub sandbox_profile: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub duration_secs: Option<f64>,
}

/// `MuIsolation` Sandbox Backend Kind (`KernelSandbox` vs `HypervisorSandbox`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SandboxBackendType {
    #[default]
    #[serde(alias = "Kernel", alias = "kernel")]
    KernelSandbox,
    #[serde(
        alias = "Hypervisor",
        alias = "hypervisor",
        alias = "Kvm",
        alias = "kvm"
    )]
    HypervisorSandbox,
}

impl SandboxBackendType {
    pub fn label(&self) -> &'static str {
        match self {
            SandboxBackendType::KernelSandbox => "KernelSandbox (Landlock + cgroupv2 + seccomp)",
            SandboxBackendType::HypervisorSandbox => "HypervisorSandbox (/dev/kvm Guest VM)",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            SandboxBackendType::KernelSandbox => "KernelSandbox",
            SandboxBackendType::HypervisorSandbox => "HypervisorSandbox",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            SandboxBackendType::KernelSandbox => "sandbox-badge sandbox-kernel",
            SandboxBackendType::HypervisorSandbox => "sandbox-badge sandbox-kvm",
        }
    }
}

/// Active `MuIsolation` sandbox telemetry for a pipeline stage (`#20`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxTelemetryRecord {
    pub stage_id: String,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub backend: SandboxBackendType,
    pub profile_name: String,
    pub memlock_limit_kb: u64,
    #[serde(default)]
    pub set_ai_thread_active: bool,
    #[serde(default)]
    pub max_pids: u32,
    #[serde(default)]
    pub network_egress_cidrs: Vec<String>,
}

/// Fail-closed `SecurityGrantDenied` privilege violation alert record (`#20`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityGrantViolationRecord {
    pub stage_id: String,
    pub violated_layer: String,
    pub field: String,
    pub requested_value: String,
    pub ceiling_value: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageRecord {
    pub id: String,
    pub name: String,
    pub tags: Vec<String>,
    pub threads: u32,
    pub toolchain: Option<String>,
    pub depends_on: Vec<String>,
    pub status: StageStatus,
    #[serde(default)]
    pub assigned_node: Option<String>,
    #[serde(default)]
    pub exclusive: bool,
    #[serde(default = "default_max_concurrent_per_node")]
    pub max_concurrent_per_node: u32,
    #[serde(default)]
    pub tasks: Vec<PipelineTaskItem>,
    #[serde(default)]
    pub sandbox_telemetry: Option<SandboxTelemetryRecord>,
}

impl StageRecord {
    pub fn execution_node(&self) -> Option<&str> {
        if let Some(ref node) = self.assigned_node {
            return Some(node.as_str());
        }
        match &self.status {
            StageStatus::Assigned { node_id, .. } | StageStatus::Running { node_id, .. } => {
                Some(node_id.as_str())
            }
            _ => None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            StageStatus::Assigned { .. } | StageStatus::Running { .. }
        )
    }

    pub fn node_display(&self) -> (&'static str, &'static str) {
        match self.execution_node() {
            Some("control-node") => ("🖥️ control-node (N100)", "node-pill node-n100"),
            Some("storage-node") => ("⚡ storage-node (Xeon)", "node-pill node-xeon"),
            Some("worker-node") | Some("worker-riscv64") => {
                ("🔬 worker-node (K3)", "node-pill node-k3")
            }
            Some(_) => ("⚙️ Remote Worker", "node-pill node-other"),
            None => ("⏳ Awaiting Node", "node-pill node-pending"),
        }
    }

    pub fn compact_node_display(&self) -> (&'static str, &'static str) {
        match self.execution_node() {
            Some("control-node") => ("🖥️ control-node", "node-pill node-n100"),
            Some("storage-node") => ("⚡ storage-node", "node-pill node-xeon"),
            Some("worker-node") | Some("worker-riscv64") => ("🔬 worker-node", "node-pill node-k3"),
            Some(_) => ("⚙️ remote", "node-pill node-other"),
            None => ("⏳ awaiting", "node-pill node-pending"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ClusterNodeView {
    pub node_id: String,
    pub arch: String,
    pub tags: Vec<String>,
    pub max_slots: u32,
    pub active_slots: u32,
    #[serde(default)]
    pub max_concurrent_per_node: Option<u32>,
    #[serde(default)]
    pub exclusive_locked: bool,
    #[serde(default)]
    pub exclusive_stage_id: Option<String>,
    pub ram_total_mb: u64,
    pub ram_used_mb: u64,
    pub cpu_load_pct: f32,
    pub btrfs_enabled: bool,
    pub toolchains: Vec<String>,
    pub last_seen_secs_ago: u64,
    pub is_online: bool,
}

impl ClusterNodeView {
    /// Returns the effective `max_concurrent_per_node` ceiling (defaults to `max_slots.max(1)`).
    pub fn effective_max_concurrent(&self) -> u32 {
        self.max_concurrent_per_node
            .unwrap_or_else(|| self.max_slots.max(1))
    }

    /// Resolves whether the node currently holds an exclusive stage lock, either from
    /// the controller's `exclusive_locked` field or inferred from an active `exclusive: true` stage.
    pub fn resolve_exclusive_lock(&self, jobs: &[JobRecord]) -> (bool, Option<String>, u32) {
        let max_conc = self.effective_max_concurrent();
        if self.exclusive_locked {
            let inferred_stage = self.exclusive_stage_id.clone().or_else(|| {
                for job in jobs {
                    for stage in &job.stages {
                        if stage.execution_node() == Some(self.node_id.as_str())
                            && stage.is_active()
                        {
                            return Some(stage.id.clone());
                        }
                    }
                }
                None
            });
            return (true, inferred_stage, max_conc);
        }

        for job in jobs {
            for stage in &job.stages {
                if stage.execution_node() == Some(self.node_id.as_str())
                    && stage.is_active()
                    && stage.exclusive
                {
                    return (true, Some(stage.id.clone()), max_conc);
                }
            }
        }

        (false, None, max_conc)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobRecord {
    pub job: Job,
    pub status: JobStatus,
    pub log_file: String,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub metrics: Option<JobMetrics>,
    #[serde(default)]
    pub queue_position: Option<usize>,
    #[serde(default)]
    pub elapsed_secs: Option<u64>,
    #[serde(default)]
    pub stages: Vec<StageRecord>,
    #[serde(default)]
    pub sandbox_telemetry: Vec<SandboxTelemetryRecord>,
    #[serde(default)]
    pub security_violations: Vec<SecurityGrantViolationRecord>,
    #[serde(default)]
    pub traceparent: Option<String>,
    #[serde(default)]
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueItemView {
    pub position: usize,
    pub wait_secs: u64,
    #[serde(default)]
    pub elapsed_secs: Option<u64>,
    pub record: JobRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct QueueStats {
    pub total_jobs: usize,
    pub running_jobs: usize,
    pub queued_jobs: usize,
    pub completed_jobs: usize,
    pub failed_jobs: usize,
    pub skipped_jobs: usize,
    #[serde(default)]
    pub superseded_jobs: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub status: String,
    pub concurrency: usize,
    pub queue_depth: usize,
    #[serde(default)]
    pub active_job: Option<QueueItemView>,
    #[serde(default)]
    pub active_jobs: Vec<QueueItemView>,
    #[serde(default)]
    pub queued_jobs: Vec<QueueItemView>,
    #[serde(default)]
    pub stats: QueueStats,
    #[serde(default)]
    pub recent_jobs: Vec<JobRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobsListResponse {
    pub job_ids: Vec<String>,
    pub total: usize,
    #[serde(default)]
    pub queue_depth: usize,
    #[serde(default)]
    pub active_job_id: Option<String>,
    #[serde(default)]
    pub stats: Option<QueueStats>,
    pub jobs: Vec<JobRecord>,
}

impl JobStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, JobStatus::Queued | JobStatus::Running { .. })
    }

    pub fn category(&self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running { .. } => "running",
            JobStatus::Completed { exit_code: 0, .. } => "passing",
            JobStatus::Completed { .. } | JobStatus::Failed { .. } => "failing",
            JobStatus::Skipped { .. } => "skipped",
        }
    }

    pub fn badge_label(&self) -> &'static str {
        match self {
            JobStatus::Queued => "⏱ QUEUED",
            JobStatus::Running { .. } => "⏳ RUNNING",
            JobStatus::Completed { exit_code: 0, .. } => "✓ PASSING",
            JobStatus::Completed { .. } | JobStatus::Failed { .. } => "✗ FAILING",
            JobStatus::Skipped { reason } => {
                if reason.contains("Superseded") {
                    "⊘ SUPERSEDED"
                } else {
                    "⊘ SKIPPED"
                }
            }
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            JobStatus::Queued => "status-pill status-queued",
            JobStatus::Running { .. } => "status-pill status-running",
            JobStatus::Completed { exit_code: 0, .. } => "status-pill status-passing",
            JobStatus::Completed { .. } | JobStatus::Failed { .. } => "status-pill status-failing",
            JobStatus::Skipped { .. } => "status-pill status-skipped",
        }
    }

    pub fn duration_label(&self, now: DateTime<Utc>) -> String {
        match self {
            JobStatus::Completed { duration_secs, .. } => format!("{}s", duration_secs),
            JobStatus::Running { started_at } => {
                let d = (now - *started_at).num_seconds().max(0);
                format!("{}s (running)", d)
            }
            JobStatus::Queued => "queued".to_string(),
            JobStatus::Failed { .. } => "failed".to_string(),
            JobStatus::Skipped { reason } => {
                if reason.contains("Superseded") {
                    "coalesced".to_string()
                } else {
                    "skipped".to_string()
                }
            }
        }
    }
}

pub fn short_sha(sha: &str) -> &str {
    if sha.len() >= 8 {
        &sha[..8]
    } else if !sha.is_empty() {
        sha
    } else {
        "n/a"
    }
}

pub fn short_branch(ref_name: &str) -> &str {
    ref_name
        .strip_prefix("refs/heads/")
        .or_else(|| ref_name.strip_prefix("refs/tags/"))
        .unwrap_or(ref_name)
}

pub fn score_pill_class(score: u32) -> &'static str {
    match score {
        90..=100 => "score-pill score-high",
        75..=89 => "score-pill score-med",
        _ => "score-pill score-low",
    }
}

#[allow(dead_code)]
pub fn card_svg_url(api_base: &str, repo_name: &str, ref_name: &str) -> String {
    let base = api_base.trim_end_matches('/');
    let branch = short_branch(ref_name);
    if branch.is_empty() {
        format!("{}/cards/{}/card.svg", base, repo_name)
    } else if branch.contains('/') {
        format!("{}/cards/{}/card.svg?branch={}", base, repo_name, branch)
    } else {
        format!("{}/cards/{}/{}/card.svg", base, repo_name, branch)
    }
}

pub fn job_card_svg_url(api_base: &str, job_id: &str, updated_ts: i64) -> String {
    let base = api_base.trim_end_matches('/');
    format!("{}/api/jobs/{}/card.svg?t={}", base, job_id, updated_ts)
}

pub fn job_submodule_card_svg_url(
    api_base: &str,
    job_id: &str,
    submodule_name: &str,
    updated_ts: i64,
) -> String {
    let base = api_base.trim_end_matches('/');
    let sub_short = submodule_name
        .rsplit_once('/')
        .map(|(_, s)| s)
        .unwrap_or(submodule_name);
    format!(
        "{}/api/jobs/{}/submodules/{}/card.svg?t={}",
        base, job_id, sub_short, updated_ts
    )
}

#[allow(dead_code)]
pub fn submodule_card_svg_url(parent_repo: &str, submodule_name: &str, api_base: &str) -> String {
    let base = api_base.trim_end_matches('/');
    let owner = parent_repo
        .split_once('/')
        .map(|(o, _)| o)
        .unwrap_or("LMA1980");
    let sub_short = submodule_name
        .rsplit_once('/')
        .map(|(_, s)| s)
        .unwrap_or(submodule_name);
    format!("{}/cards/{}/{}/card.svg", base, owner, sub_short)
}

pub fn extract_repositories(jobs: &[JobRecord]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for rec in jobs {
        set.insert(rec.job.repo_name.clone());
    }
    set.into_iter().collect()
}

pub fn filter_jobs(
    jobs: &[JobRecord],
    repo_filter: &str,
    status_filter: &str,
    search_query: &str,
) -> Vec<JobRecord> {
    let repo_f = repo_filter.trim().to_ascii_lowercase();
    let status_f = status_filter.trim().to_ascii_lowercase();
    let query = search_query.trim().to_ascii_lowercase();

    jobs.iter()
        .filter(|rec| {
            if !repo_f.is_empty() && repo_f != "all" {
                let matches_parent = rec.job.repo_name.to_ascii_lowercase() == repo_f
                    || rec.job.repo_name.to_ascii_lowercase().contains(&repo_f);
                let matches_sub = rec
                    .metrics
                    .as_ref()
                    .map(|m| {
                        m.submodules
                            .keys()
                            .any(|k| k.to_ascii_lowercase().contains(&repo_f))
                    })
                    .unwrap_or(false);
                if !matches_parent && !matches_sub {
                    return false;
                }
            }

            if !status_f.is_empty() && status_f != "all" && rec.status.category() != status_f {
                return false;
            }

            if !query.is_empty() {
                let in_id = rec.job.id.to_ascii_lowercase().contains(&query);
                let in_repo = rec.job.repo_name.to_ascii_lowercase().contains(&query);
                let in_sha = rec.job.commit_sha.to_ascii_lowercase().contains(&query);
                let in_ref = rec.job.ref_name.to_ascii_lowercase().contains(&query);
                let in_sender = rec.job.sender.to_ascii_lowercase().contains(&query);
                if !(in_id || in_repo || in_sha || in_ref || in_sender) {
                    return false;
                }
            }

            true
        })
        .cloned()
        .collect()
}

/// Determines if a scrollable container is currently positioned at or near the tail (bottom).
/// Returns true if the distance from the bottom is within `threshold` pixels, or if the content
/// fits within the container without overflowing.
pub fn is_scrolled_to_bottom(
    scroll_height: i32,
    client_height: i32,
    scroll_top: i32,
    threshold: i32,
) -> bool {
    let max_scroll = scroll_height - client_height;
    if max_scroll <= 0 {
        return true;
    }
    let distance_to_bottom = max_scroll - scroll_top;
    distance_to_bottom <= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(id: &str, repo: &str, sha: &str, status: JobStatus) -> JobRecord {
        JobRecord {
            job: Job {
                id: id.to_string(),
                event_type: "push".to_string(),
                repo_name: repo.to_string(),
                clone_url: format!("https://github.com/{}.git", repo),
                commit_sha: sha.to_string(),
                ref_name: "refs/heads/main".to_string(),
                sender: "LMA1980".to_string(),
                queued_at: Utc::now(),
            },
            status,
            log_file: format!("logs/{}/{}.log", repo, id),
            updated_at: Utc::now(),
            metrics: None,
            queue_position: None,
            elapsed_secs: None,
            stages: Vec::new(),
            sandbox_telemetry: Vec::new(),
            security_violations: Vec::new(),
            traceparent: None,
            trace_id: None,
        }
    }

    #[test]
    fn test_filter_jobs_by_repo_status_and_search() {
        let jobs = vec![
            sample_record(
                "job-1",
                "LMA1980/ActionRunner-rs",
                "1111aaaa2222bbbb",
                JobStatus::Completed {
                    duration_secs: 12,
                    exit_code: 0,
                },
            ),
            sample_record(
                "job-2",
                "LMA1980/model-runner",
                "3333cccc4444dddd",
                JobStatus::Queued,
            ),
            sample_record(
                "job-3",
                "LMA1980/model-runner",
                "5555eeee6666ffff",
                JobStatus::Skipped {
                    reason: "Superseded in queue by newer commit 3333cccc".to_string(),
                },
            ),
        ];

        let all = filter_jobs(&jobs, "all", "all", "");
        assert_eq!(all.len(), 3);

        let model_runner_only = filter_jobs(&jobs, "LMA1980/model-runner", "all", "");
        assert_eq!(model_runner_only.len(), 2);

        let queued_only = filter_jobs(&jobs, "all", "queued", "");
        assert_eq!(queued_only.len(), 1);
        assert_eq!(queued_only[0].job.id, "job-2");

        let sha_search = filter_jobs(&jobs, "all", "all", "1111aaaa");
        assert_eq!(sha_search.len(), 1);
        assert_eq!(sha_search[0].job.id, "job-1");
    }

    #[test]
    fn test_helpers_and_urls() {
        assert_eq!(short_sha("1234567890abcdef"), "12345678");
        assert_eq!(short_branch("refs/heads/main"), "main");
        assert_eq!(short_branch("refs/tags/v0.1.0"), "v0.1.0");
        assert_eq!(
            card_svg_url("", "LMA1980/model-runner", "refs/heads/main"),
            "/cards/LMA1980/model-runner/main/card.svg"
        );
        assert_eq!(
            card_svg_url("", "LMA1980/mu-ci-web", "feat/first-view-host-attribution"),
            "/cards/LMA1980/mu-ci-web/card.svg?branch=feat/first-view-host-attribution"
        );
        assert_eq!(
            job_card_svg_url("", "job-1792858126-9ca1c51a-2", 1792858557),
            "/api/jobs/job-1792858126-9ca1c51a-2/card.svg?t=1792858557"
        );
        assert_eq!(
            job_submodule_card_svg_url("", "job-1792858126-9ca1c51a-2", "code/core", 1792858557),
            "/api/jobs/job-1792858126-9ca1c51a-2/submodules/core/card.svg?t=1792858557"
        );
        assert_eq!(
            submodule_card_svg_url("LMA1980/model-runner", "k3-core", ""),
            "/cards/LMA1980/k3-core/card.svg"
        );
    }

    #[test]
    fn test_stage_record_node_displays() {
        let mut stage = StageRecord {
            id: "fmt".to_string(),
            name: "fmt-check".to_string(),
            tags: vec!["fmt".to_string()],
            threads: 2,
            toolchain: None,
            depends_on: vec![],
            status: StageStatus::Pending,
            assigned_node: None,
            exclusive: false,
            max_concurrent_per_node: 1,
            tasks: vec![],
            sandbox_telemetry: None,
        };

        assert_eq!(stage.compact_node_display().0, "⏳ awaiting");

        stage.assigned_node = Some("control-node".to_string());
        assert_eq!(stage.compact_node_display().0, "🖥️ control-node");
        assert_eq!(stage.compact_node_display().1, "node-pill node-n100");

        stage.assigned_node = Some("storage-node".to_string());
        assert_eq!(stage.compact_node_display().0, "⚡ storage-node");
        assert_eq!(stage.compact_node_display().1, "node-pill node-xeon");

        stage.assigned_node = Some("worker-node".to_string());
        assert_eq!(stage.compact_node_display().0, "🔬 worker-node");
        assert_eq!(stage.compact_node_display().1, "node-pill node-k3");
    }

    #[test]
    fn test_is_scrolled_to_bottom() {
        // At exact bottom: (2000 - 650) - 1350 = 0 <= 40
        assert!(is_scrolled_to_bottom(2000, 650, 1350, 40));
        // Within 40px threshold (20px from bottom): distance = 20 <= 40
        assert!(is_scrolled_to_bottom(2000, 650, 1330, 40));
        // Exactly at 40px threshold: distance = 40 <= 40
        assert!(is_scrolled_to_bottom(2000, 650, 1310, 40));
        // 50px away from bottom (scrolled up): distance = 50 > 40
        assert!(!is_scrolled_to_bottom(2000, 650, 1300, 40));
        // Scrolled all the way to top: distance = 1350 > 40
        assert!(!is_scrolled_to_bottom(2000, 650, 0, 40));
        // macOS / iOS rubber-banding overshoot: distance = -10 <= 40
        assert!(is_scrolled_to_bottom(2000, 650, 1360, 40));
        // Content shorter than container (fits entirely on screen): distance = -250 <= 40
        assert!(is_scrolled_to_bottom(400, 650, 0, 40));
    }

    #[test]
    fn test_queue_snapshot_multiple_active_and_precomputed_queued_jobs() {
        let mut precomputed_record =
            sample_record("job-3", "LMA1980/k3-engine", "3333cccc", JobStatus::Queued);
        precomputed_record.stages = vec![
            StageRecord {
                id: "fmt-check".to_string(),
                name: "fmt-check".to_string(),
                tags: vec!["fmt".to_string()],
                threads: 2,
                toolchain: None,
                depends_on: vec![],
                status: StageStatus::Completed {
                    exit_code: 0,
                    duration_secs: 3,
                },
                assigned_node: Some("node-a".to_string()),
                exclusive: false,
                max_concurrent_per_node: 1,
                tasks: vec![],
                sandbox_telemetry: None,
            },
            StageRecord {
                id: "test-check".to_string(),
                name: "test-check".to_string(),
                tags: vec!["k3".to_string()],
                threads: 6,
                toolchain: Some("llvm-24".to_string()),
                depends_on: vec!["fmt-check".to_string()],
                status: StageStatus::Pending,
                assigned_node: None,
                exclusive: true,
                max_concurrent_per_node: 1,
                tasks: vec![],
                sandbox_telemetry: None,
            },
        ];

        let orig = QueueSnapshot {
            status: "ok".to_string(),
            concurrency: 3,
            queue_depth: 1,
            active_job: Some(QueueItemView {
                position: 0,
                wait_secs: 2,
                elapsed_secs: Some(15),
                record: sample_record(
                    "job-1",
                    "LMA1980/model-runner",
                    "1111aaaa",
                    JobStatus::Running {
                        started_at: Utc::now(),
                    },
                ),
            }),
            active_jobs: vec![
                QueueItemView {
                    position: 0,
                    wait_secs: 2,
                    elapsed_secs: Some(15),
                    record: sample_record(
                        "job-1",
                        "LMA1980/model-runner",
                        "1111aaaa",
                        JobStatus::Running {
                            started_at: Utc::now(),
                        },
                    ),
                },
                QueueItemView {
                    position: 0,
                    wait_secs: 5,
                    elapsed_secs: Some(8),
                    record: sample_record(
                        "job-2",
                        "LMA1980/k3-core",
                        "2222bbbb",
                        JobStatus::Running {
                            started_at: Utc::now(),
                        },
                    ),
                },
            ],
            queued_jobs: vec![QueueItemView {
                position: 1,
                wait_secs: 42,
                elapsed_secs: None,
                record: precomputed_record,
            }],
            stats: QueueStats {
                total_jobs: 3,
                running_jobs: 2,
                queued_jobs: 1,
                ..Default::default()
            },
            recent_jobs: vec![],
        };

        let payload = serde_json::to_string(&orig).unwrap();
        let snap: QueueSnapshot = serde_json::from_str(&payload).unwrap();
        assert_eq!(snap.active_jobs.len(), 2);
        assert_eq!(snap.active_jobs[0].record.job.id, "job-1");
        assert_eq!(snap.active_jobs[1].record.job.id, "job-2");
        assert_eq!(snap.queued_jobs.len(), 1);
        let precomputed = snap.queued_jobs[0]
            .record
            .stages
            .iter()
            .filter(|st| matches!(st.status, StageStatus::Completed { .. }))
            .count();
        assert_eq!(precomputed, 1);
    }

    #[test]
    fn test_job_status_is_active() {
        assert!(JobStatus::Queued.is_active());
        assert!(JobStatus::Running {
            started_at: Utc::now()
        }
        .is_active());
        assert!(!JobStatus::Completed {
            duration_secs: 5,
            exit_code: 0
        }
        .is_active());
        assert!(!JobStatus::Failed {
            error: "boom".into()
        }
        .is_active());
        assert!(!JobStatus::Skipped {
            reason: "Superseded".into()
        }
        .is_active());
    }
}
