//! Balanced-primality proof, requested as a companion to `anomaly_fold.rs`: does
//! primality itself admit a pararity-legal reading?
//!
//! **The claim, stated precisely (not "0 is prime" — that's false, see below).** `0` and
//! `1` share a real third category in standard number theory: a prime is `n > 1` with
//! exactly two positive divisors; `0` has infinitely many divisors (every nonzero integer
//! divides it), `1` has exactly one (itself) — neither has exactly two, so neither is
//! prime, and neither is composite either (composite = has a divisor strictly between 1
//! and itself). "Neither prime nor composite" is `{0, 1}`'s actual classification, in any
//! standard reference, not a repo-local convention.
//!
//! What IS a repo-local, authored construction (`PARARITY.md` §10, Sean Morin, ARCH000):
//! encoding that existing three-way split — Neither / Composite / Prime — as a balanced
//! trit, with `{0, 1}` sharing the fixed point (trit `0`) and Composite/Prime forming the
//! 2-orbit under the natural "has a nontrivial factorization or not" reflection. This is
//! the same n=3, k=1 shape as `anomaly_fold.rs`, proven the same way.
//!
//! Integer-only throughout (C14 firewall) — `isqrt_u64` (`crate::fixed_point`) reused
//! rather than re-derived (C06 revascularize).

use crate::fixed_point::isqrt_u64;

/// The three-way split every natural number falls into. `Neither` covers exactly `{0, 1}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimalityClass {
    /// `0` or `1` — has other than exactly two divisors, in both directions. This lane's
    /// fixed point (trit 0).
    Neither,
    /// `n > 1` with a divisor strictly between `1` and `n`.
    Composite,
    /// `n > 1` with exactly two divisors: `1` and itself.
    Prime,
}

impl PrimalityClass {
    /// Classify `n` by trial division up to `isqrt(n)`. Deterministic, total, integer-only.
    pub const fn classify(n: u64) -> Self {
        if n < 2 {
            return PrimalityClass::Neither;
        }
        if n == 2 || n == 3 {
            return PrimalityClass::Prime;
        }
        if n % 2 == 0 {
            return PrimalityClass::Composite;
        }
        let limit = isqrt_u64(n);
        let mut d = 3u64;
        while d <= limit {
            if n % d == 0 {
                return PrimalityClass::Composite;
            }
            d += 2;
        }
        PrimalityClass::Prime
    }

    /// The involution: `Composite` and `Prime` are each other's reflection (has a
    /// nontrivial factorization, or doesn't); `Neither` reflects to itself. `f(f(x)) == x`
    /// for all three states, and `Fix(f) == { Neither }` exactly — proven below.
    #[inline]
    pub const fn fold(self) -> Self {
        match self {
            PrimalityClass::Neither => PrimalityClass::Neither,
            PrimalityClass::Composite => PrimalityClass::Prime,
            PrimalityClass::Prime => PrimalityClass::Composite,
        }
    }

    /// The balanced-trit reading: `Neither` is the true zero, precisely because it is
    /// `fold`'s only fixed point — the same signature `{0, 1}` earns in `anomaly_fold.rs`.
    #[inline]
    pub const fn to_trit(self) -> i8 {
        match self {
            PrimalityClass::Composite => -1,
            PrimalityClass::Neither => 0,
            PrimalityClass::Prime => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PrimalityClass::{self, Composite, Neither, Prime};

    const ALL: [PrimalityClass; 3] = [Neither, Composite, Prime];

    /// `f` is a real involution over all three states.
    #[test]
    fn fold_is_an_involution_over_all_states() {
        for x in ALL {
            assert_eq!(x.fold().fold(), x, "f(f({x:?})) must equal {x:?}");
        }
    }

    /// Corollary 2's exact signature: `Fix(f)` has size 1, and it is `Neither`.
    #[test]
    fn fixed_point_set_is_exactly_neither() {
        let fixed: Vec<PrimalityClass> = ALL.into_iter().filter(|x| x.fold() == *x).collect();
        assert_eq!(fixed, vec![Neither], "Fix(f) must be exactly {{Neither}}, k=1");
    }

    /// One 2-orbit (`Composite` <-> `Prime`), not two fixed points and not a 3-cycle.
    #[test]
    fn nonfixed_states_form_one_two_orbit() {
        assert_eq!(Composite.fold(), Prime);
        assert_eq!(Prime.fold(), Composite);
        assert_ne!(Composite.fold(), Composite);
        assert_ne!(Prime.fold(), Prime);
    }

    /// `classify(0)` and `classify(1)` are BOTH `Neither` — the precise claim ("0 and 1
    /// share the fixed slot"), never "0 is prime".
    #[test]
    fn zero_and_one_are_neither_prime_nor_composite() {
        assert_eq!(PrimalityClass::classify(0), Neither);
        assert_eq!(PrimalityClass::classify(1), Neither);
        assert_ne!(PrimalityClass::classify(0), Prime, "0 is NOT prime");
        assert_ne!(PrimalityClass::classify(1), Prime, "1 is NOT prime");
    }

    /// Dual-oracle (C11): cross-check `classify` against a hand-independent sieve of
    /// Eratosthenes over 0..200, not just spot values from the same trial-division logic.
    #[test]
    fn classify_agrees_with_an_independent_sieve_over_0_to_200() {
        const CAP: usize = 200;
        let mut is_prime_sieve = [true; CAP + 1];
        is_prime_sieve[0] = false;
        is_prime_sieve[1] = false;
        let mut p = 2usize;
        while p * p <= CAP {
            if is_prime_sieve[p] {
                let mut m = p * p;
                while m <= CAP {
                    is_prime_sieve[m] = false;
                    m += p;
                }
            }
            p += 1;
        }

        for n in 0..=CAP as u64 {
            let expect_prime = is_prime_sieve[n as usize];
            let got = PrimalityClass::classify(n);
            if n < 2 {
                assert_eq!(got, Neither, "{n} must be Neither");
            } else if expect_prime {
                assert_eq!(got, Prime, "{n} must be Prime per independent sieve");
            } else {
                assert_eq!(got, Composite, "{n} must be Composite per independent sieve");
            }
        }
    }

    /// Trit reading agrees with fixed-point structure, same discipline as `anomaly_fold.rs`.
    #[test]
    fn trit_reading_agrees_with_fixed_point_structure() {
        for x in ALL {
            let is_fixed = x.fold() == x;
            let is_zero_trit = x.to_trit() == 0;
            assert_eq!(is_fixed, is_zero_trit, "{x:?}: fixed-point status must match trit==0");
        }
    }
}
