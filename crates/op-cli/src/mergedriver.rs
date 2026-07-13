use std::process::ExitCode;

pub fn run(ancestor: &str, current: &str, other: &str) -> ExitCode {
    match merge(ancestor, current, other) {
        Ok(Outcome::Merged) => ExitCode::SUCCESS,
        Ok(Outcome::Conflict) => {
            // TODO(skeleton): 3-way section merge. Until then, conflict rather than
            // drop a divergent edit.
            eprintln!("oplan merge-driver: section merge not yet implemented — conflict");
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("oplan merge-driver: {err}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Merged,
    Conflict,
}

fn merge(ancestor: &str, current: &str, other: &str) -> std::io::Result<Outcome> {
    let ours = std::fs::read_to_string(current)?;
    let theirs = std::fs::read_to_string(other)?;
    let _base = std::fs::read_to_string(ancestor)?;

    if let Ok(task) = op_task::Task::from_file_string(&ours) {
        let sections = op_md::headings(&task.body)
            .iter()
            .filter(|h| h.level >= 2)
            .count();
        eprintln!("oplan merge-driver: {current} has {sections} section(s)");
    }

    Ok(if ours == theirs {
        Outcome::Merged
    } else {
        Outcome::Conflict
    })
}
