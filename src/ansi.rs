use serde::{Deserialize, Serialize};

/// ANSI SGR color representation supporting 16-color standard/bright palettes,
/// 8-bit 256-color indexed palettes (`38;5;n` / `48;5;n`), and 24-bit TrueColor (`38;2;r;g;b` / `48;2;r;g;b`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnsiColor {
    /// Standard (`0..=7`) and bright (`8..=15`) 16-color palette mapped to CSS classes.
    Named(u8),
    /// 8-bit 256-color palette index (`0..=255`).
    Indexed256(u8),
    /// 24-bit RGB color (`r, g, b`).
    TrueColor(u8, u8, u8),
}

impl AnsiColor {
    /// Converts a 256-color index or TrueColor value into an `(r, g, b)` tuple for inline styling
    /// when not using a named 16-color CSS class.
    pub fn to_rgb(self) -> Option<(u8, u8, u8)> {
        match self {
            AnsiColor::Named(_) => None,
            AnsiColor::TrueColor(r, g, b) => Some((r, g, b)),
            AnsiColor::Indexed256(idx) => match idx {
                0..=15 => None,
                16..=231 => {
                    let c = idx - 16;
                    let r_idx = c / 36;
                    let g_idx = (c % 36) / 6;
                    let b_idx = c % 6;
                    let scale = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                    Some((scale(r_idx), scale(g_idx), scale(b_idx)))
                }
                232..=255 => {
                    let gray = 8 + (idx - 232) * 10;
                    Some((gray, gray, gray))
                }
            },
        }
    }

    /// Returns the `0..=15` named index if this color maps to the 16-color CSS palette.
    pub fn named_index(self) -> Option<u8> {
        match self {
            AnsiColor::Named(idx) if idx < 16 => Some(idx),
            AnsiColor::Indexed256(idx) if idx < 16 => Some(idx),
            _ => None,
        }
    }
}

