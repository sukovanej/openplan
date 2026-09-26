use op_task::Abbreviation;
use op_task::reference::{Target, body_target, relative, resolve};

fn opp() -> Option<Abbreviation> {
    Some("OPP".parse().unwrap())
}

#[test]
fn a_path_is_relative_to_the_directory_of_the_file_that_holds_it() {
    assert_eq!(relative("tasks", "tasks/00042-x.md"), "./00042-x.md");
    assert_eq!(relative("tasks", "docs/storage.md"), "../docs/storage.md");
    assert_eq!(relative("docs", "tasks/00042-x.md"), "../tasks/00042-x.md");
}

#[test]
fn a_path_resolves_against_the_directory_and_keeps_no_section() {
    assert_eq!(
        resolve("docs", "../tasks/00042-x.md#Plan").as_deref(),
        Some("tasks/00042-x.md")
    );
    assert_eq!(
        resolve("tasks", "./00042-x.md").as_deref(),
        Some("tasks/00042-x.md")
    );
    assert_eq!(
        resolve("tasks", "00042-x.md").as_deref(),
        Some("tasks/00042-x.md")
    );
    assert_eq!(resolve("tasks", "../../x.md"), None);
    assert_eq!(resolve("tasks", "OPP-42"), None);
}

#[test]
fn a_body_reference_names_what_its_path_resolves_to() {
    assert_eq!(
        body_target(opp(), "docs", "./00042-x.md"),
        Some(Target::Doc("00042-x".to_owned()))
    );
    assert_eq!(
        body_target(opp(), "tasks", "./00042-x.md"),
        Some(Target::Task(42))
    );
    assert_eq!(
        body_target(opp(), "tasks", "../docs/storage.md#Layout"),
        Some(Target::Doc("storage".to_owned()))
    );
    assert_eq!(body_target(opp(), "docs", "../tags/bug.md"), None);
}

#[test]
fn a_key_names_a_task_and_a_name_names_a_doc() {
    assert_eq!(body_target(opp(), "docs", "OPP-42"), Some(Target::Task(42)));
    assert_eq!(
        body_target(opp(), "docs", "storage"),
        Some(Target::Doc("storage".to_owned()))
    );
    assert_eq!(body_target(opp(), "docs", "WEB-7"), None);
    assert_eq!(body_target(opp(), "docs", "42"), None);
    assert_eq!(body_target(opp(), "docs", "Some Title"), None);
}
