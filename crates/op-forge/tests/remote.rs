use op_forge::{Forge, ForgeKind, glab_hosts};

fn no_other_gitlab(_: &str) -> bool {
    false
}

#[test]
fn each_remote_form_names_the_same_github_repository() {
    for url in [
        "https://github.com/sukovanej/openplan",
        "https://github.com/sukovanej/openplan.git",
        "https://github.com/sukovanej/openplan/",
        "https://token@github.com/sukovanej/openplan.git",
        "git@github.com:sukovanej/openplan.git",
        "git@github.com:sukovanej/openplan",
        "ssh://git@github.com/sukovanej/openplan.git",
        "ssh://git@github.com:22/sukovanej/openplan.git",
        "https://GitHub.com/sukovanej/openplan.git",
    ] {
        assert_eq!(
            Forge::of_remote(url, no_other_gitlab),
            Some(Forge {
                kind: ForgeKind::Github,
                host: "github.com".to_owned(),
                repo: "sukovanej/openplan".to_owned(),
            }),
            "{url}"
        );
    }
}

#[test]
fn gitlab_com_is_gitlab_and_keeps_the_group_path() {
    assert_eq!(
        Forge::of_remote("git@gitlab.com:group/sub/app.git", no_other_gitlab),
        Some(Forge {
            kind: ForgeKind::Gitlab,
            host: "gitlab.com".to_owned(),
            repo: "group/sub/app".to_owned(),
        })
    );
}

#[test]
fn another_host_is_gitlab_only_when_the_caller_knows_it() {
    let url = "https://gitlab.example.com/group/app.git";
    assert_eq!(Forge::of_remote(url, no_other_gitlab), None);
    assert_eq!(
        Forge::of_remote(url, |host| host == "gitlab.example.com"),
        Some(Forge {
            kind: ForgeKind::Gitlab,
            host: "gitlab.example.com".to_owned(),
            repo: "group/app".to_owned(),
        })
    );
}

#[test]
fn a_remote_with_no_forge_repository_has_no_forge() {
    let any_gitlab = |_: &str| true;
    for url in [
        "",
        "/srv/git/openplan.git",
        "../openplan",
        "file:///srv/git/openplan.git",
        "C:\\git\\openplan",
        "https://github.com/sukovanej",
        "https://github.com/a/b/c",
        "https://gitlab.com/app",
    ] {
        assert_eq!(Forge::of_remote(url, any_gitlab), None, "{url}");
    }
}

#[test]
fn the_hosts_of_glab_are_the_lines_that_are_not_indented() {
    let auth_status = "gitlab.com\n  \u{2713} Logged in to gitlab.com as ada (keyring)\n  \u{2713}                        Token found: ****\n\nGitLab.Example.com\n  x gitlab.example.com: api call                        failed\nNo token found\n";
    assert_eq!(
        glab_hosts(auth_status),
        vec!["gitlab.com".to_owned(), "gitlab.example.com".to_owned()]
    );
}