/// Active ANSI SGR typography and color attributes for a text span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AnsiStyle {
    pub fg: Option<AnsiColor>,
    pub bg: Option<AnsiColor>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl AnsiStyle {
    pub fn is_empty(&self) -> bool {
        self.fg.is_none()
            && self.bg.is_none()
            && !self.bold
            && !self.dim
            && !self.italic
            && !self.underline
            && !self.strikethrough
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Builds the space-separated CSS class string for this style.
    pub fn css_class(&self) -> String {
        let mut classes: Vec<String> = Vec::new();
        if self.bold {
            classes.push("ansi-bold".to_string());
        }
        if self.dim {
            classes.push("ansi-dim".to_string());
        }
        if self.italic {
            classes.push("ansi-italic".to_string());
        }
        if self.underline {
            classes.push("ansi-underline".to_string());
        }
        if self.strikethrough {
            classes.push("ansi-strike".to_string());
        }
        if let Some(fg) = self.fg.and_then(AnsiColor::named_index) {
            classes.push(format!("ansi-fg-{}", fg));
        }
        if let Some(bg) = self.bg.and_then(AnsiColor::named_index) {
            classes.push(format!("ansi-bg-{}", bg));
        }
        classes.join(" ")
    }

    /// Builds the optional inline CSS style string for 256-color (`16..=255`) or 24-bit TrueColor values.
    pub fn inline_style(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if let Some((r, g, b)) = self.fg.and_then(AnsiColor::to_rgb) {
            parts.push(format!("color: rgb({}, {}, {});", r, g, b));
        }
        if let Some((r, g, b)) = self.bg.and_then(AnsiColor::to_rgb) {
            parts.push(format!("background-color: rgb({}, {}, {});", r, g, b));
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    }

    /// Applies a slice of numeric SGR parameters (`\x1b[<params>m` or bare `[<params>m`).
    pub fn apply_sgr_params(&mut self, params: &[u16]) {
        if params.is_empty() {
            self.reset();
            return;
        }

        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => self.reset(),
                1 => {
                    self.bold = true;
                    self.dim = false;
                }
                2 => {
                    self.dim = true;
                    self.bold = false;
                }
                3 => self.italic = true,
                4 => self.underline = true,
                9 => self.strikethrough = true,
                21 | 22 => {
                    self.bold = false;
                    self.dim = false;
                }
                23 => self.italic = false,
                24 => self.underline = false,
                29 => self.strikethrough = false,
                code @ 30..=37 => self.fg = Some(AnsiColor::Named((code - 30) as u8)),
                39 => self.fg = None,
                code @ 40..=47 => self.bg = Some(AnsiColor::Named((code - 40) as u8)),
                49 => self.bg = None,
                code @ 90..=97 => self.fg = Some(AnsiColor::Named((code - 90 + 8) as u8)),
                code @ 100..=107 => self.bg = Some(AnsiColor::Named((code - 100 + 8) as u8)),
                38 | 48 => {
                    let is_fg = params[i] == 38;
                    if i + 2 < params.len() && params[i + 1] == 5 {
                        let color = AnsiColor::Indexed256((params[i + 2].min(255)) as u8);
                        if is_fg {
                            self.fg = Some(color);
                        } else {
                            self.bg = Some(color);
                        }
                        i += 3;
                        continue;
                    } else if i + 4 < params.len() && params[i + 1] == 2 {
                        let r = (params[i + 2].min(255)) as u8;
                        let g = (params[i + 3].min(255)) as u8;
                        let b = (params[i + 4].min(255)) as u8;
                        let color = AnsiColor::TrueColor(r, g, b);
                        if is_fg {
                            self.fg = Some(color);
                        } else {
                            self.bg = Some(color);
                        }
                        i += 5;
                        continue;
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

/// A contiguous text segment with uniform `AnsiStyle`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyledSpan {
    pub text: String,
    pub style: AnsiStyle,
}

/// Structured prefix extracted from distributed runner log lines:
/// `<RFC3339-timestamp> [<node_id>] [<stage_id>] [stdout|stderr] <payload>`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLinePrefix {
    pub timestamp: String,
    pub node_id: String,
    pub stage_id: String,
    pub stream_tag: Option<String>,
}

/// Parsed representation of a single console log line with optional structured prefix and styled SGR spans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedConsoleLine {
    pub prefix: Option<LogLinePrefix>,
    pub spans: Vec<StyledSpan>,
}

/// Validates and parses an SGR parameter string (inside `\x1b[<inner>m` or bare `[<inner>m`).
/// Returns `Some(params)` only if every semicolon-separated token is a valid SGR parameter sequence.
fn parse_sgr_param_list(inner: &str) -> Option<Vec<u16>> {
    if inner.is_empty() {
        return Some(vec![0]);
    }
    if !inner.bytes().all(|b| b.is_ascii_digit() || b == b';') {
        return None;
    }

    let mut params = Vec::new();
    for part in inner.split(';') {
        if part.is_empty() {
            params.push(0);
        } else {
            let val = part.parse::<u16>().ok()?;
            params.push(val);
        }
    }

    // Validate that all codes in `params` belong to the canonical SGR grammar
    let mut i = 0;
    while i < params.len() {
        match params[i] {
            0..=9 | 21..=29 | 30..=37 | 39 | 40..=47 | 49 | 90..=97 | 100..=107 => {
                i += 1;
            }
            38 | 48 => {
                if i + 2 < params.len() && params[i + 1] == 5 && params[i + 2] <= 255 {
                    i += 3;
                } else if i + 4 < params.len()
                    && params[i + 1] == 2
                    && params[i + 2] <= 255
                    && params[i + 3] <= 255
                    && params[i + 4] <= 255
                {
                    i += 5;
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    Some(params)
}

/// Returns `true` if `params` contains an explicit color code (`30..=39`, `40..=49`, `90..=97`, `100..=107`)
/// or an explicit reset (`0`).
fn has_explicit_color_or_reset(params: &[u16]) -> bool {
    params.iter().any(|&c| {
        c == 0
            || (30..=39).contains(&c)
            || (40..=49).contains(&c)
            || (90..=97).contains(&c)
            || (100..=107).contains(&c)
    })
}

/// Attempts to match a bare SGR bracket sequence `[<params>m` at the start of `s` (where `\x1b` was stripped).
/// Disambiguates against ordinary bracketed text (`[stderr]`, `[1/5]`, `[unoptimized + debuginfo]`) by requiring:
/// 1. `<params>` is non-empty and strictly valid SGR codes followed immediately by `m`, AND
/// 2. Either `active_style` is currently non-empty, OR `params` contains an explicit color/reset code,
///    OR it is immediately followed by another `[<params>m` or `\x1b[` SGR sequence (e.g. `[1m[33m`, `[1m[92m`).
fn match_bare_sgr_bracket(s: &str, style_is_active: bool) -> Option<(usize, Vec<u16>)> {
    if !s.starts_with('[') {
        return None;
    }
    let rest = &s[1..];
    let m_pos = rest.find('m')?;
    // Bare SGR sequences are compact (e.g. `[1m`, `[33m`, `[38;2;255;128;0m`)
    if m_pos == 0 || m_pos > 20 {
        return None;
    }
    let inner = &rest[..m_pos];
    let params = parse_sgr_param_list(inner)?;
    let consumed = 1 + m_pos + 1;

    let next_slice = &s[consumed..];
    let chained_next_sgr = next_slice.starts_with("\x1b[")
        || next_slice.starts_with('\u{009b}')
        || ({
            if let Some(after_bracket) = next_slice.strip_prefix('[') {
                if let Some(next_m) = after_bracket.find('m') {
                    next_m > 0
                        && next_m <= 20
                        && parse_sgr_param_list(&after_bracket[..next_m]).is_some()
                } else {
                    false
                }
            } else {
                false
            }
        });

    if style_is_active || has_explicit_color_or_reset(&params) || chained_next_sgr {
        Some((consumed, params))
    } else {
        None
    }
}

/// Attempts to match a standard `ESC`-prefixed (`\x1b[`) or C1 (`\u{009b}`) CSI sequence at the start of `s`.
/// Returns `Some((bytes_consumed, Option<sgr_params>))` where `Option<Vec<u16>>` is `Some` for SGR (`m`)
/// and `None` for non-SGR CSI control sequences (like `\x1b[2K` or `\x1b[?25h`) so they are cleanly stripped.
fn match_esc_csi(s: &str) -> Option<(usize, Option<Vec<u16>>)> {
    let (prefix_len, rest) = if let Some(r) = s.strip_prefix("\x1b[") {
        (2, r)
    } else {
        let r = s.strip_prefix('\u{009b}')?;
        ('\u{009b}'.len_utf8(), r)
    };

    // Scan CSI parameter and intermediate bytes until a final byte in 0x40..=0x7E
    for (idx, b) in rest.bytes().enumerate() {
        if (0x40..=0x7E).contains(&b) {
            let consumed = prefix_len + idx + 1;
            if b == b'm' {
                let inner = &rest[..idx];
                let params = parse_sgr_param_list(inner).unwrap_or_else(|| vec![0]);
                return Some((consumed, Some(params)));
            } else {
                return Some((consumed, None));
            }
        }
        // If an unexpected non-CSI byte is encountered, abort CSI match
        if !(0x20..=0x3F).contains(&b) {
            break;
        }
    }
    None
}

/// Parses a single line or string into a sequence of `StyledSpan`s, resolving both standard `\x1b[...m`
/// CSI sequences and stripped-ESC bare SGR bracket sequences (`[1m[33mwarning[0m`).
pub fn parse_ansi_spans(input: &str) -> Vec<StyledSpan> {
    let mut spans: Vec<StyledSpan> = Vec::new();
    let mut current_style = AnsiStyle::default();
    let mut current_text = String::new();
    let mut i = 0;

    let flush = |spans: &mut Vec<StyledSpan>, text: &mut String, style: AnsiStyle| {
        if !text.is_empty() {
            let taken = std::mem::take(text);
            if let Some(last) = spans.last_mut() {
                if last.style == style {
                    last.text.push_str(&taken);
                    return;
                }
            }
            spans.push(StyledSpan { text: taken, style });
        }
    };

    while i < input.len() {
        let slice = &input[i..];

        if let Some((consumed, maybe_sgr)) = match_esc_csi(slice) {
            flush(&mut spans, &mut current_text, current_style);
            if let Some(params) = maybe_sgr {
                current_style.apply_sgr_params(&params);
            }
            i += consumed;
            continue;
        }

        if let Some((consumed, params)) = match_bare_sgr_bracket(slice, !current_style.is_empty()) {
            flush(&mut spans, &mut current_text, current_style);
            current_style.apply_sgr_params(&params);
            i += consumed;
            continue;
        }

        // Append next UTF-8 character
        if let Some(ch) = slice.chars().next() {
            // Skip stray raw ESC (0x1B) if not part of a valid CSI sequence
            if ch != '\x1b' {
                current_text.push(ch);
            }
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    flush(&mut spans, &mut current_text, current_style);
    spans
}

/// Strips all ANSI CSI escape sequences and bare SGR bracket sequences from `input`,
/// returning clean plain text suitable for search filtering, failure detection, and `Copy Clean Log`.
pub fn strip_ansi_codes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for (idx, line) in input.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        for span in parse_ansi_spans(line) {
            out.push_str(&span.text);
        }
    }
    out
}

/// Splits a console log line into its optional distributed runner prefix
/// (`<RFC3339-timestamp> [<node_id>] [<stage_id>] [stdout|stderr] `) and styled SGR payload spans.
pub fn parse_console_line(line: &str) -> ParsedConsoleLine {
    let parts: Vec<&str> = line.splitn(5, ' ').collect();
    if parts.len() >= 4
        && (parts[0].contains('T') && (parts[0].contains('+') || parts[0].ends_with('Z')))
        && parts[1].starts_with('[')
        && parts[1].ends_with(']')
        && parts[2].starts_with('[')
        && parts[2].ends_with(']')
    {
        let timestamp = parts[0].to_string();
        let node_id = parts[1].to_string();
        let stage_id = parts[2].to_string();

        if parts.len() == 5 && (parts[3] == "[stdout]" || parts[3] == "[stderr]") {
            let stream_tag = Some(parts[3].to_string());
            let spans = parse_ansi_spans(parts[4]);
            return ParsedConsoleLine {
                prefix: Some(LogLinePrefix {
                    timestamp,
                    node_id,
                    stage_id,
                    stream_tag,
                }),
                spans,
            };
        } else {
            let remainder = line.splitn(4, ' ').nth(3).unwrap_or_default();
            let spans = parse_ansi_spans(remainder);
            return ParsedConsoleLine {
                prefix: Some(LogLinePrefix {
                    timestamp,
                    node_id,
                    stage_id,
                    stream_tag: None,
                }),
                spans,
            };
        }
    }

    ParsedConsoleLine {
        prefix: None,
        spans: parse_ansi_spans(line),
    }
}

fn html_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Renders a multi-line console log string into deterministic, XSS-escaped HTML
/// with styled prefix badges and ANSI SGR `<span class="...">` elements.
pub fn render_console_log_html(raw: &str) -> String {
    let mut html = String::new();
    for (idx, line) in raw.lines().enumerate() {
        if idx > 0 {
            html.push('\n');
        }
        let parsed = parse_console_line(line);
        html.push_str(r#"<span class="log-line">"#);
        if let Some(pref) = parsed.prefix {
            html.push_str(&format!(
                r#"<span class="log-prefix-ts">{}</span> <span class="log-prefix-node">{}</span> <span class="log-prefix-stage">{}</span> "#,
                html_escape(&pref.timestamp),
                html_escape(&pref.node_id),
                html_escape(&pref.stage_id)
            ));
            if let Some(stream) = pref.stream_tag {
                let stream_cls = if stream == "[stderr]" {
                    "log-prefix-stderr"
                } else {
                    "log-prefix-stdout"
                };
                html.push_str(&format!(
                    r#"<span class="{}">{}</span> "#,
                    stream_cls,
                    html_escape(&stream)
                ));
            }
        }
        for span in parsed.spans {
            let escaped_text = html_escape(&span.text);
            if span.style.is_empty() {
                html.push_str(&escaped_text);
            } else {
                let cls = span.style.css_class();
                let inline = span.style.inline_style();
                match (cls.is_empty(), inline) {
                    (false, Some(st)) => {
                        html.push_str(&format!(
                            r#"<span class="{}" style="{}">{}</span>"#,
                            cls, st, escaped_text
                        ));
                    }
                    (false, None) => {
                        html.push_str(&format!(r#"<span class="{}">{}</span>"#, cls, escaped_text));
                    }
                    (true, Some(st)) => {
                        html.push_str(&format!(r#"<span style="{}">{}</span>"#, st, escaped_text));
                    }
                    (true, None) => {
                        html.push_str(&escaped_text);
                    }
                }
            }
        }
        html.push_str("</span>");
    }
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bare_sgr_and_distributed_prefix_examples() {
        let sample = concat!(
            "2026-11-01T09:21:02.585185+00:00 [worker-riscv64] [native-k3] [stderr] [1m[33mwarning[0m: `action-runner` (test \"challenge_m3_worker_gating_test\") generated 1 warning (1 duplicate)\n",
            "2026-11-01T09:21:02.585189+00:00 [worker-riscv64] [native-k3] [stderr] [1m[92m    Finished[0m `test` profile [unoptimized + debuginfo] target(s) in 3m 44s\n",
            "2026-11-01T09:21:02.609555+00:00 [worker-riscv64] [native-k3] [stderr] [1m[92m     Running[0m unittests src/lib.rs (target/llvm-cov-target/debug/deps/action_runner-79cd41c4e20b3103)"
        );

        let stripped = strip_ansi_codes(sample);
        assert!(!stripped.contains("[1m"));
        assert!(!stripped.contains("[33m"));
        assert!(!stripped.contains("[92m"));
        assert!(!stripped.contains("[0m"));
        // Verify legitimate brackets are preserved verbatim
        assert!(stripped.contains("[worker-riscv64] [native-k3] [stderr] warning:"));
        assert!(stripped.contains("[unoptimized + debuginfo]"));

        let html = render_console_log_html(sample);
        assert!(html.contains(r#"<span class="log-prefix-node">[worker-riscv64]</span>"#));
        assert!(html.contains(r#"<span class="log-prefix-stage">[native-k3]</span>"#));
        assert!(html.contains(r#"<span class="log-prefix-stderr">[stderr]</span>"#));
        assert!(html.contains(r#"<span class="ansi-bold ansi-fg-3">warning</span>"#));
        assert!(html.contains(r#"<span class="ansi-bold ansi-fg-10">    Finished</span>"#));
        assert!(html.contains(r#"<span class="ansi-bold ansi-fg-10">     Running</span>"#));
        assert!(html.contains("[unoptimized + debuginfo]"));
    }

    #[test]
    fn test_standard_esc_256_and_truecolor_sequences() {
        let line = "\x1b[1;31merror\x1b[0m: \x1b[38;5;208mcustom256\x1b[0m and \x1b[4;38;2;64;128;255mtruecolor\x1b[0m";
        let spans = parse_ansi_spans(line);
        assert_eq!(spans.len(), 5);
        assert_eq!(spans[0].text, "error");
        assert!(spans[0].style.bold);
        assert_eq!(spans[0].style.fg, Some(AnsiColor::Named(1)));

        assert_eq!(spans[2].text, "custom256");
        assert_eq!(spans[2].style.fg, Some(AnsiColor::Indexed256(208)));
        assert_eq!(
            spans[2].style.inline_style().as_deref(),
            Some("color: rgb(255, 135, 0);")
        );

        assert_eq!(spans[4].text, "truecolor");
        assert!(spans[4].style.underline);
        assert_eq!(spans[4].style.fg, Some(AnsiColor::TrueColor(64, 128, 255)));

        let stripped = strip_ansi_codes(line);
        assert_eq!(stripped, "error: custom256 and truecolor");
    }

    #[test]
    fn test_xss_escaping_and_ordinary_brackets_preserved() {
        let line = "Step [1/5]: array [u8; 32] <script>alert('xss')</script> [1m[32mok[0m";
        let html = render_console_log_html(line);
        assert!(html.contains(
            "Step [1/5]: array [u8; 32] &lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;"
        ));
        assert!(html.contains(r#"<span class="ansi-bold ansi-fg-2">ok</span>"#));
    }
}
