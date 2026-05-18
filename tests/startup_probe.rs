use std::process::Command;

#[test]
fn startup_probe_keeps_backend_diagnostics_out_of_terminal_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_orbifold"))
        .arg("--startup-probe")
        .env("RUST_LOG", "off")
        .output()
        .expect("startup probe should run without opening a window");

    assert!(
        output.status.success(),
        "startup probe failed: status={:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_no_raw_backend_diagnostics("stdout", &output.stdout);
    assert_no_raw_backend_diagnostics("stderr", &output.stderr);
}

fn assert_no_raw_backend_diagnostics(label: &str, output: &[u8]) {
    let text = String::from_utf8_lossy(output);
    for pattern in [
        "ALSA lib ",
        "Cannot connect to server socket",
        "JackShmReadWritePtr",
        "jack server is not running",
        "JACK server is not running",
    ] {
        assert!(
            !text.contains(pattern),
            "{label} contained raw backend diagnostic `{pattern}`:\n{text}"
        );
    }
}
