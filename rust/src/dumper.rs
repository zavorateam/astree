use regex::Regex;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub struct DumpOptions<'a> {
    pub max_level: Option<usize>,
    pub show_sizes: bool,
    pub dirs_only: bool,
    pub ignore_re: Option<&'a Regex>,
}

pub fn is_binary(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let binary_exts = [
        "o",
        "a",
        "so",
        "dylib",
        "dll",
        "exe",
        "bin",
        "lock",
        "pyc",
        "pyd",
        "wasm",
        "png",
        "jpg",
        "jpeg",
        "gif",
        "ico",
        "pdf",
        "zip",
        "tar",
        "gz",
        "bz2",
        "xz",
        "7z",
        "rlib",
        "d",
        "pdb",
        "timestamp",
    ];
    if binary_exts.contains(&ext.as_str()) {
        return true;
    }
    if let Ok(mut f) = File::open(path) {
        let mut buf = [0u8; 1024];
        if let Ok(n) = f.read(&mut buf) {
            if buf[..n].contains(&0) {
                return true;
            }
        }
    }
    false
}

pub fn format_size(bytes: u64) -> String {
    let units = ["B", "K", "M", "G", "T", "P"];
    let mut b = bytes as f64;
    for (i, u) in units.iter().enumerate() {
        if b < 1024.0 || i == units.len() - 1 {
            if i == 0 {
                return format!("[{:>4}B]", bytes);
            }
            if b < 9.95 {
                let rounded = (b * 10.0).round() / 10.0;
                return format!("[{:>4.1}{}]", rounded, u);
            }
            let rounded_int = (b + 0.5).floor() as u64;
            return format!("[{:>4}{}]", rounded_int, u);
        }
        b /= 1024.0;
    }
    format!("[{:>4.1}P]", b)
}

pub fn extract_comment(path: &Path) -> Option<String> {
    if is_binary(path) {
        return None;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let mut md_header: Option<String> = None;

    if let Ok(f) = File::open(path) {
        let reader = BufReader::new(f);
        for line in reader.lines().take(15).map_while(Result::ok) {
            let s = line.trim();
            if s.is_empty() || s.starts_with("#!") {
                continue;
            }

            if ext == "rs" {
                if let Some(clean) = s.strip_prefix("//!").or_else(|| s.strip_prefix("///")) {
                    let c = clean.trim();
                    if !c.is_empty() {
                        return Some(c.chars().take(80).collect());
                    }
                }
                if let Some(clean) = s.strip_prefix("//") {
                    let c = clean.trim();
                    if !c.is_empty() {
                        return Some(c.chars().take(80).collect());
                    }
                }
                continue;
            }

            if ext == "md" {
                if let Some(clean) = s.strip_prefix('>') {
                    let c = clean.trim();
                    if !c.is_empty() {
                        return Some(c.chars().take(80).collect());
                    }
                }
                if s.starts_with('#') && md_header.is_none() {
                    let clean = s.trim_start_matches('#').trim();
                    if !clean.is_empty() {
                        md_header = Some(clean.chars().take(80).collect());
                    }
                }
                continue;
            }

            if s.starts_with("\"\"\"") || s.starts_with("'''") {
                let clean = s.trim_matches(|c| c == '"' || c == '\'').trim();
                if !clean.is_empty() {
                    return Some(clean.chars().take(80).collect());
                }
            }
            for prefix in &["//", "#", "<!--", "--"] {
                if let Some(clean) = s.strip_prefix(prefix) {
                    let trimmed = clean.trim().trim_end_matches("-->").trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.chars().take(80).collect());
                    }
                }
            }
        }
    }
    md_header
}

pub struct GitIgnoreRule {
    pub is_negated: bool,
    pub is_dir_only: bool,
    pub has_slash: bool,
    pub regex: Regex,
}

fn glob_to_regex(pat: &str) -> Result<Regex, regex::Error> {
    let mut re_str = String::from("^");
    let mut chars = pat.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    if chars.peek() == Some(&'/') {
                        chars.next();
                        re_str.push_str("(?:.+/)?");
                    } else {
                        re_str.push_str(".*");
                    }
                } else {
                    re_str.push_str("[^/]*");
                }
            }
            '?' => re_str.push_str("[^/]"),
            '.' | '+' | '(' | ')' | '{' | '}' | '|' | '^' | '$' | '\\' | '[' | ']' => {
                re_str.push('\\');
                re_str.push(c);
            }
            _ => re_str.push(c),
        }
    }
    re_str.push('$');
    Regex::new(&re_str)
}

