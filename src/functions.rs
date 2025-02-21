use crate::RoundingMode;

use super::float::Float;

impl<const EXPONENT: usize, const MANTISSA: usize, const PARTS: usize>
    Float<EXPONENT, MANTISSA, PARTS>
{
    /// Calculates the power of two.
    pub fn sqr(&self) -> Self {
        *self * *self
    }
    /// Calculates the square root of the number using the Newton Raphson
    /// method.
    pub fn sqrt(&self) -> Self {
        if self.is_zero() {
            return *self; // (+/-) zero
        } else if self.is_nan() || self.is_negative() {
            return Self::nan(self.get_sign()); // (-/+)Nan, -Number.
        } else if self.is_inf() {
            return *self; // Inf+.
        }

        let target = *self;
        let two = Self::from_u64(2);

        // Start the search at max(2, x).
        let mut x = if target < two { two } else { target };
        let mut prev = x;

        loop {
            x = (x + (target / x)) / two;
            // Stop when value did not change or regressed.
            if prev < x || x == prev {
                return x;
            }
            prev = x;
        }
    }

    /// Returns the absolute value of this float.
    pub fn abs(&self) -> Self {
        let mut x = *self;
        x.set_sign(false);
        x
    }

    /// Returns the greater of self and `other`.
    pub fn max(&self, other: Self) -> Self {
        if self.is_nan() {
            return other;
        } else if other.is_nan() {
            return *self;
        } else if self.get_sign() != other.get_sign() {
            return if self.get_sign() { other } else { *self }; // Handle (+-)0.
        }
        if *self > other {
            *self
        } else {
            other
        }
    }

    /// Returns the smaller of self and `other`.
    pub fn min(&self, other: Self) -> Self {
        if self.is_nan() {
            return other;
        } else if other.is_nan() {
            return *self;
        } else if self.get_sign() != other.get_sign() {
            return if self.get_sign() { *self } else { other }; // Handle (+-)0.
        }
        if *self > other {
            other
        } else {
            *self
        }
    }
}

#[cfg(feature = "std")]
#[test]
fn test_sqrt() {
    use super::utils;
    use super::FP64;

    // Try a few power-of-two values.
    for i in 0..256 {
        let v16 = FP64::from_u64(i * i);
        assert_eq!(v16.sqrt().as_f64(), (i) as f64);
    }

    // Test the category and value of the different special values (inf, zero,
    // correct sign, etc).
    for v_f64 in utils::get_special_test_values() {
        let vf = FP64::from_f64(v_f64);
        assert_eq!(vf.sqrt().is_inf(), v_f64.sqrt().is_infinite());
        assert_eq!(vf.sqrt().is_nan(), v_f64.sqrt().is_nan());
        assert_eq!(vf.sqrt().is_negative(), v_f64.sqrt().is_sign_negative());
    }

    // Test precomputed values.
    fn check(inp: f64, res: f64) {
        assert_eq!(FP64::from_f64(inp).sqrt().as_f64(), res);
    }
    check(1.5, 1.224744871391589);
    check(2.3, 1.51657508881031);
    check(6.7, 2.588435821108957);
    check(7.9, 2.8106938645110393);
    check(11.45, 3.383784863137726);
    check(1049.3, 32.39290045673589);
    check(90210.7, 300.35096137685326);
    check(199120056003.73413, 446228.70369770494);
    check(0.6666666666666666, 0.816496580927726);
    check(0.4347826086956522, 0.6593804733957871);
    check(0.14925373134328357, 0.3863337046431279);
    check(0.12658227848101264, 0.35578403348241);
    check(0.08733624454148473, 0.29552706228277087);
    check(0.0009530162965786716, 0.030870962028719993);
    check(1.1085159520988087e-5, 0.00332943831914455);
    check(5.0120298432056786e-8, 0.0002238756316173263);
}

#[cfg(feature = "std")]
#[test]
fn test_min_max() {
    use super::utils;
    use super::FP64;

    fn check(v0: f64, v1: f64) {
        // Min.
        let correct = v0.min(v1);
        let test = FP64::from_f64(v0).min(FP64::from_f64(v1)).as_f64();
        assert_eq!(test.is_nan(), correct.is_nan());
        if !correct.is_nan() {
            assert_eq!(correct, test);
        }
        // Max.
        let correct = v0.max(v1);
        let test = FP64::from_f64(v0).max(FP64::from_f64(v1)).as_f64();
        assert_eq!(test.is_nan(), correct.is_nan());
        if !correct.is_nan() {
            assert_eq!(correct, test);
        }
    }

    // Test a bunch of special values (Inf, Epsilon, Nan, (+-)Zeros).
    for v0 in utils::get_special_test_values() {
        for v1 in utils::get_special_test_values() {
            check(v0, v1);
        }
    }

    let mut lfsr = utils::Lfsr::new();

    for _ in 0..100 {
        let v0 = f64::from_bits(lfsr.get64());
        let v1 = f64::from_bits(lfsr.get64());
        check(v0, v1);
    }
}

