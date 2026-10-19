use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static SERIAL: AtomicUsize = AtomicUsize::new(0);
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lhc-runtime"))
        .args(args)
        .output()
        .unwrap()
}
fn temp(contents: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "lhc-{}-{}.txt",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::SeqCst)
    ));
    fs::write(&path, contents).unwrap();
    path
}
#[test]
fn reproducible_receipt_and_tamper_rejection() {
    let a = cli(&["run", "--steps", "3000", "--bunches", "12"]);
    assert!(a.status.success(), "{:?}", a);
    let b = cli(&["run", "--steps", "3000", "--bunches", "12"]);
    assert_eq!(a.stdout, b.stdout);
    let p = temp(&a.stdout);
    assert!(cli(&["replay", p.to_str().unwrap()]).status.success());
    let mut bad = a.stdout;
    let n = bad.len();
    bad[n - 2] = if bad[n - 2] == b'0' { b'1' } else { b'0' };
    fs::write(&p, bad).unwrap();
    assert!(!cli(&["replay", p.to_str().unwrap()]).status.success());
    fs::remove_file(p).unwrap();
}
#[test]
fn malformed_and_unbounded_inputs_fail() {
    for args in [
        vec!["run", "--steps", "100001"],
        vec!["run", "--steps", "0"],
        vec!["run", "--beta", "NaN"],
        vec!["run", "--workers", "0"],
        vec!["run", "--bunches", "999999"],
        vec!["run", "--steps", "1", "--steps", "2"],
        vec!["run", "--dt-ms", "0"],
        vec!["run", "--wat", "2"],
        vec!["run", "--steps"],
    ] {
        assert!(!cli(&args).status.success(), "{args:?}");
    }
    let p = temp(&vec![b'x'; 262145]);
    assert!(!cli(&["replay", p.to_str().unwrap()]).status.success());
    fs::remove_file(p).unwrap();
}
#[test]
fn scripts_validate_order_and_replay_controls() {
    let p = temp(b"0 auto-on\n0 inject\n2 pause\n5 resume\n10 dump\n");
    let result = cli(&["run", "--steps", "20", "--script", p.to_str().unwrap()]);
    assert!(result.status.success(), "{:?}", result);
    let r = temp(&result.stdout);
    assert!(cli(&["replay", r.to_str().unwrap()]).status.success());
    fs::remove_file(r).unwrap();
    for invalid in [
        "1 inject\n0 pause\n",
        "20 inject\n",
        "0 unknown\n",
        "0 ramp\n",
    ] {
        fs::write(&p, invalid).unwrap();
        assert!(
            !cli(&["run", "--steps", "20", "--script", p.to_str().unwrap()])
                .status
                .success()
        );
    }
    fs::remove_file(p).unwrap();
}

#[test]
fn empty_script_round_trips() {
    let p = temp(b"");
    let result = cli(&["run", "--steps", "1", "--script", p.to_str().unwrap()]);
    assert!(result.status.success());
    let r = temp(&result.stdout);
    assert!(cli(&["replay", r.to_str().unwrap()]).status.success());
    fs::remove_file(p).unwrap();
    fs::remove_file(r).unwrap();
}
