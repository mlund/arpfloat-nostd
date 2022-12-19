/// \returns a mask full of 1s, of \p b bits.
fn mask(b: usize) -> usize {
    (1 << (b)) - 1
}

#[test]
fn test_masking() {
    assert_eq!(mask(0), 0x0);
    assert_eq!(mask(1), 0x1);
    assert_eq!(mask(8), 255);
}

/// Convert a mantissa in the implicit format (no possible leading 1 bit) to
/// the internal storage format. If \p leading_1 is set then a leading one is
/// added (otherwise it is a subnormal).
/// Format: [1 IIIIII 00000000]
fn expand_mantissa_to_explicit<const FROM: usize>(
    input: u64,
    leading_1: bool,
) -> u64 {
    let value: u64 = if leading_1 { 1 << 63 } else { 0 };
    let shift = 63 - FROM;
    value | (input << shift)
}

#[test]
fn test_expand_mantissa() {
    assert_eq!(expand_mantissa_to_explicit::<8>(0, true), 1 << 63);
    assert_eq!(
        expand_mantissa_to_explicit::<8>(1, true),
        0x8080000000000000
    );
    assert_eq!(
        expand_mantissa_to_explicit::<32>(0xffffffff, false),
        0x7fffffff80000000
    );
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Float<const EXPONENT: usize, const MANTISSA: usize> {
    // The Sign bit.
    sign: bool,
    // The Exponent.
    exp: u64,
    // The significand, including the possible implicit bit, aligned to the
    // left. Format [1xxxxxxx........]
    mantissa: u64,
}

impl<const EXPONENT: usize, const MANTISSA: usize> Float<EXPONENT, MANTISSA> {
    pub fn new(sign: bool, exp: i64, sig: u64) -> Self {
        let mut a = Self::default();
        a.set_sign(sign);
        a.set_exp(exp);
        a.set_mantissa(sig);
        a
    }

    pub fn zero(sign: bool) -> Self {
        let mut a = Self::default();
        a.set_sign(sign);
        a.set_unbiased_exp(0);
        a.set_mantissa(0);
        a
    }

    pub fn inf(sign: bool) -> Self {
        let mut a = Self::default();
        a.set_sign(sign);
        a.set_unbiased_exp(mask(EXPONENT) as u64);
        a.set_mantissa(0);
        a
    }
    pub fn nan(sign: bool) -> Self {
        let mut a = Self::default();
        a.set_sign(sign);
        a.set_unbiased_exp(mask(EXPONENT) as u64);
        a.set_mantissa((1 << MANTISSA) - 1);
        a
    }

    /// \returns True if the Float has the signaling exponent.
    fn in_special_exp(&self) -> bool {
        self.get_unbiased_exp() == mask(EXPONENT) as u64
    }

    /// \returns True if the Float is negative
    pub fn is_negative(&self) -> bool {
        self.get_sign()
    }

    /// \returns True if the Float is a positive or negative infinity.
    pub fn is_inf(&self) -> bool {
        self.in_special_exp() && self.get_frac_mantissa() == 0
    }

    /// \returns True if the Float is a positive or negative NaN.
    pub fn is_nan(&self) -> bool {
        self.in_special_exp() && self.get_frac_mantissa() != 0
    }

    pub fn from_f32(float: f32) -> Self {
        Self::from_bits::<8, 23>(float.to_bits() as u64)
    }

    pub fn from_f64(float: f64) -> Self {
        Self::from_bits::<11, 52>(float.to_bits())
    }

    pub fn is_normal(&self) -> bool {
        self.get_unbiased_exp() != 0
    }

    pub fn from_u64(val: u64) -> Self {
        if val == 0 {
            return Self::zero(false);
        }

        // Figure out how to shift the input to align the first bit with the
        // msb of the mantissa.
        let lz = val.leading_zeros();
        let size_in_bits = 64 - lz;

        // If we can't adjust the exponent then this is infinity.
        if size_in_bits > Self::get_exp_bounds().1 as u32 {
            return Self::inf(false);
        }

        let mut a = Self::default();
        a.set_exp(size_in_bits as i64 - 1);
        a.set_mantissa(val << lz);
        a.set_sign(false);
        a
    }

    pub fn from_i64(val: i64) -> Self {
        if val < 0 {
            let mut a = Self::from_u64(-val as u64);
            a.set_sign(true);
            return a;
        }

        Self::from_u64(val as u64)
    }

    pub fn from_bits<const E: usize, const M: usize>(float: u64) -> Self {
        // Extract the biased exponent (wipe the sign and mantissa).
        let biased_exp = (float >> M) & mask(E) as u64;
        // Wipe the original exponent and mantissa.
        let sign = (float >> (E + M)) & 1;
        // Wipe the sign and exponent.
        let mantissa = float & mask(M) as u64;
        let mut a = Self::default();
        a.set_sign(sign == 1);
        a.set_exp(biased_exp as i64 - Self::compute_ieee745_bias(E) as i64);
        let leading_1 = biased_exp != 0;
        let new_mantissa =
            expand_mantissa_to_explicit::<M>(mantissa, leading_1);
        a.set_mantissa(new_mantissa);
        a
    }

    /// \returns the sign bit.
    pub fn get_sign(&self) -> bool {
        self.sign
    }

    /// Sets the sign to \p s.
    pub fn set_sign(&mut self, s: bool) {
        self.sign = s;
    }

    /// \returns the mantissa (including the implicit 0/1 bit).
    pub fn get_mantissa(&self) -> u64 {
        // We clear the bottom bits before returning them to ensure that we
        // don't increase the accuracy of the number. Notice that we only count
        // the digits after the period in the count (1.xxxxxx).
        let unused_bits = 64 - MANTISSA - 1;
        (self.mantissa >> unused_bits) << unused_bits
    }

    /// \return the fractional part of the mantissa without the implicit 1 or 0.
    /// [(0/1).xxxxxx].
    pub fn get_frac_mantissa(&self) -> u64 {
        self.mantissa << 1
    }

    /// Sets the mantissa to \p sg (including the implicit 0/1 bit).
    pub fn set_mantissa(&mut self, sg: u64) {
        self.mantissa = sg;
    }

    /// \returns the unbiased exponent.
    pub fn get_unbiased_exp(&self) -> u64 {
        self.exp
    }

    /// \returns the biased exponent.
    pub fn get_exp(&self) -> i64 {
        self.exp as i64 - Self::get_bias() as i64
    }

    /// Sets the biased exponent to \p new_exp.
    pub fn set_exp(&mut self, new_exp: i64) {
        let (exp_min, exp_max) = Self::get_exp_bounds();
        assert!(new_exp <= exp_max);
        assert!(exp_min <= new_exp);

        let new_exp: i64 = new_exp + (Self::get_bias() as i64);
        self.exp = new_exp as u64
    }

    /// Sets the unbiased exponent to \p new_exp.
    pub fn set_unbiased_exp(&mut self, new_exp: u64) {
        self.exp = new_exp
    }
    // \returns the bias for this Float type.
    fn compute_ieee745_bias(exponent_bits: usize) -> usize {
        (1 << (exponent_bits - 1)) - 1
    }

    pub fn get_bias() -> u64 {
        Self::compute_ieee745_bias(EXPONENT) as u64
    }

    /// \returns the bounds of the upper and lower bounds of the exponent.
    pub fn get_exp_bounds() -> (i64, i64) {
        let exp_min: i64 = -(Self::get_bias() as i64);
        let exp_max: i64 = ((1 << EXPONENT) - Self::get_bias()) as i64;
        (exp_min, exp_max)
    }

    pub fn cast<const E: usize, const S: usize>(&self) -> Float<E, S> {
        let mut x = Float::<E, S>::default();
        x.set_sign(self.get_sign());
        x.set_exp(self.get_exp());
        // Handle Nan/Inf.
        if self.in_special_exp() {
            x.set_unbiased_exp(mask(E) as u64);
        }
        x.set_mantissa(self.get_mantissa());
        x
    }

    fn as_native_float<const E: usize, const M: usize>(&self) -> u64 {
        // https://en.wikipedia.org/wiki/IEEE_754
        let mut bits: u64 = self.get_sign() as u64;
        bits <<= E;
        bits |= (self.get_exp() + Self::get_bias() as i64) as u64;
        bits <<= M;
        let mant = self.get_mantissa();
        let mant = mant << 1; // Clear the explicit '1' bit.
        let mant = mant >> (64 - M); // Put the mantissa in place.
        assert!(mant <= 1 << M);
        bits |= mant;
        bits
    }
    pub fn as_f32(&self) -> f32 {
        let b: FP32 = self.cast();
        let bits = b.as_native_float::<8, 23>();
        f32::from_bits(bits as u32)
    }
    pub fn as_f64(&self) -> f64 {
        let b: FP64 = self.cast();
        let bits = b.as_native_float::<11, 52>();
        f64::from_bits(bits)
    }
    pub fn dump(&self) {
        let exp = self.get_exp();
        let mantissa = self.get_mantissa();
        let sign = self.get_sign() as usize;
        println!(
            "FP[S={} : E={} (biased {}) :SI=0x{:x}]",
            sign, self.exp, exp, mantissa
        );
    }
}

pub type FP16 = Float<5, 10>;
pub type FP32 = Float<8, 23>;
pub type FP64 = Float<11, 52>;

#[test]
fn test_round_trip_native_float_conversion() {
    let f = f32::from_bits(0x41700000);
    let a = FP32::from_f32(f);
    assert_eq!(f, a.as_f32());

    let pi = 355. / 113.;
    let a = FP64::from_f64(pi);
    assert_eq!(pi, a.as_f64());

    let a_float = f32::from_bits(0x3f8fffff);
    let a = FP64::from_f32(a_float);
    let b: FP32 = a.cast();
    assert_eq!(a.as_f32(), a_float);
    assert_eq!(b.as_f32(), a_float);

    let f = f32::from_bits(0x000000);
    let a = FP32::from_f32(f);
    assert_eq!(a.is_normal(), false);
    assert_eq!(f, a.as_f32());
}
#[test]
fn setter_test() {
    assert_eq!(FP16::get_bias(), 15);
    assert_eq!(FP32::get_bias(), 127);
    assert_eq!(FP64::get_bias(), 1023);

    let a: Float<6, 10> = Float::new(false, 2, 12);
    let mut b = a;
    b.set_exp(b.get_exp());
    assert_eq!(a.get_exp(), b.get_exp());
}

#[test]
fn test_conversion_wide_range() {
    for i in 0..(1 << 16) {
        let val = f32::from_bits(i << 16);
        let a = FP64::from_f32(val);
        let b: FP32 = a.cast();
        let res = b.as_f32();
        assert_eq!(res.to_bits(), (i << 16));
    }
}

#[test]
fn constructor_test() {
    let values: [u32; 5] =
        [0x3f8fffff, 0x40800000, 0x3f000000, 0xc60b40ec, 0xbc675793];

    for i in 0..5 {
        let output = f32::from_bits(values[i]);
        let a = FP64::from_f32(output);
        let b: FP32 = a.cast();
        assert_eq!(a.as_f32(), output);
        assert_eq!(b.as_f32(), output);
    }
}

#[test]
fn test_from_integers() {
    assert_eq!(FP64::from_i64(0).as_f64(), 0.);

    for i in -100..100 {
        let a = FP64::from_i64(i);
        let b = FP64::from_f64(i as f64);
        assert_eq!(a.as_f64(), b.as_f64());
    }
}

#[test]
fn test_nan_inf() {
    assert_eq!(FP64::zero(false).as_f64(), 0.0);
    assert_eq!(FP64::zero(true).as_f64(), -0.0);

    {
        let a = FP32::from_f32(f32::from_bits(0x3f8fffff));
        assert!(!a.is_inf());
        assert!(!a.is_nan());
        assert!(!a.is_negative());
    }
    {
        let a = FP32::from_f32(f32::from_bits(0xf48fffff));
        assert!(!a.is_inf());
        assert!(!a.is_nan());
        assert!(a.is_negative());
    }
    {
        let a = FP32::from_f32(f32::from_bits(0xff800000)); // -Inf
        a.dump();
        assert!(a.is_inf());
        assert!(!a.is_nan());
        assert!(a.is_negative());
    }
    {
        let a = FP32::from_f32(f32::from_bits(0xffc00000)); // -Nan.
        assert!(!a.is_inf());
        assert!(a.is_nan());
        assert!(a.is_negative());
    }

    {
        let mut a = FP64::from_f64(f64::from_bits((mask(32) << 32) as u64));
        assert!(!a.is_inf());
        assert!(a.is_nan());
        a.set_mantissa(0);
        assert!(a.is_inf());
        assert!(!a.is_nan());
        assert!(a.is_negative());
    }
    {
        // Check that casting propagates inf/nan.
        let a = FP32::from_f32(f32::from_bits(0xff800000)); // -Inf
        let b: FP64 = a.cast();
        assert!(b.is_inf());
        assert!(!b.is_nan());
        assert!(b.is_negative());
    }
}

// See Chapter 8. Algorithms for the Five Basic Operations -- Pg 248
pub fn add<const E: usize, const M: usize>(
    x: Float<E, M>,
    y: Float<E, M>,
) -> Float<E, M> {
    if y.get_exp() > x.get_exp() {
        return add(y, x);
    }

    assert!(x.get_exp() >= y.get_exp());
    assert!(!x.in_special_exp() && !y.in_special_exp());

    // Mantissa alignment.
    let exp_delta = x.get_exp() - y.get_exp();
    let mut er = x.get_exp();

    // Addition of the mantissa.

    let y_significand = y.get_mantissa() >> exp_delta.min(63);
    let x_significand = x.get_mantissa();

    let mut is_neg = x.is_negative();

    let is_plus = x.get_sign() == y.get_sign();

    let mut xy_significand;
    if is_plus {
        let res = x_significand.overflowing_add(y_significand);
        xy_significand = res.0;
        if res.1 {
            xy_significand >>= 1;
            xy_significand |= 1 << 63; // Set the implicit bit the overflowed.
            er += 1;
        }
    } else {
        if y_significand > x_significand {
            xy_significand = y_significand - x_significand;
            is_neg ^= true;
        } else {
            xy_significand = x_significand - y_significand;
        }
        // Cancellation happened, we need to normalize the number.
        // Shift xy_significant to the left, and subtract from the exponent
        // until you underflow or until xy_sig is normalized.
        let lz = xy_significand.leading_zeros() as u64;
        let lower_bound = Float::<E, M>::get_exp_bounds().0;
        // How far can we lower the exponent.
        let delta_to_min = er - lower_bound;
        let shift = delta_to_min.min(lz as i64).min(63);
        xy_significand <<= shift;
        er -= shift;
    }

    // Handle the case of cancellation (zero or very close to zero).
    if xy_significand == 0 {
        let mut r = Float::<E, M>::default();
        r.set_mantissa(0);
        r.set_unbiased_exp(0);
        r.set_sign(is_neg);
        return r;
    }

    let mut r = Float::<E, M>::default();
    r.set_mantissa(xy_significand);
    r.set_exp(er);
    r.set_sign(is_neg);
    r
}
#[test]
fn test_addition() {
    fn add_helper(a: f64, b: f64) -> f64 {
        let a = FP64::from_f64(a);
        let b = FP64::from_f64(b);
        let c = add(a, b);
        c.as_f64()
    }

    assert_eq!(add_helper(1., 1.), 2.);
    assert_eq!(add_helper(8., 4.), 12.);
    assert_eq!(add_helper(8., 4.), 12.);
    assert_eq!(add_helper(128., 2.), 130.);
    assert_eq!(add_helper(128., -8.), 120.);
    assert_eq!(add_helper(64., -60.), 4.);
    assert_eq!(add_helper(69., -65.), 4.);
    assert_eq!(add_helper(69., 69.), 138.);
    assert_eq!(add_helper(69., 1.), 70.);
    assert_eq!(add_helper(-128., -8.), -136.);
    assert_eq!(add_helper(64., -65.), -1.);
    assert_eq!(add_helper(-64., -65.), -129.);
    assert_eq!(add_helper(-15., -15.), -30.);

    assert_eq!(add_helper(-15., 15.), 0.);

    for i in -4..15 {
        for j in i..15 {
            assert_eq!(
                add_helper(f64::from(j), f64::from(i)),
                f64::from(i) + f64::from(j)
            );
        }
    }
}
