//! A deterministic, dependency-free educational beam-cycle model for the CPU.
//!
//! Constants and the smooth ramp, logarithmic squeeze, luminosity and burn-off
//! equations are sourced from the repository's `index.html`. This is neither a
//! validated accelerator model nor an exact port of the animation. There are no
//! random events, live data, GPU operations, or display-frame dependencies.
//!
//! Injection deliberately does not simulate transfer-line geometry: one probe
//! bunch per beam is present immediately, followed by a 40 s probe hold and a
//! 45 s aggregate physics-fill hold. All configured bunches are then populated.
//! Dump clears both beams immediately, holds for 1 s, and ramps magnets down for
//! 1200 s. The other machine-time phase durations match the frontend constants.
//!
//! Each `step` advances exactly `dt_ms` active simulation milliseconds, splitting
//! at phase boundaries. Paused steps are exact no-ops. Collision integration is
//! an endpoint rectangular rule over each segment, with the frontend's explicit
//! burn-off update. Different tick sizes therefore need not give equal results.
//! Scalar and parallel kernels preserve operation/reduction order and give the
//! same bits on a fixed build/platform. Transcendental functions are not promised
//! bit-identical across different architectures, toolchains, or math libraries.

#![forbid(unsafe_code)]

use std::fmt;
use std::thread;

pub const MAX_BUNCHES: usize = 3564;
pub const MAX_WORKERS: usize = 256;
pub const MAX_DT_MS: u32 = 1000;
pub const INJECTION_ENERGY_GEV: f64 = 450.0;
pub const TOP_ENERGY_GEV: f64 = 6800.0;
pub const INJECTION_BETA_STAR_M: f64 = 11.0;
pub const PROTONS_PER_BUNCH: f64 = 1.15e11;
pub const PROTON_MASS_GEV: f64 = 0.938272;
pub const REVOLUTION_FREQUENCY_HZ: f64 = 11245.5;
pub const NORMALIZED_EMITTANCE_M: f64 = 2.5e-6;
pub const BUNCH_LENGTH_M: f64 = 0.075;
pub const HALF_CROSSING_ANGLE_RAD: f64 = 160e-6;
pub const INELASTIC_CROSS_SECTION_CM2: f64 = 80e-27;
pub const TOTAL_CROSS_SECTION_CM2: f64 = 111e-27;
pub const EFFECTIVE_COLLISION_IPS: f64 = 2.1;
pub const RESIDUAL_LIFETIME_S: f64 = 40.0 * 3600.0;
pub const INITIAL_FILL: u64 = 10421;
pub const CANONICAL_STATE_VERSION: u64 = 1;

