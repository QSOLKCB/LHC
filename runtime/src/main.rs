//! Bounded, dependency-free runner. Receipts are comparisons, not signatures.
#![forbid(unsafe_code)]
use lhc_runtime::{Command, Config, Kernel, Phase, Runtime};
use std::{env, fs::File, io::Read, process::ExitCode, time::Instant};

const MAX_STEPS: usize = 100_000;
const MAX_INPUT: u64 = 262_144;
const HEADER: &str = "lhc-receipt-v1 model=educational-cycle-v1 package=0.1.0";

#[derive(Clone)]
struct Plan {
    config: Config,
    steps: usize,
    commands: Vec<(usize, Command)>,
}

fn number<T: std::str::FromStr>(s: &str) -> Result<T, String> {
    s.parse().map_err(|_| format!("invalid number: {s}"))
}
fn read_bounded(path: &str) -> Result<String, String> {
    let mut s = String::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_INPUT + 1)
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    if s.len() as u64 > MAX_INPUT {
        return Err("input exceeds 256 KiB".into());
    }
    Ok(s)
}
fn command(s: &str) -> Result<Command, String> {
    Ok(match s {
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
        _ => return Err(format!("unknown command: {s}")),
    })
}
fn command_name(c: Command) -> &'static str {
    match c {
        Command::Inject => "inject",
        Command::Ramp => "ramp",
        Command::Squeeze => "squeeze",
        Command::Collide => "collide",
        Command::Dump => "dump",
        Command::Pause => "pause",
        Command::Resume => "resume",
        Command::Reset => "reset",
        Command::Auto(true) => "auto-on",
        Command::Auto(false) => "auto-off",
    }
}
fn commands(text: &str, steps: usize) -> Result<Vec<(usize, Command)>, String> {
    let mut result = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 2 {
            return Err("command must be: STEP COMMAND".into());
        }
        let step = number::<usize>(fields[0])?;
        if step >= steps || result.last().is_some_and(|(prev, _)| *prev > step) {
            return Err("command steps must be ordered and less than total steps".into());
        }
        result.push((step, command(fields[1])?));
        if result.len() > 1024 {
            return Err("at most 1024 commands".into());
        }
    }
    Ok(result)
}
fn execute(plan: &Plan) -> Result<Runtime, String> {
    if plan.steps == 0 || plan.steps > MAX_STEPS {
        return Err("steps must be 1..=100000".into());
    }
    let mut runtime = Runtime::new(plan.config).map_err(|e| e.to_string())?;
    let mut next = 0;
    for step in 0..plan.steps {
        while next < plan.commands.len() && plan.commands[next].0 == step {
            runtime
                .command(plan.commands[next].1)
                .map_err(|e| format!("step {step}: {e}"))?;
            next += 1;
        }
        runtime.step().map_err(|e| e.to_string())?;
    }
    Ok(runtime)
}
fn state(runtime: &Runtime) -> String {
    runtime
        .canonical_state_bits()
        .iter()
        .map(|v| format!("{v:016x}"))
        .collect::<Vec<_>>()
        .join(",")
}
fn receipt(plan: &Plan, runtime: &Runtime) -> String {
    let kernel = match plan.config.kernel {
        Kernel::Scalar => "scalar".to_string(),
        Kernel::Parallel(n) => format!("parallel:{n}"),
    };
    let mut result = format!(
        "{HEADER}\nbunches {}\ndt_ms {}\nbeta_bits {:016x}\nkernel {kernel}\nsteps {}\ncommands\n",
        plan.config.bunches,
        plan.config.dt_ms,
        plan.config.beta_star.to_bits(),
        plan.steps
    );
    for (step, cmd) in &plan.commands {
        result.push_str(&format!("{step} {}\n", command_name(*cmd)));
    }
    result.push_str(&format!("state\n{}\n", state(runtime)));
    result
}
fn parse_receipt(text: &str) -> Result<(Plan, String), String> {
    let lines: Vec<_> = text.lines().collect();
    if lines.first() != Some(&HEADER) || lines.len() < 9 {
        return Err("unsupported or truncated receipt".into());
    }
    let field = |i: usize, key: &str| -> Result<&str, String> {
        lines[i]
            .strip_prefix(&format!("{key} "))
            .ok_or_else(|| format!("missing {key}"))
    };
    let bunches = number(field(1, "bunches")?)?;
    let dt_ms = number(field(2, "dt_ms")?)?;
    let beta_star = f64::from_bits(
        u64::from_str_radix(field(3, "beta_bits")?, 16).map_err(|_| "invalid beta bits")?,
    );
    let k = field(4, "kernel")?;
    let kernel = if k == "scalar" {
        Kernel::Scalar
    } else if let Some(n) = k.strip_prefix("parallel:") {
        Kernel::Parallel(number(n)?)
    } else {
        return Err("invalid kernel".into());
    };
    let steps = number(field(5, "steps")?)?;
    if lines[6] != "commands" {
        return Err("missing commands marker".into());
    }
    let end = lines
        .iter()
        .position(|s| *s == "state")
        .ok_or("missing state marker")?;
    if end < 7 || end + 2 != lines.len() {
        return Err("invalid receipt structure".into());
    }
    let commands = commands(&lines[7..end].join("\n"), steps)?;
    let plan = Plan {
        config: Config {
            bunches,
            dt_ms,
            beta_star,
            kernel,
        },
        steps,
        commands,
    };
    Ok((plan, lines[end + 1].into()))
}
fn bench() -> Result<(), String> {
    // Same frozen start, warm both policies, alternate paired samples.
    let mut scalar = Runtime::new(Config {
        bunches: 2808,
        ..Config::default()
    })
    .map_err(|e| e.to_string())?;
    scalar
        .command(Command::Auto(true))
        .map_err(|e| e.to_string())?;
    scalar.command(Command::Inject).map_err(|e| e.to_string())?;
    for _ in 0..100_000 {
        if scalar.snapshot().phase == Phase::StableBeams {
            break;
        }
        scalar.step().map_err(|e| e.to_string())?;
    }
    if scalar.snapshot().phase != Phase::StableBeams {
        return Err("benchmark failed to reach stable beams".into());
    }
    let plan = Plan {
        config: Config {
            bunches: 2808,
            kernel: Kernel::Parallel(4),
            ..Config::default()
        },
        steps: scalar.snapshot().tick as usize,
        commands: vec![(0, Command::Auto(true)), (0, Command::Inject)],
    };
    let parallel = execute(&plan)?;
    if scalar.canonical_state_bits() != parallel.canonical_state_bits() {
        return Err("benchmark setup parity failed".into());
    }
    let run = |base: &Runtime| -> Result<(u128, Vec<u64>), String> {
        let mut r = base.clone();
        let now = Instant::now();
        for _ in 0..128 {
            r.step().map_err(|e| e.to_string())?;
        }
        let ns = now.elapsed().as_nanos();
        Ok((ns, r.canonical_state_bits()))
    };
    run(&scalar)?;
    run(&parallel)?;
    let mut a = Vec::new();
    let mut b = Vec::new();
    for i in 0..7 {
        let (x, y) = if i % 2 == 0 {
            (run(&scalar)?, run(&parallel)?)
        } else {
            let y = run(&parallel)?;
            let x = run(&scalar)?;
            (x, y)
        };
        if x.1 != y.1 {
            return Err("benchmark full-state parity failed".into());
        }
        a.push(x.0);
        b.push(y.0);
    }
    a.sort_unstable();
    b.sort_unstable();
    println!("lhc-benchmark-v1\narch={} os={}\nbunches=2808 steps=128 samples=7 warmups=1\nrequested_workers=4 effective_workers={}\nscalar_median_ns={} parallel_median_ns={}\nfull_state_parity=pass\nNo universal speedup claim; timing includes scoped-thread overhead.",env::consts::ARCH,env::consts::OS,parallel.effective_workers(),a[3],b[3]);
    // Coarse-grained experiment: independent scenario clones, scalar per scenario.
    // This is a benchmark experiment, not a second replay format or auto policy.
    let jobs = 16usize;
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(4)
        .min(jobs);
    let batch = |parallel: bool| -> Result<(u128, Vec<Vec<u64>>), String> {
        let mut states = vec![scalar.clone(); jobs];
        let now = Instant::now();
        if parallel {
            std::thread::scope(|scope| -> Result<(), String> {
                let mut handles = Vec::new();
                for chunk in states.chunks_mut(jobs.div_ceil(workers)) {
                    handles.push(scope.spawn(move || -> Result<(), String> {
                        for r in chunk {
                            for _ in 0..128 {
                                r.step().map_err(|e| e.to_string())?;
                            }
                        }
                        Ok(())
                    }));
                }
                for handle in handles {
                    handle.join().map_err(|_| "scenario worker panicked")??;
                }
                Ok(())
            })?;
        } else {
            for r in &mut states {
                for _ in 0..128 {
                    r.step().map_err(|e| e.to_string())?;
                }
            }
        }
        let ns = now.elapsed().as_nanos();
        Ok((
            ns,
            states.iter().map(Runtime::canonical_state_bits).collect(),
        ))
    };
    batch(false)?;
    batch(true)?;
    let mut seq = Vec::new();
    let mut par = Vec::new();
    for i in 0..7 {
        let (a, b) = if i % 2 == 0 {
            (batch(false)?, batch(true)?)
        } else {
            let b = batch(true)?;
            let a = batch(false)?;
            (a, b)
        };
        if a.1 != b.1 {
            return Err("independent-scenario full-state parity failed".into());
        }
        seq.push(a.0);
        par.push(b.0);
    }
    seq.sort_unstable();
    par.sort_unstable();
    println!("scenario_jobs={jobs} scenario_workers={workers} steps_per_job=128\nscenario_serial_median_ns={} scenario_parallel_median_ns={}\nscenario_full_state_parity=pass",seq[3],par[3]);
    Ok(())
}
fn main_impl() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bench") if args.len() == 1 => bench(),
        Some("replay") if args.len() == 2 => {
            let (plan, expected) = parse_receipt(&read_bounded(&args[1])?)?;
            let runtime = execute(&plan)?;
            if state(&runtime) != expected {
                return Err("replay mismatch: full canonical state differs".into());
            }
            println!("replay verified: full canonical state matches");
            Ok(())
        }
        Some("run") => {
            let mut config = Config::default();
            let mut steps = 5000;
            let mut script = None;
            let mut seen = std::collections::HashSet::new();
            if !(args.len() - 1).is_multiple_of(2) {
                return Err("every run flag requires a value".into());
            }
            for pair in args[1..].chunks_exact(2) {
                if !seen.insert(&pair[0]) {
                    return Err(format!("duplicate flag: {}", pair[0]));
                }
                match pair[0].as_str() {
                    "--bunches" => config.bunches = number(&pair[1])?,
                    "--dt-ms" => config.dt_ms = number(&pair[1])?,
                    "--beta" => config.beta_star = number(&pair[1])?,
                    "--steps" => steps = number(&pair[1])?,
                    "--workers" => config.kernel = Kernel::Parallel(number(&pair[1])?),
                    "--script" => script = Some(pair[1].clone()),
                    _ => return Err(format!("unknown flag: {}", pair[0])),
                }
            }
            let commands = if let Some(path) = script {
                commands(&read_bounded(&path)?, steps)?
            } else {
                vec![(0, Command::Auto(true)), (0, Command::Inject)]
            };
            let plan = Plan {
                config,
                steps,
                commands,
            };
            let runtime = execute(&plan)?;
            eprintln!(
                "phase={:?} effective_workers={} fingerprint={:016x} (non-cryptographic)",
                runtime.snapshot().phase,
                runtime.effective_workers(),
                runtime.fingerprint()
            );
            print!("{}", receipt(&plan, &runtime));
            Ok(())
        }
        Some("--help") | None => {
            println!("lhc-runtime run [--bunches N] [--dt-ms N] [--beta F] [--steps N] [--workers N] [--script PATH]\nlhc-runtime replay RECEIPT\nlhc-runtime bench\nRun writes receipt to stdout. Scripts contain ordered STEP COMMAND lines; see docs/CPU-RUNTIME.md.");
            Ok(())
        }
        _ => Err("unknown command; use --help".into()),
    }
}
fn main() -> ExitCode {
    match main_impl() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
