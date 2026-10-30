use std::fs;
use std::io::Write;
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn test_cli_version() {
    let output = Command::new("cargo")
        .args(["run", "--quiet", "--", "--version"])
        .output()
        .expect("Failed to execute astree");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("astree v0.2.3"));
}

#[test]
fn test_security_parent_traversal() {
    let malicious_tree = "bad_project/\n├── ../../escaped.txt\n";
    let mut child = Command::new("cargo")
        .args(["run", "--quiet", "--", "-"])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(malicious_tree.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("prohibited"));
}

#[test]
fn test_security_root_symlink_escape() {
    let temp = tempfile::tempdir().unwrap();
    let outside = temp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let sandbox = temp.path().join("sandbox");
    std::fs::create_dir(&sandbox).unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, sandbox.join("proj")).unwrap();

    let tree = "proj/\n└── payload.sh\n";
    let mut child = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
            "--",
            "-t",
            sandbox.to_str().unwrap(),
            "-",
        ])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(tree.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());
    assert!(!outside.join("payload.sh").exists());
}

#[test]
fn test_security_nested_symlink_escape() {
    let temp = tempfile::tempdir().unwrap();
    let outside = temp.path().join("outside2");
    std::fs::create_dir(&outside).unwrap();
    let sandbox = temp.path().join("sandbox2");
    std::fs::create_dir(&sandbox).unwrap();
    let proj = sandbox.join("proj");
    std::fs::create_dir(&proj).unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, proj.join("sub")).unwrap();

    let tree = "proj/\n└── sub/payload.sh\n";
    let mut child = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
            "--",
            "-t",
            sandbox.to_str().unwrap(),
            "-",
        ])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(tree.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());
    assert!(!outside.join("payload.sh").exists());
}

#[test]
fn test_bracketed_filename_preservation() {
    let temp = tempfile::tempdir().unwrap();
    let tree = "bracket_proj/\n├── [notes] file.txt\n└── [1.5K] data.txt\n";

    let mut child = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
            "--",
            "-t",
            temp.path().to_str().unwrap(),
            "-",
        ])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(tree.as_bytes())
        .unwrap();
    let status = child.wait().unwrap();
    assert!(status.success());
    assert!(temp.path().join("bracket_proj/[notes] file.txt").exists());
    assert!(temp.path().join("bracket_proj/data.txt").exists());
}

// P3: Регрессионный тест прав на исполнение inline code для Rust
#[test]
fn test_inline_code_executable_bit_parity() {
    let temp = tempfile::tempdir().unwrap();
    let scenarios = [
        ("scripts/launch.py", "print(\"hello\")"),
        ("bin/tool.py", "print(\"hello\")"),
        ("cli/run.py", "print(\"hello\")"),
        ("main.py", "print(\"hello\")"),
        ("app.py", "print(\"hello\")"),
        ("script.bash", "echo hello"),
        ("script.zsh", "echo hello"),
    ];

    let mut tree = String::from("proj/\n");
    for (rel_path, inline_str) in &scenarios {
        tree.push_str(&format!("├── {} # = {}\n", rel_path, inline_str));
    }

    let mut child = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
            "--",
            "-t",
            temp.path().to_str().unwrap(),
            "-",
        ])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(tree.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());

    for (rel_path, inline_str) in &scenarios {
        let p = temp.path().join("proj").join(rel_path);
        assert!(p.exists());
        let content = fs::read_to_string(&p).unwrap();
        assert_eq!(content, format!("{}\n", inline_str));
        assert!(!content.starts_with("#!"));
        #[cfg(unix)]
        {
            let mode = p.metadata().unwrap().permissions().mode();
            assert!(
                mode & 0o111 != 0,
                "File {:?} must have executable permissions",
                rel_path
            );
        }
    }
}

// P4: Симметричный тест перезаписи конечного симлинка для Rust
#[test]
fn test_security_leaf_symlink_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let outside = temp.path().join("outside_target.txt");
    fs::write(&outside, "precious original").unwrap();

    let proj = temp.path().join("proj");
    fs::create_dir(&proj).unwrap();
    let link = proj.join("link.txt");

    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    let tree = "proj/\n└── link.txt # = overwritten\n";

    // Без --force -> отказ, цель не тронута
    let mut child_no_force = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
            "--",
            "-t",
            temp.path().to_str().unwrap(),
            "-",
        ])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child_no_force
        .stdin
        .as_mut()
        .unwrap()
        .write_all(tree.as_bytes())
        .unwrap();
    let status_no_force = child_no_force.wait_with_output().unwrap();
    assert!(!status_no_force.status.success());
    assert_eq!(fs::read_to_string(&outside).unwrap(), "precious original");
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());

    // C --force -> симлинк удален, создан регулярный файл, цель не тронута
    let mut child_force = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
            "--",
            "-t",
            temp.path().to_str().unwrap(),
            "-f",
            "-",
        ])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child_force
        .stdin
        .as_mut()
        .unwrap()
        .write_all(tree.as_bytes())
        .unwrap();
    assert!(child_force.wait().unwrap().success());

    assert_eq!(fs::read_to_string(&outside).unwrap(), "precious original");
    assert!(!link.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(link.is_file());
    assert_eq!(fs::read_to_string(&link).unwrap(), "overwritten\n");
}