pub fn load_gitignore_rules(root: &Path) -> Vec<GitIgnoreRule> {
    let mut rules = Vec::new();
    let gi = root.join(".gitignore");
    if let Ok(file) = File::open(gi) {
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            let mut trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let mut is_negated = false;
            if let Some(rest) = trimmed.strip_prefix('!') {
                is_negated = true;
                trimmed = rest.trim();
            }
            if trimmed.is_empty() {
                continue;
            }
            let is_dir_only = trimmed.ends_with('/');
            let clean_pat = trimmed.trim_matches('/');
            let has_slash = clean_pat.contains('/');
            if let Ok(regex) = glob_to_regex(clean_pat) {
                rules.push(GitIgnoreRule {
                    is_negated,
                    is_dir_only,
                    has_slash,
                    regex,
                });
            }
        }
    }
    rules
}

pub fn is_path_ignored(name: &str, rel_path: &str, is_dir: bool, rules: &[GitIgnoreRule]) -> bool {
    let mut ignored = false;
    for rule in rules {
        if rule.is_dir_only && !is_dir {
            continue;
        }
        let matched = if rule.has_slash {
            rule.regex.is_match(rel_path)
        } else {
            rule.regex.is_match(name) || rule.regex.is_match(rel_path)
        };
        if matched {
            ignored = !rule.is_negated;
        }
    }
    ignored
}

pub fn dump_dir(
    root: &Path,
    curr: &Path,
    prefix: &str,
    level: usize,
    options: &DumpOptions,
    out: &mut dyn Write,
) -> io::Result<()> {
    if let Some(max) = options.max_level {
        if level > max {
            return Ok(());
        }
    }

    let mut entries: Vec<PathBuf> = fs::read_dir(curr)?
        .map_while(Result::ok)
        .map(|e| e.path())
        .collect();

    entries.sort_by(|a, b| {
        let a_dir = a.is_dir();
        let b_dir = b.is_dir();
        b_dir.cmp(&a_dir).then_with(|| {
            let a_name = a.file_name().unwrap_or_default().to_string_lossy();
            let b_name = b.file_name().unwrap_or_default().to_string_lossy();
            let a_lower = a_name.to_lowercase();
            let b_lower = b_name.to_lowercase();
            a_lower.cmp(&b_lower).then_with(|| a_name.cmp(&b_name))
        })
    });

    let default_ignores = [
        ".git",
        "target",
        "build",
        "dist",
        "node_modules",
        ".venv",
        "__pycache__",
        ".pytest_cache",
    ];
    let gi_rules = load_gitignore_rules(root);

    let valid: Vec<PathBuf> = entries
        .into_iter()
        .filter(|p| {
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if default_ignores.contains(&name) {
                return false;
            }
            if let Some(re) = options.ignore_re {
                if re.is_match(name) {
                    return false;
                }
            }
            let rel = p
                .strip_prefix(root)
                .ok()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if is_path_ignored(name, rel, p.is_dir(), &gi_rules) {
                return false;
            }
            if options.dirs_only && !p.is_dir() {
                return false;
            }
            true
        })
        .collect();

    let count = valid.len();
    for (i, p) in valid.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let next_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
        let name = p.file_name().unwrap().to_string_lossy();

        let size_tag = if options.show_sizes {
            let meta = p.symlink_metadata().ok();
            let len = meta.map(|m| m.len()).unwrap_or(0);
            format!("{}  ", format_size(len))
        } else {
            String::new()
        };

        if p.is_symlink() {
            let target = fs::read_link(p).unwrap_or_default();
            writeln!(
                out,
                "{}{}{}{} -> {}",
                prefix,
                connector,
                size_tag,
                name,
                target.display()
            )?;
        } else if p.is_dir() {
            writeln!(out, "{}{}{}{}/", prefix, connector, size_tag, name)?;
            dump_dir(root, p, &next_prefix, level + 1, options, out)?;
        } else {
            #[cfg(unix)]
            let is_exec = p
                .metadata()
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false);
            #[cfg(not(unix))]
            let is_exec = false;

            let star = if is_exec { "*" } else { "" };
            let comment = extract_comment(p)
                .map(|c| format!(" # {}", c))
                .unwrap_or_default();
            writeln!(
                out,
                "{}{}{}{}{}{}",
                prefix, connector, size_tag, name, star, comment
            )?;
        }
    }
    Ok(())
}
