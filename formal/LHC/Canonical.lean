import Std

namespace LHC

/-- Word-level layout: the actual v1 prefix has 19 words, followed by both beams.
This models already-encoded words, not Float.toBits or the text parser. -/
def canonical (header beam1 beam2 : List UInt64) : List UInt64 := header ++ (beam1 ++ beam2)

theorem canonical_length (h a b : List UInt64) (n : Nat)
    (hh : h.length = 19) (ha : a.length = n) (hb : b.length = n) :
    (canonical h a b).length = 19 + 2 * n := by simp [canonical, hh, ha, hb]; omega

theorem canonical_injective (h h' a a' b b' : List UInt64)
    (hh : h.length = h'.length) (ha : a.length = a'.length)
    (he : canonical h a b = canonical h' a' b') : h = h' ∧ a = a' ∧ b = b' := by
  have outer := List.append_inj he hh
  have inner := List.append_inj outer.2 ha
  exact ⟨outer.1, inner.1, inner.2⟩

def replayAccept (actual recorded : List UInt64) : Bool := actual == recorded

theorem replay_accept_iff (actual recorded : List UInt64) :
    replayAccept actual recorded = true ↔ actual = recorded := by simp [replayAccept]

theorem replay_rejects_difference (actual recorded : List UInt64) (h : actual ≠ recorded) :
    replayAccept actual recorded = false := by simp [replayAccept, h]

theorem replay_rejects_changed_word (actual recorded : List UInt64) (i : Nat)
    (h : actual[i]? ≠ recorded[i]?) : replayAccept actual recorded = false := by
  apply replay_rejects_difference
  intro he
  exact h (congrArg (fun xs => xs[i]?) he)

end LHC
