use op_daemon::{SERVE_ARG, serve_request};

fn args(rest: &[&str]) -> Vec<String> {
    std::iter::once("openplan")
        .chain(rest.iter().copied())
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_flag_and_a_port_ask_for_a_daemon() {
    assert_eq!(serve_request(args(&[SERVE_ARG, "7373"])), Some(7373));
}

#[test]
fn an_ordinary_command_line_asks_for_nothing() {
    assert_eq!(serve_request(args(&["list", "--json"])), None);
    assert_eq!(serve_request(args(&[])), None);
}

#[test]
fn the_flag_counts_only_as_the_first_argument() {
    assert_eq!(serve_request(args(&["create", SERVE_ARG, "7373"])), None);
}

#[test]
fn a_port_that_is_not_a_port_asks_for_nothing() {
    assert_eq!(serve_request(args(&[SERVE_ARG])), None);
    assert_eq!(serve_request(args(&[SERVE_ARG, "kettle"])), None);
}
