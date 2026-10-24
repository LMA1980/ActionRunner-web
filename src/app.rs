use chrono::Utc;
use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::log_parser::{parse_job_log, StepStatus};
use crate::models::{
    card_svg_url, extract_repositories, filter_jobs, score_pill_class, short_branch, short_sha,
    submodule_card_svg_url, ClusterNodeView, JobRecord, JobStatus, JobsListResponse, QueueSnapshot,
};

fn default_api_base() -> String {
    if let Some(win) = web_sys::window() {
        let current_origin = win.location().origin().ok();
        if let Ok(search) = win.location().search() {
            for pair in search.trim_start_matches('?').split('&') {
                if let Some(("api", val)) = pair.split_once('=') {
                    if !val.is_empty() {
                        let trimmed = val.trim_end_matches('/');
                        // Security: Only accept same-origin relative paths, localhost for dev, or matching origin
                        let is_safe = trimmed.starts_with('/')
                            || trimmed.starts_with("http://127.0.0.1")
                            || trimmed.starts_with("http://localhost")
                            || current_origin
                                .as_deref()
                                .map(|o| trimmed.starts_with(o))
                                .unwrap_or(false);

                        if is_safe {
                            return trimmed.to_string();
                        }
                    }
                }
            }
        }
    }
    String::new()
}

