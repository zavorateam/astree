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
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some(idx) = rest.find(']') {
            return rest[idx + 1..].trim_start();
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

    text.lines()
        .filter(|line| {
            let stripped = line.trim();
            !stripped.is_empty()
                && (line.contains('│')
                    || line.contains('├')
                    || line.contains('└')
                    || line.contains('─')
                    || stripped.ends_with('/'))
        })
        .map(|s| s.to_string())
        .collect()
}

pub fn parse_line(raw_line: &str) -> Option<ParsedNode> {
    let line = raw_line.trim_end();
    if line.is_empty() {
        return None;
    }

    let tree_idx = line
        .find(|c| c != '│' && c != '├' && c != '└' && c != '─' && c != ' ')
        .unwrap_or(line.len());
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

    let (body_no_exec, is_exec) = match body.strip_suffix('*') {
        Some(stripped) => (stripped, true),
        None => (body, false),
    };

    let (final_body, is_dir) = match body_no_exec.strip_suffix('/') {
        Some(stripped) => (stripped, true),
        None => (body_no_exec, false),
    };

    Some(ParsedNode {
        depth,
        name: final_body.to_string(),
        is_dir,
        is_exec,
        symlink_target,
        comment,
        inline_code,
    })
}
