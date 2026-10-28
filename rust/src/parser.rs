#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedNode {
    pub depth: usize,
    pub name: String,
    pub is_dir: bool,
    pub is_exec: bool,
    pub symlink_target: Option<String>,
    pub comment: Option<String>,
    pub inline_code: Option<String>,
}

pub fn strip_size_prefix(s: &str) -> &str {
    let trimmed = s.trim_start();
    if trimmed.starts_with('[') {
        if let Some(idx) = trimmed.find(']') {
            return trimmed[idx + 1..].trim_start();
        }
    }
    s
}

pub fn clean_input_lines(raw: &str) -> Vec<String> {
    let mut text = raw;
    if let Some(start) = raw.find("```") {
        let after_start = &raw[start + 3..];
        if let Some(newline) = after_start.find('\n') {
            let body = &after_start[newline + 1..];
            if let Some(end) = body.find("```") {
                text = &body[..end];
            }
        }
    }

    let mut lines = Vec::new();
    for line in text.lines() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        if line.contains('│') || line.contains('├') || line.contains('└') || line.contains('─') || stripped.ends_with('/') {
            lines.push(line.to_string());
        }
    }
    lines
}

pub fn parse_line(raw_line: &str) -> Option<ParsedNode> {
    let line = raw_line.trim_end();
    if line.is_empty() {
        return None;
    }

    let tree_idx = line.find(|c| c != '│' && c != '├' && c != '└' && c != '─' && c != ' ').unwrap_or(line.len());
    let prefix = &line[..tree_idx];
    let mut body = &line[tree_idx..];

    let depth = prefix.chars().count() / 4;
    body = strip_size_prefix(body);
    if body.is_empty() {
        return None;
    }

    let mut comment = None;
    let mut inline_code = None;
    if let Some(pos) = body.find(" # ").or_else(|| body.find(" // ")) {
        let c = body[pos + 3..].trim();
        body = body[..pos].trim();
        if let Some(code) = c.strip_prefix('=') {
            inline_code = Some(code.trim().replace("\\n", "\n"));
        } else {
            comment = Some(c.to_string());
        }
    }

    let mut symlink_target = None;
    if let Some((src, dst)) = body.split_once("->") {
        body = src.trim();
        symlink_target = Some(dst.trim().to_string());
    }

    let is_exec = body.ends_with('*');
    if is_exec {
        body = &body[..body.len() - 1];
    }

    let is_dir = body.ends_with('/');
    if is_dir {
        body = body.trim_end_matches('/');
    }

    Some(ParsedNode {
        depth,
        name: body.to_string(),
        is_dir,
        is_exec,
        symlink_target,
        comment,
        inline_code,
    })
}
