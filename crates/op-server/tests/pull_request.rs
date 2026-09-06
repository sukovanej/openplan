use op_server::pull_request::web_url;

#[test]
fn a_github_remote_becomes_a_web_url() {
    for remote in [
        "git@github.com:sukovanej/openplan.git",
        "https://github.com/sukovanej/openplan.git",
        "https://github.com/sukovanej/openplan",
        "ssh://git@github.com/sukovanej/openplan.git",
    ] {
        assert_eq!(
            web_url(remote).as_deref(),
            Some("https://github.com/sukovanej/openplan"),
            "{remote}"
        );
    }
}

#[test]
fn anything_else_has_no_web_url() {
    for remote in [
        "git@gitlab.com:sukovanej/openplan.git",
        "/tmp/remote.git",
        "https://github.com/",
    ] {
        assert_eq!(web_url(remote), None, "{remote}");
    }
}