fn initial_selected_job_id() -> Option<String> {
    let win = web_sys::window()?;
    if let Ok(hash) = win.location().hash() {
        if let Some(id) = hash.strip_prefix("#/jobs/") {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    if let Ok(pathname) = win.location().pathname() {
        if let Some(id) = pathname.strip_prefix("/jobs/") {
            let clean = id.trim_matches('/');
            if !clean.is_empty() && !clean.contains('/') {
                return Some(clean.to_string());
            }
        }
    }
    if let Ok(search) = win.location().search() {
        for pair in search.trim_start_matches('?').split('&') {
            if let Some(("job", val)) = pair.split_once('=') {
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

fn set_url_job_hash(job_id: Option<&str>) {
    if let Some(win) = web_sys::window() {
        let target_hash = match job_id {
            Some(id) => format!("#/jobs/{}", id),
            None => "#/queue".to_string(),
        };
        let _ = win.location().set_hash(&target_hash);
    }
}

async fn fetch_queue_snapshot(api_base: &str) -> Result<QueueSnapshot, String> {
    let primary_url = format!("{}/api/queue", api_base);
    match Request::get(&primary_url).send().await {
        Ok(resp) if resp.ok() => resp
            .json::<QueueSnapshot>()
            .await
            .map_err(|e| format!("Failed to parse /api/queue JSON: {}", e)),
        _ => {
            let fallback_url = format!("{}/queue", api_base);
            let resp = Request::get(&fallback_url)
                .send()
                .await
                .map_err(|e| format!("Network error fetching /queue: {}", e))?;
            if !resp.ok() {
                return Err(format!("HTTP {} from /queue", resp.status()));
            }
            resp.json::<QueueSnapshot>()
                .await
                .map_err(|e| format!("Failed to parse /queue JSON: {}", e))
        }
    }
}

async fn fetch_jobs_list(api_base: &str) -> Result<JobsListResponse, String> {
    let primary_url = format!("{}/api/jobs", api_base);
    match Request::get(&primary_url).send().await {
        Ok(resp) if resp.ok() => resp
            .json::<JobsListResponse>()
            .await
            .map_err(|e| format!("Failed to parse /api/jobs JSON: {}", e)),
        _ => {
            let fallback_url = format!("{}/jobs", api_base);
            let resp = Request::get(&fallback_url)
                .send()
                .await
                .map_err(|e| format!("Network error fetching /jobs: {}", e))?;
            if !resp.ok() {
                return Err(format!("HTTP {} from /jobs", resp.status()));
            }
            resp.json::<JobsListResponse>()
                .await
                .map_err(|e| format!("Failed to parse /jobs JSON: {}", e))
        }
    }
}

async fn fetch_single_job(api_base: &str, job_id: &str) -> Result<JobRecord, String> {
    let primary_url = format!("{}/api/jobs/{}", api_base, job_id);
    match Request::get(&primary_url).send().await {
        Ok(resp) if resp.ok() => resp
            .json::<JobRecord>()
            .await
            .map_err(|e| format!("Failed to parse job JSON: {}", e)),
        _ => {
            let fallback_url = format!("{}/jobs/{}", api_base, job_id);
            let resp = Request::get(&fallback_url)
                .send()
                .await
                .map_err(|e| format!("Network error fetching job {}: {}", job_id, e))?;
            if !resp.ok() {
                return Err(format!("Job {} returned HTTP {}", job_id, resp.status()));
            }
            resp.json::<JobRecord>()
                .await
                .map_err(|e| format!("Failed to parse job JSON: {}", e))
        }
    }
}

async fn fetch_job_log_text(api_base: &str, job_id: &str) -> Result<String, String> {
    let primary_url = format!("{}/api/jobs/{}/log", api_base, job_id);
    match Request::get(&primary_url).send().await {
        Ok(resp) if resp.ok() => resp
            .text()
            .await
            .map_err(|e| format!("Failed reading log text: {}", e)),
        _ => {
            let fallback_url = format!("{}/jobs/{}/log", api_base, job_id);
            let resp = Request::get(&fallback_url)
                .send()
                .await
                .map_err(|e| format!("Network error fetching log: {}", e))?;
            if !resp.ok() {
                return Err(format!("Log not available yet (HTTP {})", resp.status()));
            }
            resp.text()
                .await
                .map_err(|e| format!("Failed reading log text: {}", e))
        }
    }
}

async fn fetch_cluster_nodes(api_base: &str) -> Result<Vec<ClusterNodeView>, String> {
    let url = format!("{}/api/cluster/nodes", api_base);
    match Request::get(&url).send().await {
        Ok(resp) if resp.ok() => resp
            .json::<Vec<ClusterNodeView>>()
            .await
            .map_err(|e| format!("Failed to parse cluster nodes JSON: {}", e)),
        _ => Ok(vec![]),
    }
}

#[component]
pub fn App() -> impl IntoView {
    let api_base = RwSignal::new(default_api_base());
    let queue_snap = RwSignal::new(None::<QueueSnapshot>);
    let jobs_list = RwSignal::new(Vec::<JobRecord>::new());
    let cluster_nodes = RwSignal::new(Vec::<ClusterNodeView>::new());
    let error_banner = RwSignal::new(None::<String>);
    let last_updated = RwSignal::new(String::from("Loading..."));
    let auto_refresh = RwSignal::new(true);

    // Filter signals for Queue & Job Explorer
    let repo_filter = RwSignal::new(String::from("all"));
    let status_filter = RwSignal::new(String::from("all"));
    let search_query = RwSignal::new(String::new());

    // Drill-down state
    let selected_job_id = RwSignal::new(initial_selected_job_id());
    let selected_job_record = RwSignal::new(None::<JobRecord>);
    let selected_job_log = RwSignal::new(String::new());
    let log_loading = RwSignal::new(false);
    let log_view_mode = RwSignal::new(String::from("steps")); // "steps" or "raw"
    let log_search = RwSignal::new(String::new());

    let refresh_all = move || {
        let base = api_base.get_untracked();
        let current_job_id = selected_job_id.get_untracked();
        spawn_local(async move {
            match (
                fetch_queue_snapshot(&base).await,
                fetch_jobs_list(&base).await,
            ) {
                (Ok(snap), Ok(list)) => {
                    queue_snap.set(Some(snap));
                    jobs_list.set(list.jobs);
                    error_banner.set(None);
                    last_updated.set(Utc::now().format("%H:%M:%S UTC").to_string());
                }
                (Err(e), _) | (_, Err(e)) => {
                    error_banner.set(Some(e));
                }
            }

            if let Ok(nodes) = fetch_cluster_nodes(&base).await {
                cluster_nodes.set(nodes);
            }

            if let Some(jid) = current_job_id {
                if let Ok(rec) = fetch_single_job(&base, &jid).await {
                    selected_job_record.set(Some(rec));
                }
                if let Ok(log_txt) = fetch_job_log_text(&base, &jid).await {
                    selected_job_log.set(log_txt);
                }
            }
        });
    };

    let open_job_drilldown = move |job_id: String| {
        set_url_job_hash(Some(&job_id));
        selected_job_id.set(Some(job_id.clone()));
        log_loading.set(true);
        let base = api_base.get_untracked();
        // Pre-populate from in-memory list if available
        if let Some(existing) = jobs_list
            .get_untracked()
            .into_iter()
            .find(|r| r.job.id == job_id)
        {
            selected_job_record.set(Some(existing));
        }
        spawn_local(async move {
            if let Ok(rec) = fetch_single_job(&base, &job_id).await {
                selected_job_record.set(Some(rec));
            }
            match fetch_job_log_text(&base, &job_id).await {
                Ok(txt) => selected_job_log.set(txt),
                Err(e) => selected_job_log.set(format!("[ActionRunner-web] {}", e)),
            }
            log_loading.set(false);
        });
    };

    let close_drilldown = move |_| {
        set_url_job_hash(None);
        selected_job_id.set(None);
        selected_job_record.set(None);
        selected_job_log.set(String::new());
    };

    // Initial load + 3s auto-refresh timer on wasm32
    Effect::new(move |_| {
        refresh_all();
        if let Some(init_id) = selected_job_id.get_untracked() {
            open_job_drilldown(init_id);
        }
    });

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;
        if let Some(win) = web_sys::window() {
            let cb = Closure::wrap(Box::new(move || {
                if auto_refresh.get_untracked() {
                    refresh_all();
                }
            }) as Box<dyn FnMut()>);
            let _ = win.set_interval_with_callback_and_timeout_and_arguments_0(
                cb.as_ref().unchecked_ref(),
                3000,
            );
            cb.forget();
        }
    }

    let _runner_busy = move || {
        queue_snap
            .get()
            .map(|s| s.status == "busy")
            .unwrap_or(false)
    };

    let repositories = move || extract_repositories(&jobs_list.get());
    let filtered_jobs = move || {
        filter_jobs(
            &jobs_list.get(),
            &repo_filter.get(),
            &status_filter.get(),
            &search_query.get(),
        )
    };

    view! {
        <div class="app-shell">
            // Top Navbar
            <header class="topbar">
                <div class="topbar-brand" on:click=close_drilldown style="cursor: pointer;">
                    <span class="brand-logo">"⚡"</span>
                    <div>
                        <div class="brand-title">"ActionRunner Distributed CI Fabric"</div>
                        <div class="brand-subtitle">"Tag-Orchestrated Multi-Node Execution • Zero-Copy Arrow Streaming"</div>
                    </div>
                </div>

                <div class="topbar-controls">
                    {move || {
                        let nodes = cluster_nodes.get();
                        let active: u32 = nodes.iter().map(|n| n.active_slots).sum();
                        let total: u32 = nodes.iter().map(|n| n.max_slots).sum();
                        let is_busy = active > 0;
                        let pill_cls = if is_busy { "runner-pill runner-busy" } else { "runner-pill runner-idle" };
                        let pill_txt = if is_busy {
                            format!("⏳ FABRIC BUSY ({}/{})", active, total.max(1))
                        } else {
                            format!("✓ FABRIC IDLE (0/{})", total.max(1))
                        };
                        view! {
                            <span class=pill_cls>{pill_txt}</span>
                        }
                    }}

                    <span class="updated-label">
                        "Synced: " {move || last_updated.get()}
                    </span>

                    <button
                        class=move || if auto_refresh.get() { "btn btn-active" } else { "btn" }
                        on:click=move |_| auto_refresh.update(|v| *v = !*v)
                    >
                        <span class="btn-text-full">
                            {move || if auto_refresh.get() { "⟳ Auto-Refresh: ON (3s)" } else { "⏸ Auto-Refresh: PAUSED" }}
                        </span>
                        <span class="btn-text-short">
                            {move || if auto_refresh.get() { "⟳ Auto: ON" } else { "⏸ Auto: OFF" }}
                        </span>
                    </button>

                    <button class="btn btn-primary" on:click=move |_| refresh_all()>
                        "Refresh Now"
                    </button>
                </div>
            </header>

            // Error Banner if API unreachable
            {move || error_banner.get().map(|err| view! {
                <div class="alert-error">
                    <strong>"API Connection Warning: "</strong> {err}
                </div>
            })}

            <main class="main-container">
                {move || match selected_job_id.get() {
                    None => view! {
                        <div class="dashboard-view">
                            // 1. KPI Summary Strip
                            {move || {
                                let stats = queue_snap.get().map(|s| s.stats).unwrap_or_default();
                                view! {
                                    <section class="kpi-grid">
                                        <div class="kpi-tile">
                                            <div class="kpi-label">"QUEUE DEPTH"</div>
                                            <div class="kpi-value kpi-amber">{stats.queued_jobs}</div>
                                            <div class="kpi-sub">"Waiting in FIFO order"</div>
                                        </div>
                                        <div class="kpi-tile">
                                            <div class="kpi-label">"ACTIVE WORKER"</div>
                                            <div class="kpi-value kpi-blue">{format!("{}/1", stats.running_jobs)}</div>
                                            <div class="kpi-sub">"Single-host concurrency"</div>
                                        </div>
                                        <div class="kpi-tile">
                                            <div class="kpi-label">"PASSING BUILDS"</div>
                                            <div class="kpi-value kpi-green">{stats.completed_jobs}</div>
                                            <div class="kpi-sub">"Exit code 0"</div>
                                        </div>
                                        <div class="kpi-tile">
                                            <div class="kpi-label">"FAILED BUILDS"</div>
                                            <div class="kpi-value kpi-red">{stats.failed_jobs}</div>
                                            <div class="kpi-sub">"Requires attention"</div>
                                        </div>
                                        <div class="kpi-tile">
                                            <div class="kpi-label">"COALESCED EVENTS"</div>
                                            <div class="kpi-value kpi-muted">{stats.superseded_jobs}</div>
                                            <div class="kpi-sub">"Superseded in queue"</div>
                                        </div>
                                        <div class="kpi-tile">
                                            <div class="kpi-label">"TOTAL TRACKED"</div>
                                            <div class="kpi-value">{stats.total_jobs}</div>
                                            <div class="kpi-sub">"Persisted in index"</div>
                                        </div>
                                    </section>
                                }
                            }}

                            // Cluster Fabric Nodes Section
                            {move || {
                                let nodes = cluster_nodes.get();
                                if nodes.is_empty() {
                                    view! { <span /> }.into_any()
                                } else {
                                    let online_count = nodes.iter().filter(|n| n.is_online).count();
                                    view! {
                                        <section class="panel" style="margin-bottom: 20px;">
                                            <div class="panel-header">
                                                <div>
                                                    <h2 class="panel-title">"Distributed Fabric Nodes"</h2>
                                                    <div class="panel-subtitle">"Real-time hardware acceleration and execution slot telemetry"</div>
                                                </div>
                                                <span class="mono muted-text">{format!("{} online node(s)", online_count)}</span>
                                            </div>
                                            <div class="nodes-grid">
                                                {nodes.into_iter().map(|node| {
                                                    let status_cls = if node.is_online { "node-pill node-online" } else { "node-pill node-offline" };
                                                    let status_txt = if node.is_online { "● ONLINE" } else { "○ OFFLINE" };
                                                    let slots_txt = format!("{}/{} slots", node.active_slots, node.max_slots);
                                                    let ram_txt = format!("{}/{} MB", node.ram_used_mb, node.ram_total_mb);
                                                    let cpu_txt = format!("{:.1}% CPU", node.cpu_load_pct);
                                                    let arch = node.arch.clone();
                                                    view! {
                                                        <div class="node-card">
                                                            <div class="node-card-header">
                                                                <div>
                                                                    <div class="node-name">{node.node_id}</div>
                                                                    <span class="mono-chip">{arch}</span>
                                                                </div>
                                                                <span class=status_cls>{status_txt}</span>
                                                            </div>
                                                            <div class="node-metrics-row">
                                                                <span class="node-metric"><strong>"Slots: "</strong>{slots_txt}</span>
                                                                <span class="node-metric"><strong>"CPU: "</strong>{cpu_txt}</span>
                                                                <span class="node-metric"><strong>"RAM: "</strong>{ram_txt}</span>
                                                            </div>
                                                            <div class="node-tags-row">
                                                                {node.tags.into_iter().map(|tag| view! { <span class="tag-chip">{tag}</span> }).collect_view()}
                                                            </div>
                                                        </div>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        </section>
                                    }.into_any()
                                }
                            }}

                            // 2. Active Job & Pending Queue Monitor Section
                            <section class="panel">
                                <div class="panel-header">
                                    <h2 class="panel-title">"Live Webhook Queue Monitor"</h2>
                                    <span class="panel-badge">
                                        {move || {
                                            let depth = queue_snap.get().map(|s| s.queue_depth).unwrap_or(0);
                                            format!("{} Queued", depth)
                                        }}
                                    </span>
                                </div>

                                // Active Running Job Spotlight
                                {move || {
                                    let active_opt = queue_snap.get().and_then(|s| s.active_job);
                                    match active_opt {
                                        Some(active) => {
                                            let jid = active.record.job.id.clone();
                                            let repo = active.record.job.repo_name.clone();
                                            let branch = short_branch(&active.record.job.ref_name).to_string();
                                            let sha = short_sha(&active.record.job.commit_sha).to_string();
                                            let sender = active.record.job.sender.clone();
                                            let evt = active.record.job.event_type.clone();
                                            let elapsed = active.elapsed_secs.unwrap_or(0);
                                            view! {
                                                <div class="active-job-spotlight">
                                                    <div class="spotlight-left">
                                                        <span class="status-pill status-running">"⏳ EXECUTING NOW"</span>
                                                        <span class="job-repo">{repo}</span>
                                                        <span class="branch-pill">{branch}</span>
                                                        <span class="mono-chip">{sha}</span>
                                                        <span class="muted-text">{format!("{} by @{}", evt, sender)}</span>
                                                    </div>
                                                    <div class="spotlight-right">
                                                        <span class="mono-timer">{format!("Elapsed: {}s", elapsed)}</span>
                                                        <button
                                                            class="btn btn-primary"
                                                            on:click=move |_| open_job_drilldown(jid.clone())
                                                        >
                                                            "Drill Down & Live Log →"
                                                        </button>
                                                    </div>
                                                </div>
                                            }.into_any()
                                        }
                                        None => view! {
                                            <div class="queue-empty-active">
                                                "No build currently executing — worker is idle and ready for incoming GitHub webhooks."
                                            </div>
                                        }.into_any()
                                    }
                                }}

                                // Pending FIFO Queue Items
                                {move || {
                                    let queued = queue_snap.get().map(|s| s.queued_jobs).unwrap_or_default();
                                    if queued.is_empty() {
                                        view! {
                                            <div class="queue-empty-list">
                                                "Queue is empty (0 pending webhook jobs)."
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="queue-list">
                                                {queued.into_iter().map(|item| {
                                                    let jid = item.record.job.id.clone();
                                                    let pos = item.position;
                                                    let wait = item.wait_secs;
                                                    let repo = item.record.job.repo_name.clone();
                                                    let branch = short_branch(&item.record.job.ref_name).to_string();
                                                    let sha = short_sha(&item.record.job.commit_sha).to_string();
                                                    let trigger = format!("{} by @{}", item.record.job.event_type, item.record.job.sender);
                                                    view! {
                                                        <div
                                                            class="queue-row"
                                                            on:click=move |_| open_job_drilldown(jid.clone())
                                                        >
                                                            <div class="queue-row-left">
                                                                <span class="queue-pos-badge">{format!("#{}", pos)}</span>
                                                                <span class="status-pill status-queued">"⏱ QUEUED"</span>
                                                                <span class="job-repo">{repo}</span>
                                                                <span class="branch-pill">{branch}</span>
                                                                <span class="mono-chip">{sha}</span>
                                                                <span class="muted-text">{trigger}</span>
                                                            </div>
                                                            <div class="queue-row-right">
                                                                <span class="muted-text mono">{format!("waiting {}s", wait)}</span>
                                                                <span class="drill-link">"Inspect →"</span>
                                                            </div>
                                                        </div>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        }.into_any()
                                    }
                                }}
                            </section>

                            // 3. Current & Past Jobs Explorer
                            <section class="panel">
                                <div class="panel-header">
                                    <h2 class="panel-title">"Current & Past Jobs History"</h2>
                                    <div class="filter-bar">
                                        <select
                                            class="select-input"
                                            aria-label="Filter jobs by repository"
                                            on:change=move |ev| repo_filter.set(event_target_value(&ev))
                                        >
                                            <option value="all">"All Repositories"</option>
                                            {move || repositories().into_iter().map(|r| {
                                                let val = r.clone();
                                                view! { <option value=val>{r}</option> }
                                            }).collect_view()}
                                        </select>

                                        <div class="status-tabs" role="tablist" aria-label="Filter jobs by status">
                                            {[
                                                ("all", "All", "All"),
                                                ("running", "Running", "Running"),
                                                ("queued", "Queued", "Queued"),
                                                ("passing", "Passing", "Passing"),
                                                ("failing", "Failing", "Failing"),
                                                ("skipped", "Skipped / Coalesced", "Skipped"),
                                            ].into_iter().map(|(val, full_label, short_label)| {
                                                let v = val.to_string();
                                                view! {
                                                    <button
                                                        type="button"
                                                        role="tab"
                                                        class=move || if status_filter.get() == val { "tab-btn tab-active" } else { "tab-btn" }
                                                        on:click=move |_| status_filter.set(v.clone())
                                                    >
                                                        <span class="tab-label-full">{full_label}</span>
                                                        <span class="tab-label-short">{short_label}</span>
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>

                                        <input
                                            type="text"
                                            class="search-input"
                                            aria-label="Filter jobs by query"
                                            placeholder="Filter by SHA, branch, sender, job ID..."
                                            prop:value=move || search_query.get()
                                            on:input=move |ev| search_query.set(event_target_value(&ev))
                                        />
                                    </div>
                                </div>

                                <div class="jobs-table">
                                    {move || {
                                        let jobs = filtered_jobs();
                                        let now = Utc::now();
                                        if jobs.is_empty() {
                                            view! {
                                                <div class="queue-empty-list">
                                                    "No jobs match the current filters."
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <div>
                                                    {jobs.into_iter().map(|rec| {
                                                        let jid = rec.job.id.clone();
                                                        let jid_label = rec.job.id.clone();
                                                        let repo = rec.job.repo_name.clone();
                                                        let branch = short_branch(&rec.job.ref_name).to_string();
                                                        let sha = short_sha(&rec.job.commit_sha).to_string();
                                                        let trigger = format!("{} by @{}", rec.job.event_type, rec.job.sender);
                                                        let status_cls = rec.status.css_class();
                                                        let status_lbl = rec.status.badge_label();
                                                        let duration_str = rec.status.duration_label(now);
                                                        let updated_str = rec.updated_at.format("%Y-%m-%d %H:%M UTC").to_string();

                                                        let metrics_opt = rec.metrics.clone();
                                                        let sub_list: Vec<_> = metrics_opt
                                                            .as_ref()
                                                            .map(|m| m.submodules.values().cloned().collect())
                                                            .unwrap_or_default();

                                                        let status_note = match &rec.status {
                                                            JobStatus::Failed { error } => Some(error.clone()),
                                                            JobStatus::Skipped { reason } => Some(reason.clone()),
                                                            _ => None,
                                                        };

                                                        view! {
                                                            <div
                                                                class="job-card-row"
                                                                on:click=move |_| open_job_drilldown(jid.clone())
                                                            >
                                                                <div class="job-row-top">
                                                                    <div class="job-row-title">
                                                                        <span class=status_cls>{status_lbl}</span>
                                                                        <span class="job-repo">{repo}</span>
                                                                        <span class="branch-pill">{branch}</span>
                                                                        <span class="job-id-chip mono">{jid_label}</span>
                                                                    </div>

                                                                    <div class="job-row-badges">
                                                                        {metrics_opt.as_ref().map(|m| {
                                                                            let s_cls = score_pill_class(m.quality_score);
                                                                            let score_txt = format!("{}% {}", m.quality_score, m.letter_grade);
                                                                            let tests_txt = format!("{}/{} Tests", m.tests_passed, m.tests_total);
                                                                            let cov_txt = m.coverage_pct.map(|c| format!("{:.1}% Cov", c));
                                                                            view! {
                                                                                <div class="metrics-mini-row">
                                                                                    <span class="metric-chip">{tests_txt}</span>
                                                                                    {cov_txt.map(|c| view! { <span class="metric-chip">{c}</span> })}
                                                                                    <span class=s_cls>{score_txt}</span>
                                                                                </div>
                                                                            }
                                                                        })}
                                                                        <span class="drill-link">"Drill Down →"</span>
                                                                    </div>
                                                                </div>

                                                                // Submodule pills (supports 3 or more submodules wrapping cleanly)
                                                                {(!sub_list.is_empty()).then(|| view! {
                                                                    <div class="submodule-pills-row">
                                                                        <span class="sub-row-label">"SUBMODULES"</span>
                                                                        {sub_list.into_iter().map(|sub| {
                                                                            let s_cls = score_pill_class(sub.quality_score);
                                                                            let s_txt = format!("{}% {}", sub.quality_score, sub.letter_grade);
                                                                            view! {
                                                                                <span class="submodule-pill">
                                                                                    <span class="sub-pill-name">{sub.name}</span>
                                                                                    <span class=s_cls>{s_txt}</span>
                                                                                </span>
                                                                            }
                                                                        }).collect_view()}
                                                                    </div>
                                                                })}

                                                                {status_note.map(|note| view! {
                                                                    <div class="job-status-note">{note}</div>
                                                                })}

                                                                // Two-line metadata layout matching the fixed K3 scorecard
                                                                <div class="job-meta-grid">
                                                                    <div class="job-meta-line">
                                                                        <span><span class="meta-label">"Commit: "</span><span class="mono">{sha}</span></span>
                                                                        <span><span class="meta-label">"Trigger: "</span>{trigger}</span>
                                                                    </div>
                                                                    <div class="job-meta-line">
                                                                        <span><span class="meta-label">"Duration: "</span><span class="mono">{duration_str}</span></span>
                                                                        <span><span class="meta-label">"Updated: "</span>{updated_str}</span>
                                                                    </div>
                                                                </div>
                                                            </div>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            }.into_any()
                                        }
                                    }}
                                </div>
                            </section>
                        </div>
                    }.into_any(),

                    // JOB DRILL-DOWN VIEW
                    Some(job_id) => {
                        let rec_opt = selected_job_record.get();
                        let base = api_base.get();
                        let raw_log = selected_job_log.get();
                        let parsed_log = parse_job_log(&raw_log);
                        let now = Utc::now();

                        view! {
                            <div class="drilldown-view">
                                <div class="drilldown-nav">
                                    <button class="btn" on:click=close_drilldown>
                                        "← Back to Queue & Jobs Monitor"
                                    </button>
                                    <span class="mono muted-text">{format!("Job ID: {}", job_id)}</span>
                                </div>

                                {match rec_opt {
                                    None => view! {
                                        <div class="panel">
                                            <div class="queue-empty-list">"Loading job details..."</div>
                                        </div>
                                    }.into_any(),
                                    Some(rec) => {
                                        let repo = rec.job.repo_name.clone();
                                                        let branch = short_branch(&rec.job.ref_name).to_string();
                                        let sha = short_sha(&rec.job.commit_sha).to_string();
                                        let full_sha = rec.job.commit_sha.clone();
                                        let trigger = format!("{} by @{}", rec.job.event_type, rec.job.sender);
                                        let duration_str = rec.status.duration_label(now);
                                        let updated_str = rec.updated_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
                                        let queued_str = rec.job.queued_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
                                        let status_cls = rec.status.css_class();
                                        let status_lbl = rec.status.badge_label();
                                        let card_url = card_svg_url(&base, &rec.job.repo_name, &rec.job.ref_name);

                                        let callout = match &rec.status {
                                            JobStatus::Failed { error } => Some(("alert-error", format!("Failure Reason: {}", error))),
                                            JobStatus::Skipped { reason } => Some(("alert-skipped", format!("Skipped / Coalesced: {}", reason))),
                                            _ => None,
                                        };

                                        let metrics_opt = rec.metrics.clone();
                                        let submodules: Vec<_> = metrics_opt
                                            .as_ref()
                                            .map(|m| m.submodules.values().cloned().collect())
                                            .unwrap_or_default();

                                        view! {
                                            <div>
                                                // Drill-Down Header Card
                                                <section class="panel">
                                                    <div class="drilldown-header">
                                                        <div>
                                                            <div class="job-row-title">
                                                                <span class=status_cls>{status_lbl}</span>
                                                                <h1 class="drilldown-repo">{repo.clone()}</h1>
                                                                <span class="branch-pill">{branch}</span>
                                                            </div>
                                                            <div class="drilldown-meta-rows">
                                                                <div class="job-meta-line">
                                                                    <span><span class="meta-label">"Commit: "</span><span class="mono" title=full_sha>{sha}</span></span>
                                                                    <span><span class="meta-label">"Trigger: "</span>{trigger}</span>
                                                                    <span><span class="meta-label">"Queued At: "</span>{queued_str}</span>
                                                                </div>
                                                                <div class="job-meta-line">
                                                                    <span><span class="meta-label">"Duration: "</span><span class="mono">{duration_str}</span></span>
                                                                    <span><span class="meta-label">"Updated: "</span>{updated_str}</span>
                                                                    {rec.queue_position.map(|p| view! {
                                                                        <span><span class="meta-label">"Queue Position: "</span><span class="mono">{format!("#{}", p)}</span></span>
                                                                    })}
                                                                </div>
                                                            </div>
                                                        </div>

                                                        {metrics_opt.as_ref().map(|m| {
                                                            let s_cls = score_pill_class(m.quality_score);
                                                            view! {
                                                                <div class="drilldown-score-box">
                                                                    <div class="kpi-label">"QUALITY SCORE"</div>
                                                                    <span class=s_cls style="font-size: 16px; padding: 6px 14px;">
                                                                        {format!("{}% {}", m.quality_score, m.letter_grade)}
                                                                    </span>
                                                                </div>
                                                            }
                                                        })}
                                                    </div>

                                                    {callout.map(|(cls, msg)| view! {
                                                        <div class=cls style="margin-top: 12px;">{msg}</div>
                                                    })}

                                                    // DAG Pipeline Stages
                                                    {
                                                        let stages = rec.stages.clone();
                                                        if stages.is_empty() {
                                                            view! { <span /> }.into_any()
                                                        } else {
                                                            view! {
                                                                <div style="margin-top: 16px;">
                                                                    <div class="kpi-label" style="margin-bottom: 8px;">"DAG PIPELINE STAGES"</div>
                                                                    <div class="stages-grid">
                                                                        {stages.into_iter().map(|s| {
                                                                            let st_lbl = s.status.badge_label();
                                                                            let st_cls = s.status.css_class();
                                                                            let tc = s.toolchain.unwrap_or_else(|| "system".to_string());
                                                                            let deps = if s.depends_on.is_empty() { "none".to_string() } else { s.depends_on.join(", ") };
                                                                            view! {
                                                                                <div class="stage-card">
                                                                                    <div class="stage-card-header">
                                                                                        <span class="stage-title">{s.name}</span>
                                                                                        <span class=st_cls>{st_lbl}</span>
                                                                                    </div>
                                                                                    <div class="stage-meta-row">
                                                                                        <span><span class="meta-label">"Stage: "</span><span class="mono">{s.id}</span></span>
                                                                                        <span><span class="meta-label">"Threads: "</span><span class="mono">{s.threads}</span></span>
                                                                                        <span><span class="meta-label">"Toolchain: "</span><span class="mono">{tc}</span></span>
                                                                                    </div>
                                                                                    <div class="stage-meta-row" style="margin-top: 4px;">
                                                                                        <span><span class="meta-label">"Depends: "</span><span class="mono">{deps}</span></span>
                                                                                    </div>
                                                                                    <div class="stage-tags-row">
                                                                                        {s.tags.into_iter().map(|t| view! { <span class="tag-chip">{t}</span> }).collect_view()}
                                                                                    </div>
                                                                                </div>
                                                                            }
                                                                        }).collect_view()}
                                                                    </div>
                                                                </div>
                                                            }.into_any()
                                                        }
                                                    }

                                                    // 4 Factory Pipeline Metric Tiles + Live SVG Card Preview
                                                    {metrics_opt.as_ref().map(|m| {
                                                        let tests_val = format!("{}/{} Passed", m.tests_passed, m.tests_total);
                                                        let cov_val = m.coverage_pct.map(|c| format!("{:.1}%", c)).unwrap_or_else(|| "n/a".to_string());
                                                        let bench_val = m.benchmark_summary.clone().unwrap_or_else(|| "n/a".to_string());
                                                        let hygiene_val = format!("{} Advisories / {} Warnings", m.advisories_count, m.clippy_warnings);
                                                        view! {
                                                            <div class="drilldown-metrics-grid">
                                                                <div class="kpi-tile">
                                                                    <div class="kpi-label">"TESTS"</div>
                                                                    <div class="kpi-value">{tests_val}</div>
                                                                    <div class="kpi-sub">"Workspace unit & integration"</div>
                                                                </div>
                                                                <div class="kpi-tile">
                                                                    <div class="kpi-label">"LINE COVERAGE"</div>
                                                                    <div class="kpi-value kpi-green">{cov_val}</div>
                                                                    <div class="kpi-sub">"cargo llvm-cov"</div>
                                                                </div>
                                                                <div class="kpi-tile">
                                                                    <div class="kpi-label">"CARGO BENCH"</div>
                                                                    <div class="kpi-value kpi-blue" style="font-size: 15px;">{bench_val}</div>
                                                                    <div class="kpi-sub">"Native K3 micro-benchmarks"</div>
                                                                </div>
                                                                <div class="kpi-tile">
                                                                    <div class="kpi-label">"SECURITY & LINTS"</div>
                                                                    <div class="kpi-value" style="font-size: 15px;">{hygiene_val}</div>
                                                                    <div class="kpi-sub">"cargo audit & clippy"</div>
                                                                </div>
                                                            </div>
                                                        }
                                                    })}

                                                    <div class="scorecard-preview-box">
                                                        <div class="kpi-label" style="margin-bottom: 8px;">"LIVE SVG FACTORY SCORECARD"</div>
                                                        <img src=card_url alt="Repository Factory Scorecard SVG" class="scorecard-svg-img" />
                                                    </div>
                                                </section>

                                                // Submodules Drill-Down Section (supports 3+ submodules dynamically)
                                                {(!submodules.is_empty()).then(|| {
                                                    let parent_repo = repo.clone();
                                                    let base_for_subs = base.clone();
                                                    view! {
                                                        <section class="panel">
                                                            <div class="panel-header">
                                                                <h2 class="panel-title">
                                                                    {format!("Submodules Quality Breakdown ({})", submodules.len())}
                                                                </h2>
                                                            </div>
                                                            <div class="submodules-grid">
                                                                {submodules.into_iter().map(|sub| {
                                                                    let s_cls = score_pill_class(sub.quality_score);
                                                                    let s_txt = format!("{}% {}", sub.quality_score, sub.letter_grade);
                                                                    let sub_card = submodule_card_svg_url(&parent_repo, &sub.name, &base_for_subs);
                                                                    let cov_str = sub.coverage_pct.map(|c| format!("{:.1}%", c)).unwrap_or_else(|| "—".to_string());
                                                                    let bench_str = sub.benchmark_summary.clone().unwrap_or_else(|| "—".to_string());
                                                                    view! {
                                                                        <div class="submodule-detail-card">
                                                                            <div class="submodule-card-header">
                                                                                <div>
                                                                                    <span class="job-repo">{sub.name.clone()}</span>
                                                                                    <span class="mono muted-text" style="margin-left: 8px;">{sub.path.clone()}</span>
                                                                                </div>
                                                                                <span class=s_cls>{s_txt}</span>
                                                                            </div>
                                                                            <div class="submodule-stats-row">
                                                                                <span><span class="meta-label">"Tests: "</span>{format!("{}/{}", sub.tests_passed, sub.tests_total)}</span>
                                                                                <span><span class="meta-label">"Coverage: "</span>{cov_str}</span>
                                                                                <span><span class="meta-label">"Bench: "</span>{bench_str}</span>
                                                                                <span><span class="meta-label">"Clippy: "</span>{format!("{} warn", sub.clippy_warnings)}</span>
                                                                            </div>
                                                                            <details class="sub-card-details">
                                                                                <summary>"Preview Submodule Scorecard SVG"</summary>
                                                                                <img src=sub_card alt="Submodule Scorecard" class="scorecard-svg-img" style="margin-top: 8px;" />
                                                                            </details>
                                                                        </div>
                                                                    }
                                                                }).collect_view()}
                                                            </div>
                                                        </section>
                                                    }
                                                })}

                                                // Step-by-Step & Raw Build Log Viewer
                                                <section class="panel">
                                                    <div class="panel-header">
                                                        <h2 class="panel-title">"Build & Execution Logs"</h2>
                                                        <div class="filter-bar">
                                                            <div class="status-tabs" role="tablist" aria-label="Log viewer mode">
                                                                <button
                                                                    type="button"
                                                                    role="tab"
                                                                    class=move || if log_view_mode.get() == "steps" { "tab-btn tab-active" } else { "tab-btn" }
                                                                    on:click=move |_| log_view_mode.set("steps".to_string())
                                                                >
                                                                    "Structured Steps"
                                                                </button>
                                                                <button
                                                                    type="button"
                                                                    role="tab"
                                                                    class=move || if log_view_mode.get() == "raw" { "tab-btn tab-active" } else { "tab-btn" }
                                                                    on:click=move |_| log_view_mode.set("raw".to_string())
                                                                >
                                                                    "Raw Log Output"
                                                                </button>
                                                            </div>

                                                            <input
                                                                type="text"
                                                                class="search-input"
                                                                aria-label="Filter log lines"
                                                                placeholder="Filter log lines..."
                                                                prop:value=move || log_search.get()
                                                                on:input=move |ev| log_search.set(event_target_value(&ev))
                                                            />
                                                        </div>
                                                    </div>

                                                    {move || {
                                                        if log_loading.get() && selected_job_log.get().is_empty() {
                                                            return view! {
                                                                <div class="queue-empty-list">"Loading execution log..."</div>
                                                            }.into_any();
                                                        }

                                                        let filter_q = log_search.get().trim().to_ascii_lowercase();
                                                        if log_view_mode.get() == "steps" && !parsed_log.steps.is_empty() {
                                                            let steps = parsed_log.steps.clone();
                                                            let pre = parsed_log.pre_step_output.clone();
                                                            let post = parsed_log.footer_output.clone();
                                                            view! {
                                                                <div class="steps-container">
                                                                    {(!pre.is_empty()).then(|| view! {
                                                                        <details class="step-accordion">
                                                                            <summary class="step-summary">
                                                                                <span class="step-badge step-passed">"✓ GIT SYNC"</span>
                                                                                <span class="step-title">"Workspace Clone, Reset & Submodule Sync"</span>
                                                                            </summary>
                                                                            <pre class="log-pre">{pre}</pre>
                                                                        </details>
                                                                    })}

                                                                    {steps.into_iter().map(|step| {
                                                                        let is_failed = step.status == StepStatus::Failed || step.status == StepStatus::TimedOut || step.status == StepStatus::Running;
                                                                        let badge_cls = step.status.css_class();
                                                                        let badge_lbl = step.status.label();
                                                                        let header_title = format!("Step [{}/{}]: {}", step.number, step.total, step.name);
                                                                        let cmd_str = step.command.clone();
                                                                        let out = if filter_q.is_empty() {
                                                                            step.output.clone()
                                                                        } else {
                                                                            step.output
                                                                                .lines()
                                                                                .filter(|l| l.to_ascii_lowercase().contains(&filter_q))
                                                                                .collect::<Vec<_>>()
                                                                                .join("\n")
                                                                        };
                                                                        view! {
                                                                            <details class="step-accordion" open=is_failed>
                                                                                <summary class="step-summary">
                                                                                    <span class=badge_cls>{badge_lbl}</span>
                                                                                    <span class="step-title">{header_title}</span>
                                                                                    <code class="step-cmd mono">{cmd_str}</code>
                                                                                </summary>
                                                                                <pre class="log-pre">{out}</pre>
                                                                            </details>
                                                                        }
                                                                    }).collect_view()}

                                                                    {(!post.is_empty()).then(|| view! {
                                                                        <pre class="log-pre" style="margin-top: 8px;">{post}</pre>
                                                                    })}
                                                                </div>
                                                            }.into_any()
                                                        } else {
                                                            let raw_txt = selected_job_log.get();
                                                            let filtered = if filter_q.is_empty() {
                                                                raw_txt
                                                            } else {
                                                                raw_txt
                                                                    .lines()
                                                                    .filter(|l| l.to_ascii_lowercase().contains(&filter_q))
                                                                    .collect::<Vec<_>>()
                                                                    .join("\n")
                                                            };
                                                            view! {
                                                                <pre class="log-pre">{filtered}</pre>
                                                            }.into_any()
                                                        }
                                                    }}
                                                </section>
                                            </div>
                                        }.into_any()
                                    }
                                }}
                            </div>
                        }.into_any()
                    }
                }}
            </main>
        </div>
    }
}