#[cfg(feature = "std")]
#[test]
fn test_abs() {
    use super::utils;
    use super::FP64;
    for v in utils::get_special_test_values() {
        if !v.is_nan() {
            assert_eq!(FP64::from_f64(v).abs().as_f64(), v.abs());
        }
    }
}

//  Compute basic constants.

impl<const EXPONENT: usize, const MANTISSA: usize, const PARTS: usize>
    Float<EXPONENT, MANTISSA, PARTS>
{
    /// Computes PI -- Algorithm description in Pg 246:
    /// Fast Multiple-Precision Evaluation of Elementary Functions
    /// by Richard P. Brent.
    pub fn pi() -> Self {
        let one = Self::from_i64(1);
        let two = Self::from_i64(2);
        let four = Self::from_i64(4);

        let mut a = one;
        let mut b = one / two.sqrt();
        let mut t = one / four;
        let mut x = one;

        while a != b {
            let y = a;
            a = (a + b) / two;
            b = (b * y).sqrt();
            t = t - x * ((a - y).sqr());
            x = x * two;
        }
        a * a / t
    }

    /// Computes e using Euler's continued fraction, which is a simple series.
    pub fn e() -> Self {
        let two = Self::from_i64(2);
        let one = Self::from_i64(1);
        let mut term = one;
        let iterations: i64 = (EXPONENT * 2) as i64;
        for i in (1..iterations).rev() {
            let v = Self::from_i64(i);
            term = v + v / term;
        }

        two + one / term
    }
}

#[cfg(feature = "std")]
#[test]
fn test_pi() {
    use super::FP128;
    assert_eq!(FP128::pi().as_f64(), std::f64::consts::PI);
}

#[cfg(feature = "std")]
#[test]
fn test_e() {
    use super::{FP128, FP32};
    assert_eq!(FP128::e().as_f64(), std::f64::consts::E);
    assert_eq!(FP32::e().as_f32(), std::f32::consts::E);
}

impl<const EXPONENT: usize, const MANTISSA: usize, const PARTS: usize>
    Float<EXPONENT, MANTISSA, PARTS>
{
    /// Similar to 'scalbln'. Adds or subtracts to the exponent of the number,
    /// and scaling it by 2^exp.
    pub fn scale(&self, scale: i64, rm: RoundingMode) -> Self {
        use crate::bigint::LossFraction;
        if !self.is_normal() {
            return *self;
        }

        let mut r = Self::new(
            self.get_sign(),
            self.get_exp() + scale,
            self.get_mantissa(),
        );
        r.normalize(rm, LossFraction::ExactlyZero);
        r
    }

    /// Returns the remainder from a division of two floats. This is equivalent
    /// to rust 'rem' or c 'fmod'.
    pub fn rem(&self, rhs: Self) -> Self {
        use core::ops::Sub;
        // Handle NaNs.
        if self.is_nan() || rhs.is_nan() || self.is_inf() || rhs.is_zero() {
            return Self::nan(self.get_sign());
        }
        // Handle values that are obviously zero or self.
        if self.is_zero() || rhs.is_inf() {
            return *self;
        }

        // Operate on integers.
        let mut lhs = self.abs();
        let rhs = if rhs.is_negative() { rhs.neg() } else { rhs };
        debug_assert!(lhs.is_normal() && rhs.is_normal());

        // This is a clever algorithm. Subtracting the RHS from LHS in a loop
        // would be slow, but we perform a divide-like algorithm where we shift
        // 'rhs' by higher powers of two, and subtract it from LHS, until LHS is
        // lower than RHS.
        while lhs >= rhs && lhs.is_normal() {
            let scale = lhs.get_exp() - rhs.get_exp();

            // Scale RHS by a power of two. If we overshoot, take a step back.
            let mut diff = rhs.scale(scale, RoundingMode::NearestTiesToEven);
            if diff > lhs {
                diff = rhs.scale(scale - 1, RoundingMode::NearestTiesToEven);
            }

            lhs = lhs.sub(diff);
        }

        // Set the original sign.
        lhs.set_sign(self.get_sign());
        lhs
    }
}

