mod common;

use common::{Home, ok, stderr};

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

fn auto(home: &Home) -> serde_json::Value {
    let text = std::fs::read_to_string(home.path().join("update.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()["auto"].clone()
}

#[test]
fn the_auto_switch_turns_the_daemon_updates_off_and_on() {
    let home = Home::new();

    let off = ok(home
        .cmd()
        .args(["update", "--auto", "off"])
        .output()
        .unwrap());
    assert!(off.contains("does not update itself"), "{off}");
    assert_eq!(auto(&home), false);

    let on = ok(home
        .cmd()
        .args(["update", "--auto", "on"])
        .output()
        .unwrap());
    assert!(on.contains("updates itself"), "{on}");
    assert_eq!(auto(&home), true);
}
