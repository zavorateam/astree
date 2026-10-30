use std::io::Write;
use std::process::Command;

#[test]
fn test_cli_version() {
    let output = Command::new("cargo")
        .args(["run", "--quiet", "--", "--version"])
        .output()
        .expect("Failed to execute astree");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("astree v0.2.2"));
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
