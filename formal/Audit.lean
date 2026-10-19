import LHC
import Lean

/-! Enumerate every LHC theorem and reject additional axioms, including sorryAx
and native_decide's Lean.ofReduceBool. Proofs stay inside the Lean kernel. -/
open Lean in
run_cmd do
  let env ← getEnv
  let mut count := 0
  for (name, info) in env.constants.toList do
    if (`LHC).isPrefixOf name then
      if info matches .axiomInfo _ then
        throwError "Project axiom declaration {name} is forbidden"
      if info.isTheorem then
        count := count + 1
        let axioms ← collectAxioms name
        for ax in axioms do
          unless #[`propext, `Quot.sound, `Classical.choice].contains ax do
            throwError "Unexpected axiom {ax} in {name}"
  if count = 0 then throwError "No LHC theorems found"
  logInfo m!"Audited {count} LHC theorems; only standard logical axioms allowed."
