use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use regex::Regex;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub fn is_binary(path: &Path) -> bool {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let binary_exts = [
        "o", "a", "so", "dylib", "dll", "exe", "bin", "lock",
        "pyc", "pyd", "wasm", "png", "jpg", "jpeg", "gif", "ico",
        "pdf", "zip", "tar", "gz", "bz2", "xz", "7z", "rlib", "d"
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
    let units = ["B", "K", "M", "G", "T"];
    let mut b = bytes as f64;
    for u in &units {
        if b < 1024.0 {
            if *u == "B" {
                return format!("[{:>4}B]", b as u64);
            }
            if b < 10.0 {
                return format!("[{:>4.1}{}]", b, u);
            }
            return format!("[{:>4}{}]", b as u64, u);
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
    if let Ok(f) = File::open(path) {
        let reader = BufReader::new(f);
        for line in reader.lines().take(15).flatten() {
            let s = line.trim();
            if s.is_empty() || s.starts_with("#!") {
                continue;
            }

            // Rust: строго //, ///, //!, игнорируя атрибуты #[cfg(...)]
            if ext == "rs" {
                if let Some(clean) = s.strip_prefix("//!").or_else(|| s.strip_prefix("///")) {
                    let c = clean.trim();
                    if !c.is_empty() { return Some(c.chars().take(80).collect()); }
                }
                if let Some(clean) = s.strip_prefix("//") {
                    let c = clean.trim();
                    if !c.is_empty() { return Some(c.chars().take(80).collect()); }
                }
                continue;
            }

            if s.starts_with("\"\"\"") || s.starts_with("'''") {
                let clean = s.trim_matches(|c| c == '"' || c == '\'').trim();
                if !clean.is_empty() { return Some(clean.chars().take(80).collect()); }
            }
            if ext == "md" && s.starts_with('#') {
                return Some(s.trim_start_matches('#').trim().chars().take(80).collect());
            }
            for prefix in &["//", "#", "<!--", "--"] {
                if s.starts_with(prefix) {
                    let clean = s[prefix.len()..].trim().trim_end_matches("-->").trim();
                    if !clean.is_empty() { return Some(clean.chars().take(80).collect()); }
                }
            }
        }
    }
    None
}

pub fn dump_dir(
    root: &Path, 
    curr: &Path, 
    prefix: &str, 
    level: usize, 
    max_level: Option<usize>, 
    show_sizes: bool, 
    dirs_only: bool,
    ignore_re: Option<&Regex>,
    out: &mut dyn Write
) {
    if let Some(max) = max_level {
        if level > max { return; }
    }

    let mut entries: Vec<PathBuf> = match fs::read_dir(curr) {
        Ok(read) => read.flatten().map(|e| e.path()).collect(),
        Err(_) => return,
    };

    // Регистронезависимая сортировка (полный паритет с Python и tree)
    entries.sort_by(|a, b| {
        let a_dir = a.is_dir();
        let b_dir = b.is_dir();
        if a_dir != b_dir {
            b_dir.cmp(&a_dir)
        } else {
            let a_name = a.file_name().unwrap_or_default().to_string_lossy();
            let b_name = b.file_name().unwrap_or_default().to_string_lossy();
            let a_lower = a_name.to_lowercase();
            let b_lower = b_name.to_lowercase();
            if a_lower != b_lower {
                a_lower.cmp(&b_lower)
            } else {
                a_name.cmp(&b_name)
            }
        }
    });

    let default_ignores = [".git", "target", "build", "dist", "node_modules", ".venv", "__pycache__", ".pytest_cache"];

    let valid: Vec<PathBuf> = entries.into_iter().filter(|p| {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if default_ignores.contains(&name) { return false; }
        if let Some(re) = ignore_re {
            if re.is_match(name) { return false; }
        }
        if dirs_only && !p.is_dir() { return false; }
        true
    }).collect();

    let count = valid.len();
    for (i, p) in valid.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let next_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
        let name = p.file_name().unwrap().to_string_lossy();

        let size_tag = if show_sizes {
            let meta = p.symlink_metadata().ok();
            let len = meta.map(|m| m.len()).unwrap_or(0);
            format!("{}  ", format_size(len))
        } else {
            String::new()
        };

        if p.is_symlink() {
            let target = fs::read_link(p).unwrap_or_default();
            let _ = writeln!(out, "{}{}{}{} -> {}", prefix, connector, size_tag, name, target.display());
        } else if p.is_dir() {
            let _ = writeln!(out, "{}{}{}{}/", prefix, connector, size_tag, name);
            dump_dir(root, p, &next_prefix, level + 1, max_level, show_sizes, dirs_only, ignore_re, out);
        } else {
            #[cfg(unix)]
            let is_exec = p.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false);
            #[cfg(not(unix))]
            let is_exec = false;

            let star = if is_exec { "*" } else { "" };
            let comment = extract_comment(p).map(|c| format!(" # {}", c)).unwrap_or_default();
            let _ = writeln!(out, "{}{}{}{}{}{}", prefix, connector, size_tag, name, star, comment);
        }
    }
}
