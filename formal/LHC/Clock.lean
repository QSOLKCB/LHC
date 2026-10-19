import LHC.Control

namespace LHC

def u64Max : Nat := 18446744073709551615

def clocksFit (c : Config) (s : State) : Bool :=
  s.tick + 1 ≤ u64Max && s.elapsedMs + c.dtMs ≤ u64Max && s.phaseMs + c.dtMs ≤ u64Max

def interval (remaining : Nat) (s : State) : Nat :=
  match duration s.phase s.auto with
  | none => remaining
  | some d => min remaining (d - s.phaseMs)

/-- Bounded executable projection. Failure is explicit for exhausted fuel or
invalid phase timing; reachable valid Rust traces must never take that branch. -/
def consume (c : Config) : Nat → Nat → State → Option State
  | _, 0, s => some s
  | 0, _ + 1, _ => none
  | fuel + 1, remaining + 1, s =>
      let s := normalize c s
      let n := interval (remaining + 1) s
      if n = 0 then none else
        consume c fuel (remaining + 1 - n)
          (normalize c { s with phaseMs := s.phaseMs + n })

def step (c : Config) (s : State) : Bool × State :=
  if s.paused then (true, s)
  else if clocksFit c s then
    match consume c (c.dtMs + 1) c.dtMs s with
    | none => (false, s)
    | some next => (true, { next with tick := s.tick + 1, elapsedMs := s.elapsedMs + c.dtMs })
  else (false, s)

theorem paused_step_exact (c : Config) (s : State) (h : s.paused = true) :
    step c s = (true, s) := by simp [step, h]

theorem overflow_atomic (c : Config) (s : State)
    (hp : s.paused = false) (hf : clocksFit c s = false) : step c s = (false, s) := by
  simp [step, hp, hf]

theorem rejected_step_atomic (c : Config) (s : State) (h : (step c s).1 = false) :
    (step c s).2 = s := by
  unfold step at *
  split at * <;> simp_all
  split at * <;> simp_all
  split at * <;> simp_all

theorem successful_step_clocks (c : Config) (s : State)
    (hp : s.paused = false) (h : (step c s).1 = true) :
    (step c s).2.tick = s.tick + 1 ∧
    (step c s).2.elapsedMs = s.elapsedMs + c.dtMs := by
  simp only [step, hp, Bool.false_eq_true, ↓reduceIte] at *
  split at * <;> simp_all
  split at * <;> simp_all

theorem successful_step_no_clock_overflow (c : Config) (s : State)
    (hp : s.paused = false) (h : (step c s).1 = true) :
    (step c s).2.tick ≤ u64Max ∧ (step c s).2.elapsedMs ≤ u64Max := by
  have clocks := successful_step_clocks c s hp h
  have fit : clocksFit c s = true := by
    cases hf : clocksFit c s with
    | true => rfl
    | false => rw [overflow_atomic c s hp hf] at h; contradiction
  simp only [clocksFit, Bool.and_eq_true, decide_eq_true_eq] at fit
  omega

theorem segment_conserves_time (remaining available : Nat) :
    min remaining available + (remaining - min remaining available) = remaining := by omega

theorem segment_makes_progress (remaining available : Nat)
    (hr : 0 < remaining) (ha : 0 < available) :
    0 < min remaining available ∧ remaining - min remaining available < remaining := by omega

theorem interval_bounded (remaining : Nat) (s : State) : interval remaining s ≤ remaining := by
  unfold interval
  split <;> omega

theorem interval_positive (remaining : Nat) (s : State) (hr : 0 < remaining)
    (hs : ∀ d, duration s.phase s.auto = some d → s.phaseMs < d) :
    0 < interval remaining s := by
  unfold interval
  split
  · exact hr
  · rename_i d hd
    have := hs d hd
    omega

/-- The fuel and zero-interval failures in the executable model are unreachable
when the initial fuel covers the requested integer milliseconds. -/
theorem consume_total (c : Config) (fuel remaining : Nat) (s : State)
    (hr : remaining ≤ fuel) : ∃ next, consume c fuel remaining s = some next := by
  induction fuel generalizing remaining s with
  | zero =>
      have : remaining = 0 := by omega
      subst remaining
      exact ⟨s, rfl⟩
  | succ fuel ih =>
      cases remaining with
      | zero => exact ⟨s, rfl⟩
      | succ remaining =>
          have hn : 0 < interval (remaining + 1) (normalize c s) :=
            interval_positive _ _ (by omega) (normalize_before_deadline c s)
          obtain ⟨next, he⟩ := ih
            (remaining + 1 - interval (remaining + 1) (normalize c s))
            (normalize c { normalize c s with
              phaseMs := (normalize c s).phaseMs + interval (remaining + 1) (normalize c s) })
            (by omega)
          exact ⟨next, by simpa only [consume, if_neg (Nat.ne_of_gt hn)] using he⟩

theorem step_succeeds_when_clocks_fit (c : Config) (s : State)
    (hf : clocksFit c s = true) : (step c s).1 = true := by
  obtain ⟨next, hn⟩ := consume_total c (c.dtMs + 1) c.dtMs s (by omega)
  cases hp : s.paused <;> simp [step, hf, hn, hp]

theorem step_success_iff (c : Config) (s : State) :
    (step c s).1 = true ↔ s.paused = true ∨ clocksFit c s = true := by
  cases hp : s.paused <;> cases hf : clocksFit c s <;>
    simp [paused_step_exact, overflow_atomic, step_succeeds_when_clocks_fit, hp, hf]

end LHC
