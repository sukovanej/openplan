mod common;

use common::{Home, stderr};

#[test]
fn update_refuses_a_binary_that_cargo_install_owns() {
    let home = Home::new();
    let cargo_home = tempfile::tempdir().unwrap();
    let bin = cargo_home.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let exe = bin.join("openplan");
    std::fs::copy(env!("CARGO_BIN_EXE_openplan"), &exe).unwrap();

    let output = std::process::Command::new(&exe)
        .env("OPENPLAN_HOME", home.path())
        .env("OPENPLAN_PORT", "0")
        .env("CARGO_HOME", cargo_home.path())
        .arg("update")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("cargo install owns"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("cargo install openplan"),
        "{}",
        stderr(&output)
    );
}
