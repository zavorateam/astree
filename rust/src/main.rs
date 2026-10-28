use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};

const VERSION: &str = env!("CARGO_PKG_VERSION");

// --- ПРОВЕРКА НА БИНАРНОСТЬ ---
fn is_binary(path: &Path) -> bool {
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

fn format_size(bytes: u64) -> String {
    let units = ["B", "K", "M", "G", "T"];
    let mut b = bytes as f64;
    for u in &units {
        if b < 1024.0 {
            if *u == "B" {
                return format!("[{:>4}B]", b as u64);
            }
            let s = format!("{:>4.1}{}", b, u);
            return format!("[{}]", s.replace(".0", " "));
        }
        b /= 1024.0;
    }
    format!("[{:>4.1}P]", b)
}

fn extract_comment(path: &Path) -> Option<String> {
    if is_binary(path) {
        return None;
    }
    if let Ok(f) = File::open(path) {
        let reader = BufReader::new(f);
        for line in reader.lines().take(15).flatten() {
            let s = line.trim();
            if s.is_empty() || s.starts_with("#!") {
                continue;
            }
            if s.starts_with("\"\"\"") || s.starts_with("'''") {
                let clean = s.trim_matches(|c| c == '"' || c == '\'').trim();
                if !clean.is_empty() { return Some(clean.chars().take(80).collect()); }
            }
            if path.extension().and_then(|e| e.to_str()) == Some("md") && s.starts_with('#') {
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

// --- DUMP ---
fn dump_dir(
    root: &Path, 
    curr: &Path, 
    prefix: &str, 
    level: usize, 
    max_level: Option<usize>, 
    show_sizes: bool, 
    dirs_only: bool,
    ignore_pat: Option<&str>
) {
    if let Some(max) = max_level {
        if level > max { return; }
    }

    let mut entries: Vec<PathBuf> = match fs::read_dir(curr) {
        Ok(read) => read.flatten().map(|e| e.path()).collect(),
        Err(_) => return,
    };

    entries.sort_by(|a, b| {
        let a_dir = a.is_dir();
        let b_dir = b.is_dir();
        if a_dir != b_dir {
            b_dir.cmp(&a_dir)
        } else {
            a.file_name().cmp(&b.file_name())
        }
    });

    let default_ignores = [".git", "target", "build", "node_modules", ".venv", "__pycache__", ".pytest_cache"];

    let valid: Vec<PathBuf> = entries.into_iter().filter(|p| {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if default_ignores.contains(&name) { return false; }
        if let Some(pat) = ignore_pat {
            if name.contains(pat) { return false; }
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
            println!("{}{}{}{} -> {}", prefix, connector, size_tag, name, target.display());
        } else if p.is_dir() {
            println!("{}{}{}{}/", prefix, connector, size_tag, name);
            dump_dir(root, p, &next_prefix, level + 1, max_level, show_sizes, dirs_only, ignore_pat);
        } else {
            let is_exec = p.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false);
            let star = if is_exec { "*" } else { "" };
            let comment = extract_comment(p).map(|c| format!(" # {}", c)).unwrap_or_default();
            println!("{}{}{}{}{}{}", prefix, connector, size_tag, name, star, comment);
        }
    }
}

// --- SCAFFOLD (РАЗВОРАЧИВАНИЕ) ---
fn strip_size_prefix(s: &str) -> &str {
    let trimmed = s.trim_start();
    if trimmed.starts_with('[') {
        if let Some(idx) = trimmed.find(']') {
            return trimmed[idx + 1..].trim_start();
        }
    }
    s
}

fn scaffold(input: &str, dry_run: bool, force: bool) {
    let mut stack: Vec<String> = Vec::new();
    let mut root_name = String::from("project");

    for raw in input.lines() {
        let line = raw.trim_end();
        if line.is_empty() { continue; }

        let tree_idx = line.find(|c| c != '│' && c != '├' && c != '└' && c != '─' && c != ' ').unwrap_or(line.len());
        let prefix = &line[..tree_idx];
        let mut body = &line[tree_idx..];

        let depth = prefix.chars().count() / 4;
        stack.truncate(depth);

        body = strip_size_prefix(body);

        // Парсинг комментариев
        let mut comment = None;
        let mut inline_code = None;
        if let Some(pos) = body.find(" # ").or_else(|| body.find(" // ")) {
            let c = body[pos + 3..].trim();
            body = body[..pos].trim();
            if let Some(code) = c.strip_prefix('=') {
                inline_code = Some(code.trim().to_string());
            } else {
                comment = Some(c.to_string());
            }
        }

        // Симлинки
        let mut symlink_target = None;
        if let Some((src, dst)) = body.split_once("->") {
            body = src.trim();
            symlink_target = Some(dst.trim().to_string());
        }

        let is_exec = body.ends_with('*');
        if is_exec { body = &body[..body.len() - 1]; }

        let current_path = if stack.is_empty() {
            PathBuf::from(body)
        } else {
            let mut p = PathBuf::from(&stack[0]);
            for part in &stack[1..] { p.push(part); }
            p.push(body);
            p
        };

        if body.ends_with('/') {
            let dir_clean = body.trim_end_matches('/');
            stack.push(dir_clean.to_string());
            if depth == 0 { root_name = dir_clean.to_string(); }
            if !dry_run { fs::create_dir_all(&current_path).ok(); }
            println!("[DIR]  {}", current_path.display());
            continue;
        }

        if let Some(target) = symlink_target {
            if !dry_run {
                if let Some(parent) = current_path.parent() { fs::create_dir_all(parent).ok(); }
                if current_path.exists() && force { fs::remove_file(&current_path).ok(); }
                #[cfg(unix)]
                symlink(&target, &current_path).ok();
            }
            println!("[LINK] {} -> {}", current_path.display(), target);
            continue;
        }

        // Генерация файла
        let ext = current_path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let file_name = current_path.file_name().and_then(|s| s.to_str()).unwrap_or("");

        let content = if let Some(code) = inline_code {
            format!("{}\n", code.replace("\\n", "\n"))
        } else if file_name == "pyproject.toml" {
            format!("[project]\nname = \"{}\"\nversion = \"0.1.0\"\nreadme = \"README.md\"\n", root_name)
        } else if file_name == "README.md" && comment.is_some() {
            format!("# {}\n\n> {}\n", root_name, comment.as_ref().unwrap())
        } else {
            let mut out = String::new();
            if is_exec || ext == "sh" || ext == "bash" {
                if ext == "py" { out.push_str("#!/usr/bin/env python3\n"); }
                else if ext == "sh" || ext == "bash" { out.push_str("#!/usr/bin/env bash\n"); }
            }
            if let Some(c) = comment {
                if ext == "py" { out.push_str(&format!("\"\"\"{}\"\"\"\n", c)); }
                else if ext == "rs" { out.push_str(&format!("//! {}\n", c)); }
                else { out.push_str(&format!("# {}\n", c)); }
            }
            out
        };

        if !dry_run {
            if let Some(parent) = current_path.parent() { fs::create_dir_all(parent).ok(); }
            let mut f = File::create(&current_path).unwrap();
            f.write_all(content.as_bytes()).ok();

            #[cfg(unix)]
            if is_exec || ext == "sh" || ext == "bash" {
                let mut perms = f.metadata().unwrap().permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&current_path, perms).ok();
            }
        }
        println!("[FILE] {}{}", current_path.display(), if is_exec { " (+x)" } else { "" });
    }
}

// --- CLI ---
fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "dump" {
        let mut dir = PathBuf::from(".");
        let mut show_sizes = false;
        let mut dirs_only = false;
        let mut max_level = None;
        let mut ignore_pat = None;

        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--human-readable" => show_sizes = true,
                "-d" | "--dirs-only" => dirs_only = true,
                "-L" => {
                    i += 1;
                    if i < args.len() { max_level = args[i].parse().ok(); }
                }
                "-I" => {
                    i += 1;
                    if i < args.len() { ignore_pat = Some(args[i].as_str()); }
                }
                "--help" => {
                    println!("Usage: astree dump [DIR] [-L level] [-I pattern] [-h] [-d]");
                    return;
                }
                other => {
                    if !other.starts_with('-') { dir = PathBuf::from(other); }
                }
            }
            i += 1;
        }

        let root = dir.canonicalize().unwrap_or(dir);
        println!("{}/", root.file_name().unwrap_or_default().to_string_lossy());
        dump_dir(&root, &root, "", 1, max_level, show_sizes, dirs_only, ignore_pat);
        return;
    }

    if args.len() > 1 && (args[1] == "--version" || args[1] == "-V") {
        println!("astree v{}", VERSION);
        return;
    }

    if args.len() > 1 && (args[1] == "--help") {
        println!("astree - smart tree to repo generator & dumper\n");
        println!("Usage:\n  astree [FILE]          Развернуть дерево (или из stdin)");
        println!("  astree dump [DIR]      Сгенерировать дерево из папки\n");
        println!("Options:\n  -h, --human-readable   Показать размеры (в режиме dump)");
        println!("  -L <level>             Глубина обхода");
        println!("  -d                     Только папки");
        return;
    }

    let mut input = String::new();
    if args.len() > 1 && args[1] != "-" && !args[1].starts_with('-') {
        input = fs::read_to_string(&args[1]).expect("Не удалось прочитать файл");
    } else {
        io::stdin().read_to_string(&mut input).expect("Ошибка чтения stdin");
    }

    scaffold(&input, false, false);
}
