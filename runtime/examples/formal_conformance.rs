//! Public-API projection for comparison with the separately implemented Lean model.
//! Test-only protocol; neither a receipt parser nor a production runtime entry point.
use lhc_runtime::{Command, Config, Runtime};
use std::io::{self, BufRead};

fn main() {
    let mut runtime = Runtime::new(Config {
        bunches: 1,
        ..Config::default()
    })
    .unwrap();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let words: Vec<_> = line.split_whitespace().collect();
        let ok = match words.as_slice() {
            ["new", n, dt] => {
                runtime = Runtime::new(Config {
                    bunches: n.parse().unwrap(),
                    dt_ms: dt.parse().unwrap(),
                    ..Config::default()
                })
                .unwrap();
                true
            }
            ["step", n] => {
                let n: usize = n.parse().unwrap();
                assert!(n <= 100_000);
                (0..n).all(|_| runtime.step().is_ok())
            }
            [cmd] => {
                let command = match *cmd {
                    "inject" => Command::Inject,
                    "ramp" => Command::Ramp,
                    "squeeze" => Command::Squeeze,
                    "collide" => Command::Collide,
                    "dump" => Command::Dump,
                    "pause" => Command::Pause,
                    "resume" => Command::Resume,
                    "reset" => Command::Reset,
                    "auto-on" => Command::Auto(true),
                    "auto-off" => Command::Auto(false),
                    _ => panic!("unknown command"),
                };
                let before = runtime.canonical_state_bits();
                let ok = runtime.command(command).is_ok();
                if !ok {
                    assert_eq!(runtime.canonical_state_bits(), before);
                }
                ok
            }
            _ => panic!("invalid conformance input"),
        };
        let s = runtime.snapshot();
        let (a, b) = runtime.intensities();
        assert_eq!(a.len(), b.len());
        assert_eq!(runtime.canonical_state_bits().len(), 19 + 2 * a.len());
        println!(
            "{},{},{},{},{},{},{},{},{},{}",
            u8::from(ok),
            s.phase as u64,
            s.phase_elapsed_ms,
            s.tick,
            s.elapsed_ms,
            s.fill,
            u8::from(s.paused),
            u8::from(s.auto),
            s.bunches_beam1,
            s.bunches_beam2
        );
    }
}
