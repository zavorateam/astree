use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};

use crate::parser::parse_line;

pub fn validate_safe_path(base_dir: &Path, rel_path: &str) -> io::Result<PathBuf> {
    let p = Path::new(rel_path);
    if p.is_absolute() {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Absolute path not allowed"));
    }
    for comp in p.components() {
        if matches!(comp, Component::ParentDir) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Parent directory traversal '..' prohibited"));
        }
    }
    Ok(base_dir.join(p))
}

pub fn safe_write_file(path: &Path, content: &str, is_exec: bool, force: bool) -> io::Result<()> {
    if path.is_symlink() {
        if !force {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("Symlink {:?} exists, use --force", path)));
        }
        fs::remove_file(path)?;
    } else if path.exists() {
        if path.is_dir() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{:?} is a directory", path)));
        }
        if !force {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("File {:?} exists, use --force", path)));
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Атомарная запись через временный файл
    let temp_path = path.with_extension(format!("tmp.{}", std::process::id()));
    {
        let mut f = File::create(&temp_path)?;
        f.write_all(content.as_bytes())?;
        #[cfg(unix)]
        if is_exec {
            let mut perms = f.metadata()?.permissions();
            perms.set_mode(0o755);
            f.set_permissions(perms)?;
        }
    }
    fs::rename(temp_path, path)?;
    Ok(())
}

pub fn scaffold(input: &str, target_dir: &Path, dry_run: bool, force: bool, no_shebang: bool, no_templates: bool) -> io::Result<()> {
    let mut stack: Vec<String> = Vec::new();
    let root_name = target_dir.file_name().and_then(|s| s.to_str()).unwrap_or("project");

    for raw in input.lines() {
        let node = match parse_line(raw) {
            Some(n) => n,
            None => continue,
        };

        stack.truncate(node.depth);
        let rel_path = if stack.is_empty() {
            node.name.clone()
        } else {
            let mut p = PathBuf::from(&stack[0]);
            for part in &stack[1..] {
                p.push(part);
            }
            p.push(&node.name);
            p.to_string_lossy().to_string()
        };

        let current_path = validate_safe_path(target_dir, &rel_path)?;

        if node.is_dir {
            stack.push(node.name);
            if !dry_run {
                if current_path.is_file() {
                    return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("Cannot create directory {:?}: file exists", current_path)));
                }
                fs::create_dir_all(&current_path)?;
            }
            println!("[DIR]  {}/", rel_path);
            continue;
        }

        if let Some(target) = node.symlink_target {
            if !dry_run {
                if current_path.is_symlink() || current_path.exists() {
                    if !force {
                        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("Target {:?} exists, use --force", current_path)));
                    }
                    if current_path.is_dir() && !current_path.is_symlink() {
                        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{:?} is a directory", current_path)));
                    }
                    fs::remove_file(&current_path)?;
                }
                if let Some(parent) = current_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                #[cfg(unix)]
                symlink(&target, &current_path)?;
            }
            println!("[LINK] {} -> {}", rel_path, target);
            continue;
        }

        let ext = current_path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let file_name = current_path.file_name().and_then(|s| s.to_str()).unwrap_or("");

        let content = if let Some(code) = node.inline_code {
            format!("{}\n", code)
        } else if !no_templates && file_name == "pyproject.toml" {
            format!("[project]\nname = \"{}\"\nversion = \"0.1.0\"\nreadme = \"README.md\"\n", root_name)
        } else if !no_templates && file_name == "README.md" && node.comment.is_some() {
            format!("# {}\n\n> {}\n", root_name, node.comment.as_ref().unwrap())
        } else {
            let mut out = String::new();
            let should_shebang = !no_shebang && (node.is_exec || ext == "sh" || ext == "bash" || (ext == "py" && (rel_path.contains("scripts/") || file_name == "main.py")));
            if should_shebang {
                if ext == "py" { out.push_str("#!/usr/bin/env python3\n"); }
                else if ext == "sh" || ext == "bash" { out.push_str("#!/usr/bin/env bash\n"); }
            }
            if let Some(c) = node.comment {
                if ext == "py" { out.push_str(&format!("\"\"\"{}\"\"\"\n", c)); }
                else if ext == "rs" { out.push_str(&format!("//! {}\n", c)); }
                else { out.push_str(&format!("# {}\n", c)); }
            }
            out
        };

        if !dry_run {
            safe_write_file(&current_path, &content, node.is_exec || ext == "sh", force)?;
        }
        println!("[FILE] {}{}", rel_path, if node.is_exec { " (+x)" } else { "" });
    }
    Ok(())
}
