module Metas where

data ℕ : Set where
  zero : ℕ
  suc  : ℕ → ℕ

data _≡_ {A : Set} (x : A) : A → Set where
  refl : x ≡ x

pred : ℕ → ℕ
pred zero    = zero
pred (suc n) = n

const : {m : ℕ} → ℕ → ℕ
const n = n

_⊕_ : {m : ℕ} → ℕ → ℕ → ℕ
a ⊕ b = a

-- Implicit arguments that nothing determines.
one : ℕ
one = const (suc zero)

two : ℕ
two = zero ⊕ zero

-- An underscore that nothing determines.
three : ℕ → ℕ
three n = suc _

-- A constraint stuck on an underscore.
stuck : pred _ ≡ zero
stuck = refl

-- A goal, which is not an unsolved meta.
held : ℕ
held = {!  !}

-- A goal whose type Agda does not know yet: that type and its sort are
-- metas on the goal, which the goal itself shows.
four : ℕ → ℕ
four n with {!  !}
... | w = n

-- A binder whose type nothing determines: its type and that type's sort.
five : ∀ {k} → ℕ
five = zero

-- An underscore whose type has an implicit argument, hidden by default.
six : zero ≡ zero
six = _
