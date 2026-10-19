import Std

/-! Discrete projection of runtime/src/lib.rs. No floating-point dynamics. -/
namespace LHC

inductive Phase where
  | noBeam | injectionProbe | injectionPhysics | readyToRamp | prepareRamp
  | ramp | flatTop | squeeze | readyToCollide | adjust | stableBeams | beamDump | rampDown
  deriving DecidableEq, Repr

def Phase.code : Phase → Nat
  | .noBeam => 0 | .injectionProbe => 1 | .injectionPhysics => 2
  | .readyToRamp => 3 | .prepareRamp => 4 | .ramp => 5 | .flatTop => 6
  | .squeeze => 7 | .readyToCollide => 8 | .adjust => 9 | .stableBeams => 10
  | .beamDump => 11 | .rampDown => 12

inductive Command where
  | inject | ramp | squeeze | collide | dump | pause | resume | reset | auto (enabled : Bool)
  deriving DecidableEq, Repr

structure Config where
  bunches : Nat
  dtMs : Nat
  deriving DecidableEq, Repr

def Config.Valid (c : Config) : Prop :=
  1 ≤ c.bunches ∧ c.bunches ≤ 3564 ∧ 1 ≤ c.dtMs ∧ c.dtMs ≤ 1000

structure State where
  phase : Phase := .noBeam
  phaseMs : Nat := 0
  tick : Nat := 0
  elapsedMs : Nat := 0
  fill : Nat := 10421
  paused : Bool := false
  auto : Bool := false
  active : Nat := 0
  deriving DecidableEq, Repr

def initial : State := {}

def setPhase (s : State) (p : Phase) : State := { s with phase := p, phaseMs := 0 }
def inject (s : State) : State := setPhase { s with active := 1 } .injectionProbe
def dump (s : State) : State := setPhase { s with active := 0 } .beamDump

def allowed (p : Phase) : Command → Bool
  | .inject => p == .noBeam
  | .ramp => p == .readyToRamp
  | .squeeze => p == .flatTop
  | .collide => p == .readyToCollide
  | .dump => !(p == .noBeam || p == .beamDump || p == .rampDown)
  | _ => true

/-- The Boolean reports success; a rejected command retains the entire projection. -/
def command (s : State) (c : Command) : Bool × State :=
  if allowed s.phase c then
    (true, match c with
      | .inject => inject s
      | .ramp => setPhase s .prepareRamp
      | .squeeze => setPhase s .squeeze
      | .collide => setPhase s .adjust
      | .dump => dump s
      | .pause => { s with paused := true }
      | .resume => { s with paused := false }
      | .reset => initial
      | .auto enabled => { s with auto := enabled })
  else (false, s)

def duration (p : Phase) (auto : Bool) : Option Nat :=
  match p with
  | .injectionProbe => some 40000
  | .injectionPhysics => some 45000
  | .prepareRamp => some 60000
  | .ramp => some 1200000
  | .flatTop => if auto then some 60000 else none
  | .squeeze => some 600000
  | .adjust => some 120000
  | .stableBeams => if auto then some 21600000 else none
  | .beamDump => some 1000
  | .rampDown => some 1200000
  | _ => none

def due (s : State) : Bool :=
  (duration s.phase s.auto).any (fun d => d ≤ s.phaseMs)

/-- One iteration of transition_if_due, including its zero-time ready edges. -/
def transition (c : Config) (s : State) : State :=
  match s.phase with
  | .readyToRamp => if s.auto then setPhase s .prepareRamp else s
  | .readyToCollide => if s.auto then setPhase s .adjust else s
  | _ => if due s then
      match s.phase with
      | .injectionProbe => setPhase s .injectionPhysics
      | .injectionPhysics => setPhase { s with active := c.bunches } .readyToRamp
      | .prepareRamp => setPhase s .ramp
      | .ramp => setPhase s .flatTop
      | .flatTop => setPhase s .squeeze
      | .squeeze => setPhase s .readyToCollide
      | .adjust => setPhase s .stableBeams
      | .stableBeams => dump s
      | .beamDump => setPhase s .rampDown
      | .rampDown =>
          let next := setPhase { s with fill := s.fill + 1 } .noBeam
          if s.auto then inject next else next
      | _ => s
    else s

/-- At most two edges are needed: a timed edge followed by a ready edge. -/
def normalize (c : Config) (s : State) : State := transition c (transition c s)

theorem rejected_command_atomic (s : State) (c : Command)
    (h : allowed s.phase c = false) : command s c = (false, s) := by
  simp [command, h]

