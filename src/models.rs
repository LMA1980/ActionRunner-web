use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
    Assigned { node_id: String, started_at: DateTime<Utc> },
    Running { node_id: String, started_at: DateTime<Utc> },
    Completed { duration_secs: u64, exit_code: i32 },
    Failed { error: String },
    Skipped { reason: String },
}

impl StageStatus {
    pub fn badge_label(&self) -> &'static str {
        match self {
            StageStatus::Pending => "⏱ PENDING",
            StageStatus::Assigned { .. } => "⏳ ASSIGNED",
            StageStatus::Running { .. } => "⚡ RUNNING",
            StageStatus::Completed { exit_code: 0, .. } => "✓ PASSED",
            StageStatus::Completed { .. } | StageStatus::Failed { .. } => "✗ FAILED",
            StageStatus::Skipped { .. } => "⊘ SKIPPED",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            StageStatus::Pending => "status-pill status-queued",
            StageStatus::Assigned { .. } => "status-pill status-running",
            StageStatus::Running { .. } => "status-pill status-running",
            StageStatus::Completed { exit_code: 0, .. } => "status-pill status-passing",
            StageStatus::Completed { .. } | StageStatus::Failed { .. } => "status-pill status-failing",
            StageStatus::Skipped { .. } => "status-pill status-skipped",
        }
    }
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ClusterNodeView {
    pub node_id: String,
    pub arch: String,
    pub tags: Vec<String>,
    pub max_slots: u32,
    pub active_slots: u32,
    pub ram_total_mb: u64,
    pub ram_used_mb: u64,
    pub cpu_load_pct: f32,
    pub btrfs_enabled: bool,
    pub toolchains: Vec<String>,
    pub last_seen_secs_ago: u64,
    pub is_online: bool,
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

pub fn card_svg_url(api_base: &str, repo_name: &str, ref_name: &str) -> String {
    let base = api_base.trim_end_matches('/');
    let branch = short_branch(ref_name);
    if branch.is_empty() || branch.contains('/') {
        format!("{}/cards/{}/card.svg", base, repo_name)
    } else {
        format!("{}/cards/{}/{}/card.svg", base, repo_name, branch)
    }
}

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
            submodule_card_svg_url("LMA1980/model-runner", "k3-core", ""),
            "/cards/LMA1980/k3-core/card.svg"
        );
    }
}
