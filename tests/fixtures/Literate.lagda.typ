= Doubling

A small module in literate Typst.

```agda
module Literate where

data ℕ : Set where
  zero : ℕ
  suc  : ℕ → ℕ

-- Arrows are typed \to.
_+_ : ℕ → ℕ → ℕ
zero  + m = m
suc n + m = suc (n + m)
```

The type of `double` comes first:

```agda
double : ℕ → ℕ
```

In words, double : ℕ → ℕ maps every n to n + n, typed \to in Agda. Its
clause calls a helper function that does not exist yet:

```agda
double n = {! aux n !}
```

A block in another language, which Agda skips:

```haskell
double :: Int -> Int
double n = n + n
```

A bare block is Agda too:

```
half : ℕ → ℕ
half n = {!  !}
```
