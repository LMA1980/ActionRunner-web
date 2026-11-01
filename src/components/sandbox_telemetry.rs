use leptos::prelude::*;

pub use crate::models::{
    JobRecord, SandboxBackendType, SandboxTelemetryRecord, SecurityGrantViolationRecord,
    StageRecord, StageStatus,
};

/// Canonical bounded `RLIMIT_MEMLOCK` ceiling (28 GiB = `29_360_128 KB`) for SpacemiT K3 target compute worker (`AGENTS.md §5`).
pub const K3_BOUNDED_MEMLOCK_CEILING_KB: u64 = 29_360_128;

/// Default bounded `RLIMIT_MEMLOCK` ceiling (`65_536 KB` / 64 MiB) for general CI stages (`AGENTS.md §5`).
pub const DEFAULT_CI_MEMLOCK_CEILING_KB: u64 = 65_536;

/// Extracts or synthesizes `MuIsolation` sandbox telemetry and `SecurityGrantDenied` fail-closed
/// violations from a `JobRecord` and its raw execution log (`#20`).
pub fn extract_sandbox_telemetry(
    job: &JobRecord,
    raw_log: &str,
) -> (
    Vec<SandboxTelemetryRecord>,
    Vec<SecurityGrantViolationRecord>,
) {
    let mut telemetry = job.sandbox_telemetry.clone();
    let mut violations = job.security_violations.clone();

    if telemetry.is_empty() {
        for stage in &job.stages {
            if let Some(ref st) = stage.sandbox_telemetry {
                telemetry.push(st.clone());
                continue;
            }

            let node_id = stage.execution_node().map(str::to_string);
            let is_k3_worker = matches!(
                node_id.as_deref(),
                Some("worker-node") | Some("worker-riscv64")
            );
            let is_kvm = stage
                .tags
                .iter()
                .any(|t| t.eq_ignore_ascii_case("kvm") || t.eq_ignore_ascii_case("hypervisor"));
            let is_ai_matrix = stage.tags.iter().any(|t| {
                t.eq_ignore_ascii_case("bench")
                    || t.eq_ignore_ascii_case("a100")
                    || t.eq_ignore_ascii_case("rvv")
            });

            let backend = if is_kvm {
                SandboxBackendType::HypervisorSandbox
            } else {
                SandboxBackendType::KernelSandbox
            };

            let profile_name = if is_kvm {
                "kvm-isolated".to_string()
            } else if is_ai_matrix {
                "k3-matrix-bench".to_string()
            } else if stage.id.contains("fmt") || stage.id.contains("clippy") {
                "strict-offline".to_string()
            } else {
                "ci-default".to_string()
            };

            let memlock_limit_kb = if is_k3_worker || is_ai_matrix {
                K3_BOUNDED_MEMLOCK_CEILING_KB
            } else {
                DEFAULT_CI_MEMLOCK_CEILING_KB
            };

            let set_ai_thread_active = is_k3_worker && is_ai_matrix;

            telemetry.push(SandboxTelemetryRecord {
                stage_id: stage.id.clone(),
                node_id,
                backend,
                profile_name,
                memlock_limit_kb,
                set_ai_thread_active,
                max_pids: 128,
                network_egress_cidrs: Vec::new(),
            });
        }
    }

    // Parse any SecurityGrantDenied fail-closed log lines if not already populated
    for line in raw_log.lines() {
        if line.contains("SecurityGrantDenied") {
            let already_recorded = violations
                .iter()
                .any(|v| line.contains(&v.field) && line.contains(&v.stage_id));
            if !already_recorded {
                violations.push(parse_violation_log_line(line));
            }
        }
    }

    // Also inspect stage failure messages for SecurityGrantDenied
    for stage in &job.stages {
        if let StageStatus::Failed { ref error } = stage.status {
            if error.contains("SecurityGrantDenied")
                && !violations.iter().any(|v| v.stage_id == stage.id)
            {
                let mut v = parse_violation_log_line(error);
                v.stage_id = stage.id.clone();
                violations.push(v);
            }
        }
    }

    (telemetry, violations)
}

fn parse_violation_log_line(line: &str) -> SecurityGrantViolationRecord {
    let extract_kv = |key: &str| -> Option<String> {
        let pat = format!("{}=", key);
        let start = line.find(&pat)? + pat.len();
        let rest = &line[start..];
        let val = if let Some(stripped) = rest.strip_prefix('"') {
            stripped.split('"').next()?
        } else {
            rest.split(|c: char| c.is_whitespace() || c == ',' || c == '}' || c == ')')
                .next()?
        };
        Some(val.to_string())
    };

    let timestamp = line
        .split_whitespace()
        .next()
        .filter(|s| s.contains('T') || s.contains(':'))
        .unwrap_or("live")
        .to_string();

    SecurityGrantViolationRecord {
        stage_id: extract_kv("stage").unwrap_or_else(|| "unknown-stage".to_string()),
        violated_layer: extract_kv("layer").unwrap_or_else(|| "TaskOverrideVsProfile".to_string()),
        field: extract_kv("field").unwrap_or_else(|| "memlock_limit_kb".to_string()),
        requested_value: extract_kv("requested").unwrap_or_else(|| "unlimited".to_string()),
        ceiling_value: extract_kv("ceiling")
            .unwrap_or_else(|| format!("{} KB", K3_BOUNDED_MEMLOCK_CEILING_KB)),
        timestamp,
    }
}

