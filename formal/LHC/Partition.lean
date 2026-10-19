import Std

namespace LHC

def workerCount (requested bunches hardware : Nat) : Nat :=
  min (min (min requested bunches) hardware) 256

theorem workers_bounded (r n h : Nat) :
    workerCount r n h ≤ r ∧ workerCount r n h ≤ n ∧
    workerCount r n h ≤ h ∧ workerCount r n h ≤ 256 := by
  simp only [workerCount]; omega

theorem workers_positive (r n h : Nat) (hr : 0 < r) (hn : 0 < n) (hh : 0 < h) :
    0 < workerCount r n h := by simp only [workerCount]; omega

/-- The Rust loop assigns the remainder to the first workers in index order. -/
def chunkSizes (n workers : Nat) : List Nat :=
  List.replicate (n % workers) (n / workers + 1) ++
  List.replicate (workers - n % workers) (n / workers)

theorem sum_append (a b : List Nat) : (a ++ b).sum = a.sum + b.sum := by
  induction a with
  | nil => simp
  | cons x xs ih => simp only [List.cons_append, List.sum_cons, ih, Nat.add_assoc]

theorem chunk_count (n w : Nat) (hw : 0 < w) : (chunkSizes n w).length = w := by
  have := Nat.mod_lt n hw
  simp [chunkSizes]
  omega

theorem chunks_cover (n w : Nat) (hw : 0 < w) : (chunkSizes n w).sum = n := by
  have hm : n % w ≤ w := Nat.le_of_lt (Nat.mod_lt n hw)
  have hd := Nat.mod_add_div n w
  simp only [chunkSizes, sum_append, List.sum_replicate_nat, Nat.mul_add, Nat.mul_one]
  calc
    n % w * (n / w) + n % w + (w - n % w) * (n / w) =
        (n % w + (w - n % w)) * (n / w) + n % w := by rw [Nat.add_mul]; omega
    _ = w * (n / w) + n % w := by rw [Nat.add_sub_of_le hm]
    _ = n := by omega

theorem chunks_nonempty (n w size : Nat) (hw : 0 < w) (hn : w ≤ n)
    (hs : size ∈ chunkSizes n w) : 0 < size := by
  have hd : 0 < n / w := Nat.div_pos hn hw
  simp only [chunkSizes, List.mem_append, List.mem_replicate] at hs
  rcases hs with ⟨_, h⟩ | ⟨_, h⟩ <;> omega

/-- Parametric in the per-element operation, including a fixed-build IEEE operation.
The theorem requires the chunks to be concatenated in their original order. -/
theorem partition_map_eq {α β : Type} (f : α → β) (chunks : List (List α)) :
    (chunks.map (List.map f)).flatten = chunks.flatten.map f := by
  induction chunks with
  | nil => rfl
  | cons xs rest ih => simp only [List.map_cons, List.flatten_cons, List.map_append, ih]

/-- No commutativity or associativity is assumed for the reduction. -/
theorem ordered_reduction_eq {α β σ : Type} (f : α → β) (op : σ → β → σ)
    (start : σ) (chunks : List (List α)) :
    ((chunks.map (List.map f)).flatten).foldl op start =
      (chunks.flatten.map f).foldl op start := by rw [partition_map_eq]

end LHC
