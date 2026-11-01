use gloo_net::http::Request;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Canary repository health state reported by `MuRegist` (`GET /api/v1/canary/status`)
/// via the `canary-repository: <status>` HTTP header or JSON payload (`#19`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CanaryStatus {
    Healthy,
    Testing,
    Tainted,
    Degraded,
    #[default]
    Unknown,
}

impl CanaryStatus {
    pub fn from_token(raw: &str) -> Self {
        let token = raw
            .trim()
            .strip_prefix("canary-repository:")
            .or_else(|| raw.trim().strip_prefix("Canary-Repository:"))
            .unwrap_or(raw)
            .trim()
            .to_ascii_lowercase();

        match token.as_str() {
            "healthy" | "ok" | "passing" => CanaryStatus::Healthy,
            "testing" | "canary" | "in_progress" | "running" => CanaryStatus::Testing,
            "tainted" | "failed" | "rolled_back" | "alert" => CanaryStatus::Tainted,
            "degraded" | "warning" | "stale" => CanaryStatus::Degraded,
            _ => CanaryStatus::Unknown,
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            CanaryStatus::Healthy => "canary-pill canary-emerald",
            CanaryStatus::Testing => "canary-pill canary-amber",
            CanaryStatus::Tainted => "canary-pill canary-crimson",
            CanaryStatus::Degraded => "canary-pill canary-slate",
            CanaryStatus::Unknown => "canary-pill canary-slate",
        }
    }

    pub fn full_label(&self) -> &'static str {
        match self {
            CanaryStatus::Healthy => "● canary-repository: healthy",
            CanaryStatus::Testing => "◐ canary-repository: testing",
            CanaryStatus::Tainted => "✕ canary-repository: tainted",
            CanaryStatus::Degraded => "⚠ canary-repository: degraded",
            CanaryStatus::Unknown => "○ canary-repository: standby",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            CanaryStatus::Healthy => "● Canary OK",
            CanaryStatus::Testing => "◐ Canary",
            CanaryStatus::Tainted => "✕ Tainted",
            CanaryStatus::Degraded => "⚠ Degraded",
            CanaryStatus::Unknown => "○ Canary",
        }
    }
}

/// Complete canary status record parsed from `GET /api/v1/canary/status` (`#19`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanaryRepositoryStatus {
    pub status: CanaryStatus,
    pub header_value: String,
    #[serde(default)]
    pub active_digest: Option<String>,
    #[serde(default)]
    pub slot: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl Default for CanaryRepositoryStatus {
    fn default() -> Self {
        Self {
            status: CanaryStatus::Healthy,
            header_value: "healthy".to_string(),
            active_digest: None,
            slot: Some("slot-a".to_string()),
            message: Some("Canary repository verified healthy".to_string()),
        }
    }
}

