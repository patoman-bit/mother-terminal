use std::collections::HashSet;
use std::fmt;
use std::process::Command;

#[derive(Clone, Copy, Debug, Default)]
pub struct ToolStatus {
    pub lynx: bool,
    pub curl: bool,
}

impl ToolStatus {
    pub fn ready(&self) -> bool {
        self.lynx
    }
}

#[derive(Debug)]
pub enum SearchError {
    ToolMissing(&'static str),
    CommandFailed(String),
    Utf8Error,
    InvalidInput(&'static str),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::ToolMissing(tool) => write!(f, "{} not available", tool),
            SearchError::CommandFailed(msg) => write!(f, "search command failed: {}", msg),
            SearchError::Utf8Error => write!(f, "output was not valid UTF-8"),
            SearchError::InvalidInput(msg) => write!(f, "{}", msg),
        }
    }
}

pub struct SearchResult {
    pub raw_text: String,
    pub candidates: Vec<String>,
}

pub fn probe_tools() -> ToolStatus {
    ToolStatus {
        lynx: check_binary("lynx"),
        curl: check_binary("curl"),
    }
}

pub fn run_search(url: &str) -> Result<SearchResult, SearchError> {
    if url.trim().is_empty() {
        return Err(SearchError::InvalidInput("url missing"));
    }

    let status = probe_tools();
    if !status.lynx {
        return Err(SearchError::ToolMissing("lynx"));
    }

    let output = Command::new("lynx")
        .args(["-dump", url])
        .output()
        .map_err(|e| SearchError::CommandFailed(e.to_string()))?;

    if !output.status.success() {
        return Err(SearchError::CommandFailed(format!(
            "exit code {}",
            output.status
        )));
    }

    let raw_text = String::from_utf8(output.stdout).map_err(|_| SearchError::Utf8Error)?;
    let candidates = extract_candidates(&raw_text);

    Ok(SearchResult {
        raw_text,
        candidates,
    })
}

pub fn extract_candidates(raw: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let normalized = normalize_line(trimmed);
        if normalized.is_empty() {
            continue;
        }

        if looks_like_url(&normalized) || looks_like_title(&normalized) {
            if seen.insert(normalized.clone()) {
                out.push(normalized);
            }
        }

        if out.len() >= 40 {
            break;
        }
    }

    out
}

fn check_binary(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn normalize_line(line: &str) -> String {
    let mut s = line.trim();
    if let Some(rest) = s.strip_prefix('[') {
        s = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        s = s.trim_start_matches(']');
    }
    s = s.trim_start_matches(|c: char| c == ':' || c == '-' || c == '.');
    s.trim().to_string()
}

fn looks_like_url(line: &str) -> bool {
    line.contains("http://") || line.contains("https://") || line.contains("www.")
}

fn looks_like_title(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 8 || trimmed.len() > 120 {
        return false;
    }
    if trimmed.split_whitespace().count() < 2 {
        return false;
    }

    let mut uppercase_words = 0;
    let mut lowercase_words = 0;

    for word in trimmed.split_whitespace() {
        if word.chars().any(|c| c.is_lowercase()) {
            lowercase_words += 1;
        } else {
            uppercase_words += 1;
        }
    }

    uppercase_words > 0 && uppercase_words >= lowercase_words
}