#[test]
fn test_scale() {
    use super::FP64;
    let x = FP64::from_u64(1);
    let y = x.scale(1, RoundingMode::NearestTiesToEven);
    assert_eq!(y.as_f64(), 2.0);
    let z = x.scale(-1, RoundingMode::NearestTiesToEven);
    assert_eq!(z.as_f64(), 0.5);
}

#[cfg(feature = "std")]
#[test]
fn test_rem() {
    use super::utils;
    use super::utils::Lfsr;
    use super::FP64;
    use core::ops::Rem;

    fn check_two_numbers(v0: f64, v1: f64) {
        let f0 = FP64::from_f64(v0);
        let f1 = FP64::from_f64(v1);
        let r0 = v0.rem(v1);
        let r1 = f0.rem(f1).as_f64();
        assert_eq!(r0.is_nan(), r1.is_nan());
        if !r0.is_nan() {
            assert_eq!(r0, r1);
        }
    }

    // Test addition, multiplication, subtraction with random values.
    check_two_numbers(1.4, 2.5);
    check_two_numbers(2.4, 1.5);
    check_two_numbers(1000., std::f64::consts::PI);
    check_two_numbers(10000000000000000000., std::f64::consts::PI / 1000.);
    check_two_numbers(10000000000000000000., std::f64::consts::PI);
    check_two_numbers(100., std::f64::consts::PI);
    check_two_numbers(100., -std::f64::consts::PI);
    check_two_numbers(0., 10.);
    check_two_numbers(std::f64::consts::PI, 10.0);

    // Test a bunch of random values:
    let mut lfsr = Lfsr::new();
    for _ in 0..5000 {
        let v0 = f64::from_bits(lfsr.get64());
        let v1 = f64::from_bits(lfsr.get64());
        check_two_numbers(v0, v1);
    }

    // Test the hard cases:
    for v0 in utils::get_special_test_values() {
        for v1 in utils::get_special_test_values() {
            check_two_numbers(v0, v1);
        }
    }
}

impl<const EXPONENT: usize, const MANTISSA: usize, const PARTS: usize>
    Float<EXPONENT, MANTISSA, PARTS>
{
    /// sin(x) = x - x^3 / 3! + x^5 / 5! - x^7/7! ....
    fn sin_taylor(x: Self) -> Self {
        let mut neg = false;
        let mut top = x;
        let mut bottom = 1;
        let mut sum = Self::zero(false);
        let x2 = x.sqr();
        for i in 1..10 {
            // Update sum.
            let elem = top / Self::from_u64(bottom);
            sum = if neg { sum - elem } else { sum + elem };

            // Prepare the next element.
            top = top * x2;
            bottom = bottom * (i * 2) * (i * 2 + 1);
            neg ^= true;
        }

        sum
    }

    /// Reduce sin(x) in the range 0..pi/2, using the identity:
    /// sin(3x) = 3sin(x)-4(sin(x)^3)
    fn sin_step4_reduction(x: Self, steps: usize) -> Self {
        if steps == 0 {
            return Self::sin_taylor(x);
        }
        let three = Self::from_u64(3);
        let four = Self::from_u64(4);

        let x3 = x / Self::from_u64(3);
        let sx = Self::sin_step4_reduction(x3, steps - 1);
        three * sx - four * (sx * sx * sx)
    }

    /// Return the sine function.
    pub fn sin(&self) -> Self {
        // Fast Trigonometric functions for Arbitrary Precision number
        // by Henrik Vestermark.

        if self.is_zero() {
            return *self;
        }
        assert!(self.is_normal());

        let mut neg = false;
        // Step1 range reduction.

        let mut val = *self;

        // Handle the negatives.
        if val.is_negative() {
            val = val.neg();
            neg ^= true;
        }
        let pi = Self::pi();
        let pi2 = pi.scale(1, RoundingMode::Zero);
        let pi_half = pi.scale(-1, RoundingMode::Zero);

        // Step 1
        if val > pi2 {
            val = val.rem(pi2);
        }

        debug_assert!(val <= pi2);
        // Step 2.
        if val > pi {
            val = val - pi;
            neg ^= true;
        }

        debug_assert!(val <= pi);
        // Step 3.
        if val > pi_half {
            val = pi - val;
        }
        debug_assert!(val <= pi_half);

        let res = Self::sin_step4_reduction(val, 5);
        if neg {
            res.neg()
        } else {
            res
        }
    }
}

#[test]
fn test_sin_taylor() {
    use super::FP128;

    for i in -10..100 {
        let f0 = i as f64;
        let r0 = f0.sin();
        let r1 = FP128::from_f64(f0).sin().as_f64();
        assert_eq!(r0, r1);
    }
}