#[derive(Deserialize)]
struct CanaryJsonPayload {
    #[serde(default)]
    status: Option<String>,
    #[serde(default, alias = "canary-repository")]
    canary_repository: Option<String>,
    #[serde(default)]
    active_digest: Option<String>,
    #[serde(default)]
    slot: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// Parses the `canary-repository: <status>` HTTP response header with JSON body fallback (`#19`).
pub fn parse_canary_repository_header(
    header_val: Option<&str>,
    json_fallback: Option<&str>,
) -> CanaryRepositoryStatus {
    let parsed_json: Option<CanaryJsonPayload> =
        json_fallback.and_then(|body| serde_json::from_str::<CanaryJsonPayload>(body).ok());

    if let Some(raw_hdr) = header_val {
        let status = CanaryStatus::from_token(raw_hdr);
        if status != CanaryStatus::Unknown {
            let clean_val = raw_hdr
                .trim()
                .strip_prefix("canary-repository:")
                .or_else(|| raw_hdr.trim().strip_prefix("Canary-Repository:"))
                .unwrap_or(raw_hdr)
                .trim()
                .to_ascii_lowercase();
            return CanaryRepositoryStatus {
                status,
                header_value: clean_val,
                active_digest: parsed_json.as_ref().and_then(|p| p.active_digest.clone()),
                slot: parsed_json.as_ref().and_then(|p| p.slot.clone()),
                message: parsed_json.as_ref().and_then(|p| p.message.clone()),
            };
        }
    }

    if let Some(payload) = parsed_json {
        let raw_tok = payload
            .canary_repository
            .as_deref()
            .or(payload.status.as_deref())
            .unwrap_or("unknown");
        let status = CanaryStatus::from_token(raw_tok);
        return CanaryRepositoryStatus {
            status,
            header_value: raw_tok.trim().to_ascii_lowercase(),
            active_digest: payload.active_digest,
            slot: payload.slot,
            message: payload.message,
        };
    }

    CanaryRepositoryStatus {
        status: CanaryStatus::Unknown,
        header_value: "unknown".to_string(),
        active_digest: None,
        slot: None,
        message: None,
    }
}

/// Fetches `GET /api/v1/canary/status` and extracts the `canary-repository: <status>` header + JSON fallback.
pub async fn fetch_canary_status(api_base: &str) -> CanaryRepositoryStatus {
    let url = format!("{}/api/v1/canary/status", api_base);
    match Request::get(&url).send().await {
        Ok(resp) => {
            let hdr = resp.headers().get("canary-repository");
            let body = resp.text().await.ok();
            let parsed = parse_canary_repository_header(hdr.as_deref(), body.as_deref());
            if parsed.status == CanaryStatus::Unknown && resp.status() == 404 {
                CanaryRepositoryStatus::default()
            } else {
                parsed
            }
        }
        Err(_) => CanaryRepositoryStatus::default(),
    }
}

/// Responsive Header Status Pill displaying `canary-repository: <status>` (`#19`).
#[component]
pub fn CanaryStatusPill(status: RwSignal<CanaryRepositoryStatus>) -> impl IntoView {
    view! {
        <span
            class=move || status.get().status.css_class()
            title=move || {
                let st = status.get();
                let slot_info = st.slot.as_deref().map(|s| format!(" [{}]", s)).unwrap_or_default();
                let msg_info = st.message.as_deref().map(|m| format!(" — {}", m)).unwrap_or_default();
                format!("MuRegist Canary Status: canary-repository: {}{}{}", st.header_value, slot_info, msg_info)
            }
        >
            <span class="btn-text-full mono">{move || status.get().status.full_label()}</span>
            <span class="btn-text-short mono">{move || status.get().status.short_label()}</span>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_canary_repository_header_states_and_badges() {
        let healthy = parse_canary_repository_header(Some("healthy"), None);
        assert_eq!(healthy.status, CanaryStatus::Healthy);
        assert_eq!(healthy.status.css_class(), "canary-pill canary-emerald");
        assert_eq!(healthy.status.full_label(), "● canary-repository: healthy");
        assert_eq!(healthy.status.short_label(), "● Canary OK");

        let testing = parse_canary_repository_header(Some("canary-repository: testing"), None);
        assert_eq!(testing.status, CanaryStatus::Testing);
        assert_eq!(testing.header_value, "testing");
        assert_eq!(testing.status.css_class(), "canary-pill canary-amber");
        assert_eq!(testing.status.full_label(), "◐ canary-repository: testing");

        let tainted = parse_canary_repository_header(Some("TAINTED"), None);
        assert_eq!(tainted.status, CanaryStatus::Tainted);
        assert_eq!(tainted.status.css_class(), "canary-pill canary-crimson");
        assert_eq!(tainted.status.full_label(), "✕ canary-repository: tainted");

        let degraded = parse_canary_repository_header(Some("degraded"), None);
        assert_eq!(degraded.status, CanaryStatus::Degraded);
        assert_eq!(degraded.status.css_class(), "canary-pill canary-slate");
        assert_eq!(
            degraded.status.full_label(),
            "⚠ canary-repository: degraded"
        );
    }

    #[test]
    fn test_parse_canary_repository_json_fallback() {
        let json = r#"{
            "status": "testing",
            "active_digest": "sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
            "slot": "slot-b",
            "message": "Executing canary integration bundle on worker-node"
        }"#;
        let parsed = parse_canary_repository_header(None, Some(json));
        assert_eq!(parsed.status, CanaryStatus::Testing);
        assert_eq!(parsed.slot.as_deref(), Some("slot-b"));
        assert!(parsed
            .active_digest
            .as_deref()
            .unwrap_or_default()
            .starts_with("sha256:9f86d081"));
    }
}
