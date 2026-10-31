use std::num::IntErrorKind;

use crate::args::parse_flags;
use crate::{Ctx, Io};

/// Enough Miller-Rabin witnesses to make the test exact for every `u64`.
const WITNESSES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (_, numbers) = match parse_flags(args, "") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "factor: {}", msg);
            ewln!(io, "Usage: factor [NUMBER...]");
            return 1;
        }
    };
    if !numbers.is_empty() {
        return factor_all(io, numbers.into_iter());
    }
    // With no operands the numbers come from stdin, separated by any blanks.
    let mut text = String::new();
    if let Err(e) = io.stdin.read_to_string(&mut text) {
        ewln!(io, "factor: -: {}", e);
        return 1;
    }
    factor_all(io, text.split_whitespace())
}

/// Prints each number followed by its prime factors, smallest first, as
/// `12: 2 2 3`. A word that isn't a number is reported and skipped.
fn factor_all<'a>(io: &mut Io<'_>, words: impl Iterator<Item = &'a str>) -> i32 {
    let mut status = 0;
    for word in words {
        match word.parse::<u64>() {
            Ok(n) => {
                let factors: String = factorize(n).iter().map(|p| format!(" {}", p)).collect();
                wln!(io, "{}:{}", n, factors);
            }
            Err(e) => {
                if *e.kind() == IntErrorKind::PosOverflow {
                    ewln!(io, "factor: '{}' is too large", word);
                } else {
                    ewln!(io, "factor: '{}' is not a valid positive integer", word);
                }
                status = 1;
            }
        }
    }
    status
}

/// The prime factors of `n` in ascending order, with repeats. 0 and 1 have
/// none.
fn factorize(mut n: u64) -> Vec<u64> {
    let mut factors = Vec::new();
    if n < 2 {
        return factors;
    }
    // Small primes are cheapest to strip by division, and doing so first
    // leaves Pollard's rho only odd numbers with no tiny factors.
    for p in WITNESSES {
        while n.is_multiple_of(p) {
            factors.push(p);
            n /= p;
        }
    }
    split(n, &mut factors);
    factors.sort_unstable();
    factors
}

fn split(n: u64, factors: &mut Vec<u64>) {
    if n == 1 {
        return;
    }
    if is_prime(n) {
        factors.push(n);
        return;
    }
    let d = find_divisor(n);
    split(d, factors);
    split(n / d, factors);
}

/// Deterministic Miller-Rabin.
fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    for p in WITNESSES {
        if n.is_multiple_of(p) {
            return n == p;
        }
    }
    let shift = (n - 1).trailing_zeros();
    let odd = (n - 1) >> shift;
    'witness: for a in WITNESSES {
        let mut x = pow_mod(a, odd, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..shift {
            x = mul_mod(x, x, n);
            if x == n - 1 {
                continue 'witness;
            }
        }
        return false;
    }
    true
}

/// A non-trivial divisor of the composite `n`, by Pollard's rho. A run that
/// cycles without finding one is retried with a different polynomial.
fn find_divisor(n: u64) -> u64 {
    let mut c: u64 = 1;
    loop {
        let step =
            |x: u64| ((u128::from(x) * u128::from(x) + u128::from(c)) % u128::from(n)) as u64;
        let (mut slow, mut fast, mut d) = (2, 2, 1);
        while d == 1 {
            slow = step(slow);
            fast = step(step(fast));
            d = gcd(slow.abs_diff(fast), n);
        }
        if d != n {
            return d;
        }
        c += 1;
    }
}

fn mul_mod(a: u64, b: u64, m: u64) -> u64 {
    (u128::from(a) * u128::from(b) % u128::from(m)) as u64
}

fn pow_mod(mut base: u64, mut exp: u64, m: u64) -> u64 {
    let mut result = 1;
    base %= m;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, base, m);
        }
        base = mul_mod(base, base, m);
        exp >>= 1;
    }
    result
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn prints_each_number_with_its_factors() {
        let out = run("factor", &["12", "97", "1", "0"], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, "12: 2 2 3\n97: 97\n1:\n0:\n");
    }

    #[test]
    fn handles_the_largest_numbers_quickly() {
        let prime = "18446744073709551557";
        assert_eq!(
            run("factor", &[prime], "").stdout,
            format!("{}: {}\n", prime, prime)
        );
        // Two primes just under 2^32: hopeless by trial division.
        let (p, q) = (4294967279u64, 4294967291u64);
        assert_eq!(super::factorize(p * q), [p, q]);
        assert_eq!(super::factorize(p * p), [p, p]);
    }

    #[test]
    fn reads_numbers_from_stdin_without_operands() {
        let out = run("factor", &[], "6\n  10 \n");
        assert_eq!(out.stdout, "6: 2 3\n10: 2 5\n");
    }

    #[test]
    fn reports_bad_numbers_and_keeps_going() {
        let out = run("factor", &["x", "4", "18446744073709551616"], "");
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, "4: 2 2\n");
        assert!(out.stderr.contains("'x' is not a valid positive integer"));
        assert!(out.stderr.contains("is too large"));
    }
}
