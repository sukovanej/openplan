use std::process::Command;

#[test]
fn update_refuses_a_binary_that_cargo_install_owns() {
    let cargo_home = tempfile::tempdir().unwrap();
    let bin = cargo_home.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let exe = bin.join("openplan");
    std::fs::copy(env!("CARGO_BIN_EXE_openplan"), &exe).unwrap();

    let output = Command::new(&exe)
        .env("CARGO_HOME", cargo_home.path())
        .arg("update")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cargo install owns"), "{stderr}");
    assert!(stderr.contains("cargo install openplan"), "{stderr}");
}
