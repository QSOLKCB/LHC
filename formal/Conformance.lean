import LHC

open LHC

def parseCommand : String → Option Command
  | "inject" => some .inject | "ramp" => some .ramp
  | "squeeze" => some .squeeze | "collide" => some .collide
  | "dump" => some .dump | "pause" => some .pause | "resume" => some .resume
  | "reset" => some .reset | "auto-on" => some (.auto true) | "auto-off" => some (.auto false)
  | _ => none

def ticks (c : Config) : Nat → State → Bool × State
  | 0, s => (true, s)
  | n + 1, s =>
      let (ok, s) := step c s
      if ok then ticks c n s else (false, s)

def row (ok : Bool) (s : State) : String :=
  String.intercalate "," ([ok.toNat, s.phase.code, s.phaseMs, s.tick, s.elapsedMs,
    s.fill, s.paused.toNat, s.auto.toNat, s.active, s.active].map toString)

partial def loop (input output : IO.FS.Stream) (c : Config) (s : State) : IO Unit := do
  let line ← input.getLine
  if line.isEmpty then return
  let words := line.trim.splitOn " "
  let (c, ok, s) ← match words with
    | ["new", n, dt] =>
        match n.toNat?, dt.toNat? with
        | some n, some dt =>
            let c : Config := ⟨n, dt⟩
            if 1 ≤ n && n ≤ 3564 && 1 ≤ dt && dt ≤ 1000 then
              pure (c, true, initial)
            else throw (IO.userError "invalid conformance configuration")
        | _, _ => throw (IO.userError "invalid numbers")
    | ["step", n] =>
        match n.toNat? with
        | some n =>
            if n > 100000 then throw (IO.userError "step count exceeds test bound")
            let (ok, s) := ticks c n s
            pure (c, ok, s)
        | none => throw (IO.userError "invalid step count")
    | [cmd] =>
        match parseCommand cmd with
        | some cmd => let (ok, s) := command s cmd; pure (c, ok, s)
        | none => throw (IO.userError "unknown command")
    | _ => throw (IO.userError "invalid conformance input")
  output.putStrLn (row ok s)
  loop input output c s

def main : IO Unit := do loop (← IO.getStdin) (← IO.getStdout) ⟨1, 1000⟩ initial
