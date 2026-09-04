use fs2::FileExt as _;
use op_daemon::{DEFAULT_PORT, DaemonInfo, Home, base_url};

fn info() -> DaemonInfo {
    DaemonInfo {
        pid: 4242,
        port: 7500,
        version: "0.1.0".to_owned(),
        started_at: 1_700_000_000,
    }
}

#[test]
fn written_info_reads_back_whole() {
    let dir = tempfile::tempdir().unwrap();
    let home = Home::at(dir.path());
    home.ensure_dir().unwrap();

    home.write_info(&info()).unwrap();

    assert_eq!(home.read_info().unwrap().port, 7500);
    assert_eq!(home.read_info().unwrap().pid, 4242);
}

#[test]
fn a_missing_or_unreadable_record_reads_as_no_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let home = Home::at(dir.path());
    home.ensure_dir().unwrap();

    assert!(home.read_info().is_none());

    std::fs::write(home.info_path(), "{ not json").unwrap();
    assert!(home.read_info().is_none());
}

#[test]
fn clearing_the_record_leaves_no_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let home = Home::at(dir.path());
    home.ensure_dir().unwrap();
    home.write_info(&info()).unwrap();

    home.clear_info();

    assert!(home.read_info().is_none());
}

#[test]
fn a_held_lock_is_not_free() {
    let dir = tempfile::tempdir().unwrap();
    let home = Home::at(dir.path());
    home.ensure_dir().unwrap();

    assert!(home.lock_is_free().unwrap());

    let held = home.open_lock().unwrap();
    held.try_lock_exclusive().unwrap();
    assert!(!home.lock_is_free().unwrap());

    fs2::FileExt::unlock(&held).unwrap();
    assert!(home.lock_is_free().unwrap());
}

#[test]
fn the_default_port_names_the_loopback_daemon() {
    assert_eq!(base_url(DEFAULT_PORT), "http://127.0.0.1:7373");
}
