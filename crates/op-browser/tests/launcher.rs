use op_browser::{DEFAULT_LAUNCHER, default_launchers, launcher, names_wsl};

const URL: &str = "http://127.0.0.1:4040/";

#[test]
fn a_wsl_kernel_release_names_wsl() {
    assert!(names_wsl("5.15.167.4-microsoft-standard-WSL2\n"));
    assert!(names_wsl("4.4.0-19041-Microsoft\n"));
    assert!(!names_wsl("6.8.0-45-generic\n"));
    assert!(!names_wsl(""));
}

#[test]
fn wsl_keeps_the_default_launcher_after_the_windows_one() {
    let candidates = default_launchers(true, URL);
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].program, "powershell.exe");
    assert_eq!(candidates[1].program, DEFAULT_LAUNCHER);
    assert_eq!(candidates[1].args, [URL]);
}

#[test]
fn a_machine_that_is_not_wsl_uses_the_default_launcher_alone() {
    let candidates = default_launchers(false, URL);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].program, DEFAULT_LAUNCHER);
    assert_eq!(candidates[0].args, [URL]);
}

#[test]
fn the_windows_launcher_passes_the_url_as_one_literal() {
    let candidates = default_launchers(true, "http://host/p?a=1&calc");
    assert_eq!(
        candidates[0].args.last().unwrap(),
        "Start-Process 'http://host/p?a=1&calc'"
    );
}

#[test]
fn the_windows_launcher_escapes_a_quote_in_the_url() {
    let candidates = default_launchers(true, "http://host/p?a='; calc; '");
    assert_eq!(
        candidates[0].args.last().unwrap(),
        "Start-Process 'http://host/p?a=''; calc; '''"
    );
}

#[test]
fn a_browser_entry_places_the_url_where_it_spells_it() {
    let chosen = launcher("firefox --new-tab %s --profile p", URL);
    assert_eq!(chosen.program, "firefox");
    assert_eq!(chosen.args, ["--new-tab", URL, "--profile", "p"]);
}

#[test]
fn a_browser_entry_without_a_placeholder_takes_the_url_last() {
    let chosen = launcher("firefox --new-tab", URL);
    assert_eq!(chosen.program, "firefox");
    assert_eq!(chosen.args, ["--new-tab", URL]);
}
