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
fn test_security_path_traversal() {
    let malicious_tree = "bad_project/\n├── ../../escaped.txt\n";
    let mut child = Command::new("cargo")
        .args(["run", "--quiet", "--", "-"])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    use std::io::Write;
    child.stdin.as_mut().unwrap().write_all(malicious_tree.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("prohibited"));
}
