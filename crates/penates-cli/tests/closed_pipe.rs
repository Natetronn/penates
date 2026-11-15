//! `penates … | head` must not panic when the reader exits early.

#[cfg(unix)]
#[test]
fn closed_pipe_is_not_a_panic() {
    let empty = std::env::temp_dir().join(format!("penates-pipe-test-{}", std::process::id()));
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_penates"))
        .arg("list") // prints "no apps yet…" with println!
        .env("XDG_CONFIG_HOME", &empty)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take()); // the reader goes away before anything is written
    let status = child.wait().unwrap();
    let stderr = std::io::read_to_string(child.stderr.take().unwrap()).unwrap();
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert_ne!(status.code(), Some(101), "a panic exits with 101");
}
