//! Transcendental functions built from arithmetic only, so the native test
//! build and the WebAssembly build agree bit for bit. Accuracy is plenty for
//! a simulation (sin/cos ~1e-6, tanh ~1e-4) and they are faster than libm.

pub const PI: f32 = core::f32::consts::PI;
pub const TAU: f32 = core::f32::consts::TAU;

/// Wrap an angle into [-PI, PI).
#[inline]
pub fn wrap_angle(a: f32) -> f32 {
    let mut a = a;
    // Angles in this sim never stray far, so a couple of conditional
    // subtractions beat a floor() call and stay exact.
    while a >= PI {
        a -= TAU;
    }
    while a < -PI {
        a += TAU;
    }
    a
}

/// Odd minimax polynomial for sin on [-PI/2, PI/2].
#[inline]
fn sin_core(x: f32) -> f32 {
    let x2 = x * x;
    // Coefficients from a degree-9 minimax fit; max abs error ~ 5e-7.
    let p = -2.390_511_3e-8_f32;
    let p = p * x2 + 2.752_567_1e-6;
    let p = p * x2 - 1.984_090_1e-4;
    let p = p * x2 + 8.333_321_6e-3;
    let p = p * x2 - 1.666_665_5e-1;
    let p = p * x2 + 1.0;
    x * p
}

#[inline]
pub fn sin(a: f32) -> f32 {
    let x = wrap_angle(a);
    // Fold [-PI, PI] onto [-PI/2, PI/2]: sin(x) = sin(PI - x).
    let x = if x > PI * 0.5 {
        PI - x
    } else if x < -PI * 0.5 {
        -PI - x
    } else {
        x
    };
    sin_core(x)
}

#[inline]
pub fn cos(a: f32) -> f32 {
    sin(a + PI * 0.5)
}

#[inline]
pub fn sin_cos(a: f32) -> (f32, f32) {
    (sin(a), cos(a))
}

/// Rational (Lambert continued-fraction) tanh, clamped to +-1 beyond |x| ~ 4.97
/// where the rational form would exceed 1 in magnitude.
#[inline]
pub fn tanh(x: f32) -> f32 {
    if x > 4.97 {
        return 1.0;
    }
    if x < -4.97 {
        return -1.0;
    }
    let x2 = x * x;
    let num = x * (135_135.0 + x2 * (17_325.0 + x2 * (378.0 + x2)));
    let den = 135_135.0 + x2 * (62_370.0 + x2 * (3_150.0 + x2 * 28.0));
    num / den
}

/// Circular distance between two hues in [0,1), result in [0, 0.5].
#[inline]
pub fn hue_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).abs();
    if d > 0.5 {
        1.0 - d
    } else {
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_cos_match_std() {
        let mut worst = 0.0f32;
        let mut a = -10.0f32;
        while a < 10.0 {
            worst = worst.max((sin(a) - a.sin()).abs());
            worst = worst.max((cos(a) - a.cos()).abs());
            a += 0.0137;
        }
        assert!(worst < 2e-6, "worst {worst}");
    }

    #[test]
    fn tanh_matches_std() {
        let mut worst = 0.0f32;
        let mut x = -8.0f32;
        while x < 8.0 {
            worst = worst.max((tanh(x) - x.tanh()).abs());
            x += 0.011;
        }
        assert!(worst < 2e-4, "worst {worst}"); // error peaks near the clamp at |x|~4.97
        assert_eq!(tanh(100.0), 1.0);
        assert_eq!(tanh(-100.0), -1.0);
    }

    #[test]
    fn wrap_angle_range() {
        for i in -100..100 {
            let a = i as f32 * 0.37;
            let w = wrap_angle(a);
            assert!((-PI..PI).contains(&w), "{a} -> {w}");
        }
    }

    #[test]
    fn hue_distance_wraps() {
        assert!((hue_distance(0.05, 0.95) - 0.1).abs() < 1e-6);
        assert!((hue_distance(0.2, 0.5) - 0.3).abs() < 1e-6);
    }
}
