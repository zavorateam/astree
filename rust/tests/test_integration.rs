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
    assert!(stdout.contains("astree v0.2.0"));
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
fn test_security_intermediate_symlink_escape() {
    let temp = tempfile::tempdir().unwrap();
    let outside = temp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let proj = temp.path().join("proj");
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
            "-",
        ])
        .current_dir(temp.path())
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
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("escapes"));
    assert!(!outside.join("payload.sh").exists());
}

#[test]
fn test_markdown_codeblock_scaffold() {
    let temp = tempfile::tempdir().unwrap();
    let md_tree = "Here is the structure:\n```bash\nmd_app/\n├── file.txt\n```\nDone!";

    let mut child = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
            "--",
            "-",
        ])
        .current_dir(temp.path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(md_tree.as_bytes())
        .unwrap();
    let status = child.wait().unwrap();
    assert!(status.success());
    assert!(temp.path().join("md_app/file.txt").exists());
}