const PROBE_PROTONS: f64 = 5e9;
const PROBE_MS: u64 = 40_000;
const PHYSICS_FILL_MS: u64 = 45_000;
const PREPARE_MS: u64 = 60_000;
const RAMP_MS: u64 = 1_200_000;
const FLAT_MS: u64 = 60_000;
const SQUEEZE_MS: u64 = 600_000;
const ADJUST_MS: u64 = 120_000;
const STABLE_MS: u64 = 6 * 3600 * 1000;
const DUMP_MS: u64 = 1_000;
const RAMP_DOWN_MS: u64 = 1_200_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kernel {
    Scalar,
    /// Independent bunch updates on scoped CPU threads. Reductions remain serial.
    /// The requested count is capped to hardware, bunch count, and MAX_WORKERS.
    Parallel(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub bunches: usize,
    pub dt_ms: u32,
    pub beta_star: f64,
    pub kernel: Kernel,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bunches: 2808,
            dt_ms: 1000,
            beta_star: 0.3,
            kernel: Kernel::Scalar,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), Error> {
        if !(1..=MAX_BUNCHES).contains(&self.bunches) {
            return Err(Error::InvalidConfig("bunches must be in 1..=3564"));
        }
        if !(1..=MAX_DT_MS).contains(&self.dt_ms) {
            return Err(Error::InvalidConfig("dt_ms must be in 1..=1000"));
        }
        if !self.beta_star.is_finite() || !(0.3..=1.0).contains(&self.beta_star) {
            return Err(Error::InvalidConfig(
                "beta_star must be finite and in 0.3..=1.0 metres",
            ));
        }
        if self.kernel == Kernel::Parallel(0) {
            return Err(Error::InvalidConfig(
                "parallel worker count must be positive",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum Phase {
    NoBeam = 0,
    InjectionProbe = 1,
    InjectionPhysics = 2,
    ReadyToRamp = 3,
    PrepareRamp = 4,
    Ramp = 5,
    FlatTop = 6,
    Squeeze = 7,
    ReadyToCollide = 8,
    Adjust = 9,
    StableBeams = 10,
    BeamDump = 11,
    RampDown = 12,
}

impl Phase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoBeam => "no-beam",
            Self::InjectionProbe => "injection-probe",
            Self::InjectionPhysics => "injection-physics",
            Self::ReadyToRamp => "ready-to-ramp",
            Self::PrepareRamp => "prepare-ramp",
            Self::Ramp => "ramp",
            Self::FlatTop => "flat-top",
            Self::Squeeze => "squeeze",
            Self::ReadyToCollide => "ready-to-collide",
            Self::Adjust => "adjust",
            Self::StableBeams => "stable-beams",
            Self::BeamDump => "beam-dump",
            Self::RampDown => "ramp-down",
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Inject,
    Ramp,
    Squeeze,
    Collide,
    Dump,
    Pause,
    Resume,
    /// Restore the initial state, including clock, fill, pause, and auto flags.
    Reset,
    /// Enable/disable sequencing. A fresh NoBeam state still needs Inject.
    Auto(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidConfig(&'static str),
    IllegalCommand { command: Command, phase: Phase },
    ClockOverflow,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(message) => f.write_str(message),
            Self::IllegalCommand { command, phase } => {
                write!(f, "command {command:?} is not allowed in {phase}")
            }
            Self::ClockOverflow => f.write_str("simulation clock or tick counter would overflow"),
        }
    }
}
impl std::error::Error for Error {}

/// Scalar observables. Kernel choice is intentionally absent for parity checks.
/// `luminosity_cm2_s` is the most recent integration segment's pre-burn value,
/// zero when beams are not colliding. Integrated quantities are per fill.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub elapsed_ms: u64,
    pub phase: Phase,
    pub phase_elapsed_ms: u64,
    pub fill: u64,
    pub paused: bool,
    pub auto: bool,
    pub energy_gev: f64,
    pub beta_star_m: f64,
    pub luminosity_cm2_s: f64,
    pub integrated_luminosity_fb: f64,
    pub collisions: f64,
    pub bunches_beam1: usize,
    pub bunches_beam2: usize,
    pub intensity_beam1: f64,
    pub intensity_beam2: f64,
    pub collision_fraction: f64,
}

/// Structure-of-arrays state: one contiguous intensity array for each beam.
/// Arrays always have `config.bunches` elements; inactive entries are zero.
#[derive(Clone, Debug)]
pub struct Runtime {
    config: Config,
    workers: usize,
    tick: u64,
    elapsed_ms: u64,
    phase: Phase,
    phase_elapsed_ms: u64,
    fill: u64,
    paused: bool,
    auto: bool,
    energy: f64,
    beta_star: f64,
    lumi: f64,
    integrated_lumi: f64,
    collisions: f64,
    collision_fraction: f64,
    ramp_down_from: f64,
    active_bunches: usize,
    beam1: Vec<f64>,
    beam2: Vec<f64>,
}

impl Runtime {
    pub fn new(config: Config) -> Result<Self, Error> {
        config.validate()?;
        let hardware = thread::available_parallelism().map_or(1, usize::from);
        let workers = match config.kernel {
            Kernel::Scalar => 1,
            Kernel::Parallel(requested) => {
                requested.min(config.bunches).min(hardware).min(MAX_WORKERS)
            }
        };
        Ok(Self {
            config,
            workers,
            tick: 0,
            elapsed_ms: 0,
            phase: Phase::NoBeam,
            phase_elapsed_ms: 0,
            fill: INITIAL_FILL,
            paused: false,
            auto: false,
            energy: 0.0,
            beta_star: INJECTION_BETA_STAR_M,
            lumi: 0.0,
            integrated_lumi: 0.0,
            collisions: 0.0,
            collision_fraction: 0.0,
            ramp_down_from: 0.0,
            active_bunches: 0,
            beam1: vec![0.0; config.bunches],
            beam2: vec![0.0; config.bunches],
        })
    }

    /// Execution width of collision updates (one means caller-thread execution).
    /// No threads are created outside collision updates.
    pub fn effective_workers(&self) -> usize {
        self.workers
    }

    pub fn intensities(&self) -> (&[f64], &[f64]) {
        (&self.beam1, &self.beam2)
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            tick: self.tick,
            elapsed_ms: self.elapsed_ms,
            phase: self.phase,
            phase_elapsed_ms: self.phase_elapsed_ms,
            fill: self.fill,
            paused: self.paused,
            auto: self.auto,
            energy_gev: self.energy,
            beta_star_m: self.beta_star,
            luminosity_cm2_s: self.lumi,
            integrated_luminosity_fb: self.integrated_lumi,
            collisions: self.collisions,
            bunches_beam1: self.active_bunches,
            bunches_beam2: self.active_bunches,
            intensity_beam1: ordered_sum(&self.beam1),
            intensity_beam2: ordered_sum(&self.beam2),
            collision_fraction: self.collision_fraction,
        }
    }

    /// Invalid commands return an error without changing any state. Pause and
    /// Resume are idempotent; valid machine controls also work while paused.
    pub fn command(&mut self, command: Command) -> Result<(), Error> {
        match command {
            Command::Inject if self.phase == Phase::NoBeam => self.inject(),
            Command::Ramp if self.phase == Phase::ReadyToRamp => self.set_phase(Phase::PrepareRamp),
            Command::Squeeze if self.phase == Phase::FlatTop => self.set_phase(Phase::Squeeze),
            Command::Collide if self.phase == Phase::ReadyToCollide => {
                self.set_phase(Phase::Adjust)
            }
            Command::Dump
                if !matches!(
                    self.phase,
                    Phase::NoBeam | Phase::BeamDump | Phase::RampDown
                ) =>
            {
                self.dump()
            }
            Command::Pause => self.paused = true,
            Command::Resume => self.paused = false,
            Command::Auto(enabled) => self.auto = enabled,
            Command::Reset => {
                // Keep the original execution width if hardware availability changed.
                let workers = self.workers;
                *self = Self::new(self.config)?;
                self.workers = workers;
            }
            _ => {
                return Err(Error::IllegalCommand {
                    command,
                    phase: self.phase,
                })
            }
        }
        Ok(())
    }

    /// Advance one fixed tick. Clock-overflow errors leave state unchanged.
    pub fn step(&mut self) -> Result<(), Error> {
        if self.paused {
            return Ok(());
        }
        let next_tick = self.tick.checked_add(1).ok_or(Error::ClockOverflow)?;
        let next_elapsed = self
            .elapsed_ms
            .checked_add(u64::from(self.config.dt_ms))
            .ok_or(Error::ClockOverflow)?;
        // This explicit guard also covers long manual holds before a transition.
        self.phase_elapsed_ms
            .checked_add(u64::from(self.config.dt_ms))
            .ok_or(Error::ClockOverflow)?;
        let mut remaining = u64::from(self.config.dt_ms);
        while remaining > 0 {
            self.transition_if_due();
            let interval = self.phase_duration().map_or(remaining, |duration| {
                remaining.min(duration - self.phase_elapsed_ms)
            });
            // transition_if_due consumes every zero-time transition before here.
            debug_assert!(interval > 0);
            self.phase_elapsed_ms += interval;
            self.advance_equations();
            self.advance_collisions(interval as f64 / 1000.0);
            remaining -= interval;
            self.transition_if_due();
        }
        self.tick = next_tick;
        self.elapsed_ms = next_elapsed;
        Ok(())
    }

    /// Versioned canonical words for complete deterministic-state comparisons.
    /// Encoding order: version, physical config, clocks/mode/flags, all scalar
    /// dynamics including ramp-down origin and active count, then beam arrays.
    /// Floating values are their exact IEEE-754 bits; no Debug/text rounding.
    /// Execution policy is excluded; this is a comparison format, not a loader.
    pub fn canonical_state_bits(&self) -> Vec<u64> {
        let mut words = Vec::with_capacity(22 + 2 * self.config.bunches);
        words.extend([
            CANONICAL_STATE_VERSION,
            self.config.bunches as u64,
            u64::from(self.config.dt_ms),
            self.config.beta_star.to_bits(),
            self.tick,
            self.elapsed_ms,
            self.phase as u64,
            self.phase_elapsed_ms,
            self.fill,
            u64::from(self.paused),
            u64::from(self.auto),
            self.energy.to_bits(),
            self.beta_star.to_bits(),
            self.lumi.to_bits(),
            self.integrated_lumi.to_bits(),
            self.collisions.to_bits(),
            self.collision_fraction.to_bits(),
            self.ramp_down_from.to_bits(),
            self.active_bunches as u64,
        ]);
        words.extend(self.beam1.iter().map(|value| value.to_bits()));
        words.extend(self.beam2.iter().map(|value| value.to_bits()));
        words
    }

    /// Stable FNV-1a over canonical little-endian words; not cryptographic proof.
    /// Compare canonical_state_bits for authoritative full-state parity.
    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        for word in self.canonical_state_bits() {
            for byte in word.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        hash
    }

    fn set_phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_elapsed_ms = 0;
        if !matches!(phase, Phase::Adjust | Phase::StableBeams) {
            self.lumi = 0.0;
        }
    }

    fn inject(&mut self) {
        self.beam1.fill(0.0);
        self.beam2.fill(0.0);
        self.beam1[0] = PROBE_PROTONS;
        self.beam2[0] = PROBE_PROTONS;
        self.active_bunches = 1;
        self.energy = INJECTION_ENERGY_GEV;
        self.beta_star = INJECTION_BETA_STAR_M;
        self.collision_fraction = 0.0;
        self.integrated_lumi = 0.0;
        self.collisions = 0.0;
        self.ramp_down_from = 0.0;
        self.set_phase(Phase::InjectionProbe);
    }

    fn dump(&mut self) {
        self.ramp_down_from = self.energy;
        self.beam1.fill(0.0);
        self.beam2.fill(0.0);
        self.active_bunches = 0;
        self.collision_fraction = 0.0;
        self.set_phase(Phase::BeamDump);
    }

    fn phase_duration(&self) -> Option<u64> {
        match self.phase {
            Phase::InjectionProbe => Some(PROBE_MS),
            Phase::InjectionPhysics => Some(PHYSICS_FILL_MS),
            Phase::PrepareRamp => Some(PREPARE_MS),
            Phase::Ramp => Some(RAMP_MS),
            Phase::FlatTop if self.auto => Some(FLAT_MS),
            Phase::Squeeze => Some(SQUEEZE_MS),
            Phase::Adjust => Some(ADJUST_MS),
            Phase::StableBeams if self.auto => Some(STABLE_MS),
            Phase::BeamDump => Some(DUMP_MS),
            Phase::RampDown => Some(RAMP_DOWN_MS),
            _ => None,
        }
    }

    fn transition_if_due(&mut self) {
        loop {
            let due = self
                .phase_duration()
                .is_some_and(|duration| self.phase_elapsed_ms >= duration);
            match self.phase {
                Phase::InjectionProbe if due => self.set_phase(Phase::InjectionPhysics),
                Phase::InjectionPhysics if due => {
                    self.beam1.fill(PROTONS_PER_BUNCH);
                    self.beam2.fill(PROTONS_PER_BUNCH);
                    self.active_bunches = self.config.bunches;
                    self.set_phase(Phase::ReadyToRamp);
                }
                Phase::ReadyToRamp if self.auto => self.set_phase(Phase::PrepareRamp),
                Phase::PrepareRamp if due => self.set_phase(Phase::Ramp),
                Phase::Ramp if due => {
                    self.energy = TOP_ENERGY_GEV;
                    self.set_phase(Phase::FlatTop);
                }
                Phase::FlatTop if due => self.set_phase(Phase::Squeeze),
                Phase::Squeeze if due => {
                    self.beta_star = self.config.beta_star;
                    self.set_phase(Phase::ReadyToCollide);
                }
                Phase::ReadyToCollide if self.auto => self.set_phase(Phase::Adjust),
                Phase::Adjust if due => {
                    self.collision_fraction = 1.0;
                    self.set_phase(Phase::StableBeams);
                }
                Phase::StableBeams if due => self.dump(),
                Phase::BeamDump if due => self.set_phase(Phase::RampDown),
                Phase::RampDown if due => {
                    self.energy = 0.0;
                    self.beta_star = INJECTION_BETA_STAR_M;
                    self.ramp_down_from = 0.0;
                    self.fill += 1;
                    self.set_phase(Phase::NoBeam);
                    if self.auto {
                        self.inject();
                    }
                }
                _ => break,
            }
        }
    }

    fn advance_equations(&mut self) {
        match self.phase {
            Phase::Ramp => {
                let progress = smooth(self.phase_elapsed_ms as f64 / RAMP_MS as f64);
                self.energy =
                    INJECTION_ENERGY_GEV + (TOP_ENERGY_GEV - INJECTION_ENERGY_GEV) * progress;
            }
            Phase::Squeeze => {
                let progress = smooth(self.phase_elapsed_ms as f64 / SQUEEZE_MS as f64);
                self.beta_star = (INJECTION_BETA_STAR_M.ln()
                    + (self.config.beta_star.ln() - INJECTION_BETA_STAR_M.ln()) * progress)
                    .exp();
            }
            Phase::Adjust => {
                self.collision_fraction = smooth(self.phase_elapsed_ms as f64 / ADJUST_MS as f64)
            }
            Phase::RampDown => {
                let progress = smooth(self.phase_elapsed_ms as f64 / RAMP_DOWN_MS as f64);
                self.energy = INJECTION_ENERGY_GEV
                    + (self.ramp_down_from - INJECTION_ENERGY_GEV) * (1.0 - progress);
            }
            _ => {}
        }
    }

    fn advance_collisions(&mut self, dt_s: f64) {
        if !matches!(self.phase, Phase::Adjust | Phase::StableBeams) || self.active_bunches == 0 {
            self.lumi = 0.0;
            return;
        }
        let bunches = self.active_bunches;
        let mean = (ordered_sum(&self.beam1) + ordered_sum(&self.beam2)) / (2.0 * bunches as f64);
        let gamma = self.energy / PROTON_MASS_GEV;
        let sigma = (NORMALIZED_EMITTANCE_M * self.beta_star / gamma).sqrt();
        let crossing_ratio = HALF_CROSSING_ANGLE_RAD * BUNCH_LENGTH_M / sigma;
        let geometric_factor = 1.0 / (1.0 + crossing_ratio * crossing_ratio).sqrt();
        self.lumi = mean * mean * bunches as f64 * REVOLUTION_FREQUENCY_HZ * gamma
            / (4.0 * std::f64::consts::PI * NORMALIZED_EMITTANCE_M * self.beta_star)
            * geometric_factor
            * 1e-4
            * self.collision_fraction;
        self.collisions += self.lumi * INELASTIC_CROSS_SECTION_CM2 * EFFECTIVE_COLLISION_IPS * dt_s;
        self.integrated_lumi += self.lumi * dt_s / 1e39;
        let per_bunch =
            TOTAL_CROSS_SECTION_CM2 * self.lumi * EFFECTIVE_COLLISION_IPS * dt_s / bunches as f64;
        let lifetime = (-dt_s / RESIDUAL_LIFETIME_S).exp();
        update_intensities(
            &mut self.beam1[..bunches],
            &mut self.beam2[..bunches],
            lifetime,
            per_bunch,
            self.workers,
        );
    }
}

fn smooth(t: f64) -> f64 {
    if t <= 0.0 {
        0.0
    } else if t >= 1.0 {
        1.0
    } else {
        t * t * (3.0 - 2.0 * t)
    }
}

/// Explicit left fold: never parallelize or change the grouping of this sum.
fn ordered_sum(values: &[f64]) -> f64 {
    values.iter().fold(0.0, |sum, value| sum + value)
}

fn update_pair(beam1: &mut [f64], beam2: &mut [f64], lifetime: f64, loss: f64) {
    for value in beam1.iter_mut().chain(beam2.iter_mut()) {
        *value = (*value * lifetime - loss).max(0.0);
    }
}

fn update_intensities(
    beam1: &mut [f64],
    beam2: &mut [f64],
    lifetime: f64,
    loss: f64,
    workers: usize,
) {
    if workers <= 1 {
        update_pair(beam1, beam2, lifetime, loss);
        return;
    }
    debug_assert_eq!(beam1.len(), beam2.len());
    debug_assert!(workers <= beam1.len());
    let base = beam1.len() / workers;
    let extra = beam1.len() % workers;
    thread::scope(|scope| {
        let (mut left1, mut left2) = (beam1, beam2);
        for index in 0..workers {
            let size = base + usize::from(index < extra);
            let (part1, tail1) = left1.split_at_mut(size);
            let (part2, tail2) = left2.split_at_mut(size);
            left1 = tail1;
            left2 = tail2;
            scope.spawn(move || update_pair(part1, part2, lifetime, loss));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(bunches: usize) -> Runtime {
        Runtime::new(Config {
            bunches,
            ..Config::default()
        })
        .unwrap()
    }
    fn steps(runtime: &mut Runtime, count: usize) {
        for _ in 0..count {
            runtime.step().unwrap();
        }
    }
    fn stable(mut runtime: Runtime) -> Runtime {
        runtime.command(Command::Auto(true)).unwrap();
        runtime.command(Command::Inject).unwrap();
        for _ in 0..10_000 {
            if runtime.phase == Phase::StableBeams {
                return runtime;
            }
            runtime.step().unwrap();
        }
        panic!("did not reach stable beams");
    }
    fn assert_finite_bounded(runtime: &Runtime) {
        let s = runtime.snapshot();
        for value in [
            s.energy_gev,
            s.beta_star_m,
            s.luminosity_cm2_s,
            s.integrated_luminosity_fb,
            s.collisions,
            s.intensity_beam1,
            s.intensity_beam2,
            s.collision_fraction,
        ] {
            assert!(value.is_finite() && value >= 0.0, "invalid {value}");
        }
        assert!(s.energy_gev <= TOP_ENERGY_GEV);
        assert!(
            s.beta_star_m >= runtime.config.beta_star && s.beta_star_m <= INJECTION_BETA_STAR_M
        );
        assert!(s.collision_fraction <= 1.0);
        assert!(s.bunches_beam1 <= runtime.config.bunches);
        for value in runtime.beam1.iter().chain(&runtime.beam2) {
            assert!((0.0..=PROTONS_PER_BUNCH).contains(value));
        }
        assert_eq!(runtime.beam1, runtime.beam2);
    }

    #[test]
    fn rejects_invalid_configuration() {
        for bunches in [0, MAX_BUNCHES + 1, usize::MAX] {
            assert!(Runtime::new(Config {
                bunches,
                ..Config::default()
            })
            .is_err());
        }
        for dt_ms in [0, MAX_DT_MS + 1, u32::MAX] {
            assert!(Runtime::new(Config {
                dt_ms,
                ..Config::default()
            })
            .is_err());
        }
        for beta_star in [
            0.0,
            -0.3,
            0.299,
            1.001,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            assert!(Runtime::new(Config {
                beta_star,
                ..Config::default()
            })
            .is_err());
        }
        assert!(Runtime::new(Config {
            kernel: Kernel::Parallel(0),
            ..Config::default()
        })
        .is_err());
        for bunches in [1, MAX_BUNCHES] {
            for dt_ms in [1, MAX_DT_MS] {
                for beta_star in [0.3, 1.0] {
                    assert!(Runtime::new(Config {
                        bunches,
                        dt_ms,
                        beta_star,
                        kernel: Kernel::Scalar
                    })
                    .is_ok());
                }
            }
        }
    }

    #[test]
    fn illegal_commands_are_atomic() {
        let mut r = runtime(12);
        for command in [
            Command::Ramp,
            Command::Squeeze,
            Command::Collide,
            Command::Dump,
        ] {
            let before = r.canonical_state_bits();
            assert!(matches!(
                r.command(command),
                Err(Error::IllegalCommand { .. })
            ));
            assert_eq!(r.canonical_state_bits(), before);
        }
        r.command(Command::Inject).unwrap();
        for command in [
            Command::Inject,
            Command::Ramp,
            Command::Squeeze,
            Command::Collide,
        ] {
            let before = r.canonical_state_bits();
            assert!(r.command(command).is_err());
            assert_eq!(r.canonical_state_bits(), before);
        }
    }

    #[test]
    fn manual_controls_enforce_ready_boundaries() {
        let mut r = runtime(12);
        r.command(Command::Inject).unwrap();
        steps(&mut r, 84);
        assert_eq!(r.phase, Phase::InjectionPhysics);
        assert!(r.command(Command::Ramp).is_err());
        steps(&mut r, 1);
        assert_eq!(r.phase, Phase::ReadyToRamp);
        assert_eq!(r.active_bunches, 12);
        r.command(Command::Ramp).unwrap();
        steps(&mut r, 1260);
        assert_eq!(r.phase, Phase::FlatTop);
        r.command(Command::Squeeze).unwrap();
        steps(&mut r, 599);
        assert!(r.command(Command::Collide).is_err());
        steps(&mut r, 1);
        assert_eq!(r.beta_star, 0.3);
        r.command(Command::Collide).unwrap();
        steps(&mut r, 120);
        assert_eq!(r.phase, Phase::StableBeams);
        assert!(r.lumi > 0.0 && r.integrated_lumi > 0.0 && r.collisions > 0.0);
        let before = r.snapshot();
        steps(&mut r, 10);
        assert!(r.collisions > before.collisions);
        assert!(ordered_sum(&r.beam1) < before.intensity_beam1);
        assert_finite_bounded(&r);
    }

    #[test]
    fn pause_freezes_every_bit_and_reset_is_exact() {
        let original = runtime(12);
        let mut r = stable(original.clone());
        r.command(Command::Pause).unwrap();
        let paused = r.canonical_state_bits();
        steps(&mut r, 10);
        assert_eq!(r.canonical_state_bits(), paused);
        r.command(Command::Pause).unwrap();
        assert_eq!(r.canonical_state_bits(), paused);
        r.command(Command::Resume).unwrap();
        r.step().unwrap();
        assert_ne!(r.canonical_state_bits(), paused);
        r.command(Command::Reset).unwrap();
        assert_eq!(r.canonical_state_bits(), original.canonical_state_bits());
        r.command(Command::Reset).unwrap();
        assert_eq!(r.snapshot(), original.snapshot());
    }

    #[test]
    fn dump_clears_beams_and_retains_integrated_counts_until_next_fill() {
        let mut r = stable(runtime(12));
        r.command(Command::Auto(false)).unwrap();
        let counts = (r.collisions, r.integrated_lumi);
        r.command(Command::Dump).unwrap();
        assert_eq!(r.active_bunches, 0);
        assert!(r.beam1.iter().chain(&r.beam2).all(|value| *value == 0.0));
        assert_eq!(r.lumi, 0.0);
        assert_eq!(r.collision_fraction, 0.0);
        assert!(r.command(Command::Dump).is_err());
        steps(&mut r, 1201);
        assert_eq!(r.phase, Phase::NoBeam);
        assert_eq!(r.energy, 0.0);
        assert_eq!(r.fill, INITIAL_FILL + 1);
        assert_eq!((r.collisions, r.integrated_lumi), counts);
        r.command(Command::Inject).unwrap();
        assert_eq!((r.collisions, r.integrated_lumi), (0.0, 0.0));
    }

    #[test]
    fn automatic_cycle_is_bounded_and_restarts() {
        let mut r = runtime(1);
        r.command(Command::Auto(true)).unwrap();
        r.step().unwrap();
        assert_eq!(r.phase, Phase::NoBeam);
        r.command(Command::Inject).unwrap();
        let total_seconds = (PROBE_MS
            + PHYSICS_FILL_MS
            + PREPARE_MS
            + RAMP_MS
            + FLAT_MS
            + SQUEEZE_MS
            + ADJUST_MS
            + STABLE_MS
            + DUMP_MS
            + RAMP_DOWN_MS)
            / 1000;
        for _ in 0..total_seconds {
            r.step().unwrap();
            assert_finite_bounded(&r);
        }
        assert_eq!(r.fill, INITIAL_FILL + 1);
        assert_eq!(r.phase, Phase::InjectionProbe);
        assert_eq!(r.phase_elapsed_ms, 0);
    }

    #[test]
    fn fractional_tick_preserves_transition_remainder() {
        let mut r = Runtime::new(Config {
            bunches: 1,
            dt_ms: 333,
            ..Config::default()
        })
        .unwrap();
        r.command(Command::Inject).unwrap();
        steps(&mut r, 121);
        assert_eq!(r.elapsed_ms, 40_293);
        assert_eq!(r.phase, Phase::InjectionPhysics);
        assert_eq!(r.phase_elapsed_ms, 293);
        steps(&mut r, 135);
        assert_eq!(r.elapsed_ms, 85_248);
        assert_eq!(r.phase, Phase::ReadyToRamp);
        assert_eq!(r.phase_elapsed_ms, 248);
    }

    #[test]
    fn enabling_auto_after_a_long_manual_hold_transitions_without_underflow() {
        let mut r = stable(runtime(1));
        r.command(Command::Auto(false)).unwrap();
        steps(&mut r, STABLE_MS as usize / 1000 + 1);
        r.command(Command::Auto(true)).unwrap();
        r.step().unwrap();
        assert_eq!(r.phase, Phase::RampDown);
        assert_eq!(r.phase_elapsed_ms, 0);
    }

    #[test]
    fn independent_runs_have_identical_full_state() {
        let mut a = stable(runtime(127));
        let mut b = stable(runtime(127));
        for _ in 0..100 {
            a.step().unwrap();
            b.step().unwrap();
            assert_eq!(a.snapshot(), b.snapshot());
            assert_eq!(a.canonical_state_bits(), b.canonical_state_bits());
            assert_eq!(a.fingerprint(), b.fingerprint());
        }
    }

    #[test]
    fn parallel_and_scalar_match_every_array_bit_with_tails() {
        for bunches in [1, 2, 7, 13, 127, 2808, MAX_BUNCHES] {
            let initial = stable(runtime(bunches));
            for requested in [1, 2, 4, 7] {
                let mut scalar = initial.clone();
                let mut parallel = initial.clone();
                let configured = Runtime::new(Config {
                    bunches,
                    kernel: Kernel::Parallel(requested),
                    ..Config::default()
                })
                .unwrap();
                parallel.config.kernel = Kernel::Parallel(requested);
                parallel.workers = configured.effective_workers();
                for _ in 0..4 {
                    scalar.step().unwrap();
                    parallel.step().unwrap();
                    assert_eq!(parallel.snapshot(), scalar.snapshot());
                    assert_eq!(
                        parallel.canonical_state_bits(),
                        scalar.canonical_state_bits()
                    );
                    assert_eq!(parallel.intensities(), scalar.intensities());
                }
            }
        }
    }

    #[test]
    fn forced_partition_kernel_matches_scalar_on_uneven_slices() {
        // Exercise actual partitioning even on single-CPU CI, without claiming
        // that Runtime exceeds its public hardware cap.
        let original: Vec<f64> = (0..127).map(|i| 1.0e9 + i as f64 * 12345.0).collect();
        for workers in [1, 2, 4, 7] {
            let (mut a, mut b, mut c, mut d) = (
                original.clone(),
                original.clone(),
                original.clone(),
                original.clone(),
            );
            update_intensities(&mut a, &mut b, 0.9999, 80.0, 1);
            update_intensities(&mut c, &mut d, 0.9999, 80.0, workers);
            assert_eq!(a, c);
            assert_eq!(b, d);
        }
    }

    #[test]
    fn worker_count_is_bounded() {
        let r = Runtime::new(Config {
            bunches: 13,
            kernel: Kernel::Parallel(usize::MAX),
            ..Config::default()
        })
        .unwrap();
        assert!((1..=13).contains(&r.effective_workers()));
        assert!(r.effective_workers() <= MAX_WORKERS);
        assert!(r.effective_workers() <= thread::available_parallelism().map_or(1, usize::from));
    }

    #[test]
    fn hidden_state_and_configuration_are_in_canonical_encoding() {
        let a = stable(runtime(12));
        let mut b = a.clone();
        b.beam1[0] += 1.0;
        assert_ne!(a.canonical_state_bits(), b.canonical_state_bits());
        b = a.clone();
        b.ramp_down_from = 123.0;
        assert_ne!(a.canonical_state_bits(), b.canonical_state_bits());
        b = a.clone();
        b.config.dt_ms = 999;
        assert_ne!(a.canonical_state_bits(), b.canonical_state_bits());
        b = a.clone();
        b.config.beta_star = 0.6;
        assert_ne!(a.canonical_state_bits(), b.canonical_state_bits());
        b = a.clone();
        b.config.kernel = Kernel::Parallel(7);
        assert_eq!(a.canonical_state_bits(), b.canonical_state_bits());
    }

    #[test]
    fn clock_overflow_is_atomic_and_pause_remains_safe() {
        let mut r = runtime(1);
        r.elapsed_ms = u64::MAX;
        let before = r.canonical_state_bits();
        assert_eq!(r.step(), Err(Error::ClockOverflow));
        assert_eq!(r.canonical_state_bits(), before);
        r.command(Command::Pause).unwrap();
        assert!(r.step().is_ok());
    }

    #[test]
    fn burn_off_clamps_and_never_creates_intensity() {
        let mut a = [0.0, 1.0, 10.0];
        let mut b = a;
        update_intensities(&mut a, &mut b, 0.99, 5.0, 1);
        assert_eq!(a, [0.0, 0.0, 4.9]);
        assert_eq!(a, b);
    }

    #[test]
    fn source_javascript_luminosity_vector_conforms() {
        // Independently evaluated from index.html lumiAt in Node, using
        // nColl=2808, E=6800 GeV, betaStar=0.3 m, Nb=1.15e11.
        // This checks the equation, not parity with the visual lifecycle.
        let expected = 2.076566873251595e34;
        let mut r = runtime(2808);
        r.beam1.fill(PROTONS_PER_BUNCH);
        r.beam2.fill(PROTONS_PER_BUNCH);
        r.active_bunches = 2808;
        r.phase = Phase::StableBeams;
        r.energy = TOP_ENERGY_GEV;
        r.beta_star = 0.3;
        r.collision_fraction = 1.0;
        r.advance_collisions(1.0);
        assert!((r.lumi - expected).abs() / expected <= 1e-14);
        assert_eq!(r.integrated_lumi, r.lumi / 1e39);
        assert_eq!(
            r.collisions,
            r.lumi * INELASTIC_CROSS_SECTION_CM2 * EFFECTIVE_COLLISION_IPS
        );
        assert_finite_bounded(&r);
    }

    #[test]
    fn ramp_and_squeeze_are_monotonic_and_hit_endpoints() {
        let mut r = runtime(1);
        r.command(Command::Inject).unwrap();
        steps(&mut r, 85);
        r.command(Command::Ramp).unwrap();
        steps(&mut r, 60);
        let mut previous = r.energy;
        for _ in 0..1200 {
            r.step().unwrap();
            assert!(r.energy >= previous && r.energy <= TOP_ENERGY_GEV);
            previous = r.energy;
        }
        assert_eq!(r.energy, TOP_ENERGY_GEV);
        r.command(Command::Squeeze).unwrap();
        let mut previous = r.beta_star;
        for _ in 0..600 {
            r.step().unwrap();
            assert!(r.beta_star <= previous && r.beta_star >= r.config.beta_star);
            previous = r.beta_star;
        }
        assert_eq!(r.beta_star, r.config.beta_star);
        assert_eq!(r.lumi, 0.0);
        assert_eq!(r.collisions, 0.0);
    }

    #[test]
    fn early_dump_cancels_injection_and_uses_current_energy() {
        let mut r = runtime(MAX_BUNCHES);
        r.command(Command::Inject).unwrap();
        r.command(Command::Dump).unwrap();
        steps(&mut r, 1201);
        assert_eq!(r.phase, Phase::NoBeam);
        assert_eq!(r.active_bunches, 0);
        assert!(r.beam1.iter().chain(&r.beam2).all(|value| *value == 0.0));
        assert_eq!(r.collisions, 0.0);
        assert_finite_bounded(&r);
    }
}
