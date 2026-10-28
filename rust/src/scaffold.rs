use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};

use crate::parser::parse_line;

const PYPROJECT_TEMPLATE: &str = r#"[project]
name = "{name}"
version = "0.1.0"
description = "{comment}"
readme = "README.md"
requires-python = ">=3.10"
dependencies = []

[build-system]
requires = ["setuptools>=61.0"]
build-backend = "setuptools.build_meta"
"#;

const GITIGNORE_TEMPLATE: &str = r#"__pycache__/
*.py[cod]
.venv/
build/
dist/
target/
*.egg-info/
.env
.DS_Store
"#;

pub fn validate_safe_path(base_dir: &Path, rel_path: &str, boundary: &Path) -> io::Result<PathBuf> {
    let p = Path::new(rel_path);
    if p.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "Security violation: absolute path '{}' is prohibited",
                rel_path
            ),
        ));
    }
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "Security violation: parent traversal '..' in '{}' is prohibited",
                rel_path
            ),
        ));
    }

    let canonical_boundary = boundary
        .canonicalize()
        .unwrap_or_else(|_| boundary.to_path_buf());
    let mut curr = base_dir.to_path_buf();

    let components: Vec<_> = p.components().collect();
    if components.is_empty() {
        return Ok(base_dir.to_path_buf());
    }

    for comp in &components[..components.len() - 1] {
        if let Component::Normal(c) = comp {
            curr.push(c);
            if curr.is_symlink() {
                let resolved = match curr.canonicalize() {
                    Ok(r) => r,
                    Err(_) => {
                        let target = fs::read_link(&curr)?;
                        if target.is_absolute() {
                            target
                        } else {
                            curr.parent().unwrap_or(base_dir).join(target)
                        }
                    }
                };
                if resolved.strip_prefix(&canonical_boundary).is_err() {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!("Security violation: intermediate symlink '{:?}' escapes root target boundary", curr),
                    ));
                }
            }
        }
    }

    Ok(base_dir.join(p))
}

pub fn safe_write_file(path: &Path, content: &str, is_exec: bool, force: bool) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let is_symlink = path
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);

    if !force {
        if is_symlink {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "Symlink '{:?}' already exists. Use --force to overwrite",
                    path
                ),
            ));
        }
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        let mut f = match opts.open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("File '{:?}' already exists. Use --force to overwrite", path),
                ));
            }
            Err(e) => return Err(e),
        };
        f.write_all(content.as_bytes())?;
        #[cfg(unix)]
        if is_exec {
            let mut perms = f.metadata()?.permissions();
            perms.set_mode(0o755);
            f.set_permissions(perms)?;
        }
    } else {
        if path.exists() && path.is_dir() && !is_symlink {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Cannot overwrite directory '{:?}' with a file", path),
            ));
        }

        if is_symlink {
            fs::remove_file(path)?;
        }

        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let rand_suffix: u128 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp_path = parent.join(format!(
            ".astree_tmp_{}_{}",
            std::process::id(),
            rand_suffix
        ));

        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        {
            let mut f = opts.open(&temp_path)?;
            f.write_all(content.as_bytes())?;
            #[cfg(unix)]
            if is_exec {
                let mut perms = f.metadata()?.permissions();
                perms.set_mode(0o755);
                f.set_permissions(perms)?;
            }
        }
        fs::rename(temp_path, path)?;
    }

    Ok(())
}

pub fn scaffold(
    lines: &[String],
    target_dir: &Path,
    dry_run: bool,
    force: bool,
    no_shebang: bool,
    no_templates: bool,
) -> io::Result<()> {
    let mut stack: Vec<String> = Vec::new();
    let base_dir = target_dir
        .canonicalize()
        .unwrap_or_else(|_| target_dir.to_path_buf());
    let mut root_boundary = base_dir.clone();
    let mut root_name = target_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();

    let mut has_root = false;
    for raw in lines {
        if let Some(node) = parse_line(raw) {
            if node.depth == 0 && node.is_dir {
                has_root = true;
            }
            break;
        }
    }

    for raw in lines {
        let node = match parse_line(raw) {
            Some(n) => n,
            None => continue,
        };

        let effective_depth = if has_root {
            node.depth
        } else {
            node.depth.saturating_sub(1)
        };
        stack.truncate(effective_depth);

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

        let current_path = validate_safe_path(&base_dir, &rel_path, &root_boundary)?;

        if node.is_dir {
            stack.push(node.name.clone());
            if node.depth == 0 && has_root {
                root_name = node.name.clone();
                root_boundary = base_dir.join(&node.name);
            }
            if !dry_run {
                if current_path.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("Cannot create directory '{:?}': file exists", current_path),
                    ));
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
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            format!(
                                "Cannot overwrite '{:?}' with symlink without --force",
                                current_path
                            ),
                        ));
                    }
                    if current_path.is_dir() && !current_path.is_symlink() {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            format!(
                                "Cannot overwrite directory '{:?}' with symlink",
                                current_path
                            ),
                        ));
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

        let ext = current_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let file_name = current_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let content = if let Some(code) = node.inline_code {
            format!("{}\n", code)
        } else if !no_templates && file_name == "pyproject.toml" {
            PYPROJECT_TEMPLATE
                .replace("{name}", &root_name)
                .replace("{comment}", node.comment.as_deref().unwrap_or(""))
        } else if !no_templates && file_name == "README.md" && node.comment.is_some() {
            format!("# {}\n\n> {}\n", root_name, node.comment.as_ref().unwrap())
        } else if !no_templates && file_name == ".gitignore" {
            GITIGNORE_TEMPLATE.to_string()
        } else {
            let parts: Vec<_> = current_path
                .components()
                .filter_map(|c| c.as_os_str().to_str())
                .collect();
            let in_script_dir = parts
                .iter()
                .any(|&p| matches!(p, "scripts" | "bin" | "cli"));
            let is_entrypoint = matches!(file_name, "main.py" | "app.py");

            let should_shebang = !no_shebang
                && (node.is_exec
                    || matches!(ext, "sh" | "bash" | "zsh")
                    || (ext == "py" && (in_script_dir || is_entrypoint)));

            let mut out = String::new();
            if should_shebang {
                match ext {
                    "py" => out.push_str("#!/usr/bin/env python3\n"),
                    "sh" | "bash" => out.push_str("#!/usr/bin/env bash\n"),
                    "zsh" => out.push_str("#!/usr/bin/env zsh\n"),
                    "js" => out.push_str("#!/usr/bin/env node\n"),
                    _ => {}
                }
            }
            if let Some(c) = node.comment {
                if ext == "py" {
                    out.push_str(&format!("\"\"\"{}\"\"\"\n", c));
                } else if ext == "rs" {
                    out.push_str(&format!("//! {}\n", c));
                } else {
                    out.push_str(&format!("# {}\n", c));
                }
            }
            out
        };

        if !dry_run {
            safe_write_file(&current_path, &content, node.is_exec || ext == "sh", force)?;
        }
        println!(
            "[FILE] {}{}",
            rel_path,
            if node.is_exec { " (+x)" } else { "" }
        );
    }
    Ok(())
}
