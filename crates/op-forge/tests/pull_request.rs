use op_forge::{Forge, ForgeKind, PullRequest, PullRequestError};

fn github(repo: &str) -> Forge {
    Forge {
        kind: ForgeKind::Github,
        host: "github.com".to_owned(),
        repo: repo.to_owned(),
    }
}

fn gitlab(host: &str, repo: &str) -> Forge {
    Forge {
        kind: ForgeKind::Gitlab,
        host: host.to_owned(),
        repo: repo.to_owned(),
    }
}

#[test]
fn a_github_address_names_the_repository_and_the_number() {
    let parsed = PullRequest::parse("https://github.com/sukovanej/openplan/pull/214").unwrap();
    assert_eq!(parsed, github("sukovanej/openplan").pull_request(214));
}

#[test]
fn a_gitlab_address_keeps_the_whole_group_path() {
    let parsed =
        PullRequest::parse("https://gitlab.example.com/group/sub/app/-/merge_requests/7").unwrap();
    assert_eq!(
        parsed,
        gitlab("gitlab.example.com", "group/sub/app").pull_request(7)
    );
}

#[test]
fn every_page_of_a_pull_request_has_one_canonical_address() {
    for address in [
        "https://github.com/sukovanej/openplan/pull/214",
        "https://github.com/sukovanej/openplan/pull/214/",
        "https://github.com/sukovanej/openplan/pull/214/files",
        "https://github.com/sukovanej/openplan/pull/214/commits",
        "https://github.com/sukovanej/openplan/pull/214/commits/0a1b2c3",
        "https://github.com/sukovanej/openplan/pull/214?diff=split",
        "https://github.com/sukovanej/openplan/pull/214/files#diff-1",
        "https://GitHub.com/sukovanej/openplan/pull/214",
        "  https://github.com/sukovanej/openplan/pull/214\n",
    ] {
        assert_eq!(
            PullRequest::parse(address).unwrap().url(),
            "https://github.com/sukovanej/openplan/pull/214",
            "{address}"
        );
    }
    for address in [
        "https://gitlab.com/group/app/-/merge_requests/7",
        "https://gitlab.com/group/app/-/merge_requests/7/diffs",
        "https://gitlab.com/group/app/-/merge_requests/7/commits?page=2",
        "https://gitlab.com/group/app/-/merge_requests/7#note_1",
    ] {
        assert_eq!(
            PullRequest::parse(address).unwrap().url(),
            "https://gitlab.com/group/app/-/merge_requests/7",
            "{address}"
        );
    }
}

#[test]
fn every_other_address_is_refused() {
    for address in [
        "",
        "214x",
        "sukovanej/openplan#214",
        "http://github.com/sukovanej/openplan/pull/214",
        "github.com/sukovanej/openplan/pull/214",
        "https://github.com/sukovanej/openplan",
        "https://github.com/sukovanej/openplan/issues/214",
        "https://github.com/sukovanej/openplan/pull/",
        "https://github.com/sukovanej/openplan/pull/0",
        "https://github.com/sukovanej/openplan/pull/0214",
        "https://github.com/sukovanej/openplan/pull/abc",
        "https://github.com/openplan/pull/214",
        "https://gitlab.com/group/app/-/issues/7",
        "https://gitlab.com/app/-/merge_requests/7",
        "https://gitlab.com/group/app/merge_requests/7",
        "https://example.com/owner/repo/pull/214",
    ] {
        assert_eq!(
            PullRequest::parse(address),
            Err(PullRequestError::NotAnAddress {
                got: address.to_owned()
            }),
            "{address}"
        );
    }
}

#[test]
fn a_number_names_a_pull_request_of_the_project_repository() {
    let project = github("sukovanej/openplan");
    for input in ["214", "#214", " 214 "] {
        assert_eq!(
            PullRequest::resolve(input, Some(&project)).unwrap().url(),
            "https://github.com/sukovanej/openplan/pull/214",
            "{input}"
        );
    }
    let project = gitlab("gitlab.example.com", "group/sub/app");
    assert_eq!(
        PullRequest::resolve("7", Some(&project)).unwrap().url(),
        "https://gitlab.example.com/group/sub/app/-/merge_requests/7"
    );
}

#[test]
fn a_number_is_refused_when_the_project_has_no_forge() {
    assert_eq!(
        PullRequest::resolve("#214", None),
        Err(PullRequestError::NoForge {
            got: "#214".to_owned()
        })
    );
}

#[test]
fn an_address_resolves_with_or_without_a_project_forge() {
    let address = "https://github.com/rust-lang/cargo/pull/1234/files";
    let project = github("sukovanej/openplan");
    for forge in [None, Some(&project)] {
        assert_eq!(
            PullRequest::resolve(address, forge).unwrap().url(),
            "https://github.com/rust-lang/cargo/pull/1234"
        );
    }
}

#[test]
fn the_short_form_names_the_repository_only_when_it_is_another_one() {
    let project = github("sukovanej/openplan");
    let own = github("Sukovanej/OpenPlan").pull_request(214);
    let other = github("rust-lang/cargo").pull_request(1234);
    assert_eq!(own.short(Some(&project)), "#214");
    assert_eq!(other.short(Some(&project)), "rust-lang/cargo#1234");
    assert_eq!(own.short(None), "Sukovanej/OpenPlan#214");
    assert_eq!(
        gitlab("gitlab.com", "sukovanej/openplan")
            .pull_request(214)
            .short(Some(&project)),
        "sukovanej/openplan#214"
    );
}