theorem command_success_iff (s : State) (c : Command) :
    (command s c).1 = true ↔ allowed s.phase c = true := by
  unfold command; split <;> simp_all

theorem pause_idempotent (s : State) :
    command (command s .pause).2 .pause = command s .pause := by
  simp [command, allowed]

theorem resume_idempotent (s : State) :
    command (command s .resume).2 .resume = command s .resume := by
  simp [command, allowed]

theorem reset_exact (s : State) : command s .reset = (true, initial) := by
  simp [command, allowed]

theorem dump_clears_active (s : State) : (dump s).active = 0 := rfl
theorem dump_enters_dump (s : State) : (dump s).phase = .beamDump := rfl

theorem inject_only_no_beam (s : State) :
    (command s .inject).1 = true ↔ s.phase = .noBeam := by
  simp [command_success_iff, allowed]

theorem collide_only_ready (s : State) :
    (command s .collide).1 = true ↔ s.phase = .readyToCollide := by
  simp [command_success_iff, allowed]

theorem dump_exclusions (s : State) :
    (command s .dump).1 = true ↔
      s.phase ≠ .noBeam ∧ s.phase ≠ .beamDump ∧ s.phase ≠ .rampDown := by
  cases hp : s.phase <;> simp [command_success_iff, allowed, hp]

theorem duration_positive (p : Phase) (a : Bool) (d : Nat)
    (h : duration p a = some d) : 0 < d := by
  cases p <;> cases a <;> simp_all [duration] <;> omega

theorem duration_at_least_tick (p : Phase) (a : Bool) (d : Nat)
    (h : duration p a = some d) : 1000 ≤ d := by
  cases p <;> cases a <;> simp_all [duration] <;> omega

theorem manual_ready_holds (c : Config) (s : State) (h : s.auto = false)
    (p : s.phase = .readyToRamp ∨ s.phase = .readyToCollide) : transition c s = s := by
  rcases p with p | p <;> simp [transition, p, h]

theorem fresh_auto_does_not_inject (c : Config) :
    normalize c { initial with auto := true } = { initial with auto := true } := by
  rfl

theorem transition_preserves_clocks (c : Config) (s : State) :
    (transition c s).tick = s.tick ∧ (transition c s).elapsedMs = s.elapsedMs := by
  cases hp : s.phase <;> cases ha : s.auto <;>
    simp [transition, hp, ha, setPhase, inject, dump] <;>
    split <;> simp_all [setPhase, inject, dump]

theorem command_active_bound (cfg : Config) (s : State) (c : Command)
    (hc : 1 ≤ cfg.bunches) (hs : s.active ≤ cfg.bunches) :
    (command s c).2.active ≤ cfg.bunches := by
  unfold command
  split
  · cases c <;> simp_all [inject, dump, setPhase, initial]
  · exact hs

theorem transition_active_bound (cfg : Config) (s : State)
    (hc : 1 ≤ cfg.bunches) (hs : s.active ≤ cfg.bunches) :
    (transition cfg s).active ≤ cfg.bunches := by
  cases hp : s.phase <;> cases ha : s.auto <;>
    simp [transition, hp, ha, hs, setPhase, inject, dump] <;>
    split <;> simp_all [setPhase, inject, dump]

theorem transition_not_due (c : Config) (s : State) : due (transition c s) = false := by
  cases hp : s.phase <;> cases ha : s.auto <;>
    by_cases hd : due s = true <;>
    simp [transition, hp, ha, hd, setPhase, inject, dump] <;>
    simp_all [due, duration]

theorem normalize_before_deadline (c : Config) (s : State) (d : Nat)
    (h : duration (normalize c s).phase (normalize c s).auto = some d) :
    (normalize c s).phaseMs < d := by
  have hn := transition_not_due c (transition c s)
  change due (normalize c s) = false at hn
  simp [due, h] at hn
  omega

theorem settle_ready_fixed (c : Config) (s : State) (hd : due s = false) :
    transition c (transition c s) = transition c s := by
  cases hp : s.phase <;> cases ha : s.auto <;>
    simp only [transition, hp, ha, hd, Bool.false_eq_true, ↓reduceIte] <;>
    simp [setPhase, due, duration, ha]

theorem normalize_fixed (c : Config) (s : State) :
    transition c (normalize c s) = normalize c s := by
  exact settle_ready_fixed c (transition c s) (transition_not_due c s)

theorem normalize_active_bound (c : Config) (s : State)
    (hc : 1 ≤ c.bunches) (hs : s.active ≤ c.bunches) :
    (normalize c s).active ≤ c.bunches :=
  transition_active_bound c _ hc (transition_active_bound c s hc hs)

end LHC