/// Renders a deterministic HTML string of the `MuIsolation` Sandbox Telemetry Panel
/// and `SecurityGrantDenied` violation alert table (`#20`).
pub fn render_sandbox_telemetry_html(
    telemetry: &[SandboxTelemetryRecord],
    violations: &[SecurityGrantViolationRecord],
) -> String {
    let mut html = String::new();
    html.push_str(r#"<div class="muisolation-telemetry-panel">"#);

    if !violations.is_empty() {
        html.push_str(
            r#"<div class="security-violation-alert" role="alert"><div class="violation-alert-header">🚨 FAIL-CLOSED SECURITY GRANT DENIED (SecurityGrantDenied)</div><div style="overflow-x: auto;"><table class="violation-alert-table"><thead><tr><th>Stage</th><th>Violated Layer</th><th>Field</th><th>Requested</th><th>Allowed Ceiling</th><th>Timestamp</th></tr></thead><tbody>"#,
        );
        for v in violations {
            html.push_str(&format!(
                r#"<tr class="violation-row"><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
                v.stage_id,
                v.violated_layer,
                v.field,
                v.requested_value,
                v.ceiling_value,
                v.timestamp
            ));
        }
        html.push_str("</tbody></table></div></div>");
    }

    html.push_str(r#"<div class="sandbox-telemetry-grid">"#);
    for rec in telemetry {
        let ai_thread_lbl = if rec.set_ai_thread_active {
            "/proc/set_ai_thread: REGISTERED (A100 Cluster 1)"
        } else {
            "/proc/set_ai_thread: conditional (inactive)"
        };
        html.push_str(&format!(
            r#"<div class="sandbox-stage-card" data-stage="{}"><span class="{}">{}</span><span class="sandbox-profile-name">profile: {}</span><span class="sandbox-memlock">RLIMIT_MEMLOCK: {} KB</span><span class="sandbox-ai-thread">{}</span></div>"#,
            rec.stage_id,
            rec.backend.css_class(),
            rec.backend.label(),
            rec.profile_name,
            rec.memlock_limit_kb,
            ai_thread_lbl
        ));
    }
    html.push_str("</div></div>");
    html
}

/// `MuIsolation` Active Sandbox Profile & `SecurityGrantDenied` Violation Telemetry Panel (`#20`).
#[component]
pub fn MuIsolationTelemetryPanel(
    telemetry: Vec<SandboxTelemetryRecord>,
    violations: Vec<SecurityGrantViolationRecord>,
) -> impl IntoView {
    let has_violations = !violations.is_empty();
    let violation_count = violations.len();
    let stage_count = telemetry.len();

    view! {
        <section class="panel muisolation-telemetry-panel">
            <div class="panel-header">
                <div>
                    <h2 class="panel-title">"MuIsolation Active Sandbox Profile & Security Telemetry"</h2>
                    <div class="muted-text" style="font-size: 11px; margin-top: 2px;">
                        "3-Layer Monotonic Privilege Attenuation (.actionrunner/security.yml) • Bounded RLIMIT_MEMLOCK • Conditional /proc/set_ai_thread"
                    </div>
                </div>
                <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
                    {if has_violations {
                        view! {
                            <span class="status-pill status-failing mono">
                                {format!("🚨 {} SecurityGrantDenied", violation_count)}
                            </span>
                        }.into_any()
                    } else {
                        view! {
                            <span class="status-pill status-passing mono">
                                "✓ 0 Policy Violations"
                            </span>
                        }.into_any()
                    }}
                    <span class="panel-badge">{format!("{} Sandboxed Stages", stage_count)}</span>
                </div>
            </div>

            // High-visibility Fail-Closed SecurityGrantDenied Alert Table
            {has_violations.then(|| view! {
                <div class="security-violation-alert" role="alert">
                    <div class="violation-alert-header">
                        <span>"🚨 FAIL-CLOSED PRIVILEGE ESCALATION BLOCKED (SecurityGrantDenied)"</span>
                        <span class="mono" style="font-size: 11px;">"Monotonic 3-Layer Policy Intersection"</span>
                    </div>
                    <div class="violation-table-scroll" style="overflow-x: auto; max-width: 100%;">
                        <table class="violation-alert-table mono">
                            <thead>
                                <tr>
                                    <th>"Stage ID"</th>
                                    <th>"Violated Layer"</th>
                                    <th>"Policy Field"</th>
                                    <th>"Requested Grant"</th>
                                    <th>"Enforced Ceiling"</th>
                                    <th>"Timestamp"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {violations.into_iter().map(|v| view! {
                                    <tr class="violation-row">
                                        <td class="violation-cell-stage">{v.stage_id}</td>
                                        <td>{v.violated_layer}</td>
                                        <td class="violation-cell-field">{v.field}</td>
                                        <td class="violation-cell-req">{v.requested_value}</td>
                                        <td class="violation-cell-ceil">{v.ceiling_value}</td>
                                        <td class="muted-text">{v.timestamp}</td>
                                    </tr>
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                </div>
            })}

            // Per-Stage Sandbox Telemetry Cards
            {if telemetry.is_empty() {
                view! {
                    <div class="queue-empty-list">
                        "No stage sandbox telemetry recorded for this job."
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="sandbox-telemetry-grid">
                        {telemetry.into_iter().map(|rec| {
                            let backend_cls = rec.backend.css_class();
                            let backend_lbl = rec.backend.label();
                            let node_lbl = rec.node_id.clone().unwrap_or_else(|| "worker-node".to_string());
                            let memlock_str = format!("{} KB ({:.1} MiB)", rec.memlock_limit_kb, rec.memlock_limit_kb as f64 / 1024.0);
                            let ai_thread_badge = if rec.set_ai_thread_active {
                                ("✓ /proc/set_ai_thread: REGISTERED (A100)", "ai-thread-pill ai-thread-active")
                            } else {
                                ("○ /proc/set_ai_thread: conditional (inactive)", "ai-thread-pill ai-thread-inactive")
                            };
                            let egress_str = if rec.network_egress_cidrs.is_empty() {
                                "0.0.0.0/0 DENIED (strict offline)".to_string()
                            } else {
                                rec.network_egress_cidrs.join(", ")
                            };

                            view! {
                                <div class="sandbox-stage-card">
                                    <div class="sandbox-card-top">
                                        <div>
                                            <span class="sandbox-stage-title">{rec.stage_id}</span>
                                            <span class="mono-chip mono" style="margin-left: 6px;">{node_lbl}</span>
                                        </div>
                                        <span class=backend_cls>{backend_lbl}</span>
                                    </div>

                                    <div class="sandbox-card-metrics mono">
                                        <div>
                                            <span class="meta-label">"Active Profile: "</span>
                                            <span class="sandbox-profile-chip">{rec.profile_name}</span>
                                        </div>
                                        <div>
                                            <span class="meta-label">"Bounded RLIMIT_MEMLOCK: "</span>
                                            <span style="color: #7ee787; font-weight: 600;">{memlock_str}</span>
                                        </div>
                                        <div>
                                            <span class="meta-label">"SpacemiT HMP Hook: "</span>
                                            <span class=ai_thread_badge.1>{ai_thread_badge.0}</span>
                                        </div>
                                        <div>
                                            <span class="meta-label">"PIDs / Egress: "</span>
                                            <span class="muted-text">{format!("max_pids={} • {}", rec.max_pids, egress_str)}</span>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            }}
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_normal_kernel_sandbox_telemetry_and_security_grant_denied_alert() {
        let telemetry = vec![
            SandboxTelemetryRecord {
                stage_id: "k3-matrix-bench".to_string(),
                node_id: Some("worker-node".to_string()),
                backend: SandboxBackendType::KernelSandbox,
                profile_name: "k3-matrix-bench".to_string(),
                memlock_limit_kb: K3_BOUNDED_MEMLOCK_CEILING_KB,
                set_ai_thread_active: true,
                max_pids: 64,
                network_egress_cidrs: vec![],
            },
            SandboxTelemetryRecord {
                stage_id: "kvm-integration".to_string(),
                node_id: Some("control-node".to_string()),
                backend: SandboxBackendType::HypervisorSandbox,
                profile_name: "kvm-isolated".to_string(),
                memlock_limit_kb: DEFAULT_CI_MEMLOCK_CEILING_KB,
                set_ai_thread_active: false,
                max_pids: 128,
                network_egress_cidrs: vec![],
            },
        ];

        let violations = vec![SecurityGrantViolationRecord {
            stage_id: "untrusted-task".to_string(),
            violated_layer: "ProfileVsHostCeiling".to_string(),
            field: "memlock_limit_kb".to_string(),
            requested_value: "67108864 KB".to_string(),
            ceiling_value: "29360128 KB".to_string(),
            timestamp: "2026-11-01T08:00:00Z".to_string(),
        }];

        let html = render_sandbox_telemetry_html(&telemetry, &violations);
        assert!(html.contains("KernelSandbox (Landlock + cgroupv2 + seccomp)"));
        assert!(html.contains("HypervisorSandbox (/dev/kvm Guest VM)"));
        assert!(html.contains("RLIMIT_MEMLOCK: 29360128 KB"));
        assert!(html.contains("/proc/set_ai_thread: REGISTERED (A100 Cluster 1)"));
        assert!(html.contains("SecurityGrantDenied"));
        assert!(html.contains("ProfileVsHostCeiling"));
        assert!(html.contains("67108864 KB"));
    }
}
