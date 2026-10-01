//! Mouse sensitivity math. Every conversion goes through degrees of rotation per mouse count:
//! `degrees_per_count = yaw * sensitivity`, so a game is fully described by its yaw constant.
//!
//! The functions are pure and allocation free so the same code runs natively and in WebAssembly.

#[cfg(feature = "data")]
mod games;
#[cfg(feature = "data")]
pub use games::{find, load, Game, GamesError};

const CM_PER_INCH: f64 = 2.54;

/// Physical distance in centimetres to turn 360 degrees.
pub fn cm_per_360(sens: f64, dpi: f64, yaw: f64) -> f64 {
    360.0 / (yaw * sens * dpi) * CM_PER_INCH
}

/// In-game sensitivity that produces `cm360` centimetres per 360 degrees.
pub fn sens_for_cm_per_360(cm360: f64, dpi: f64, yaw: f64) -> f64 {
    360.0 * CM_PER_INCH / (yaw * dpi * cm360)
}

/// Ratio `dst_cm360 / src_cm360` that keeps a point on screen under the same hand movement.
///
/// `coefficient` picks the point, as a fraction of the half-screen width measured from the
/// centre: 0.0 matches the centre of the screen (ratio of the FOV tangents), 1.0 matches the
/// screen edge (ratio of the FOV angles), and values between are monitor-distance matches.
/// Both FOVs are horizontal, in degrees, and must lie strictly between 0 and 180.
pub fn fov_cm_ratio(src_hfov: f64, dst_hfov: f64, coefficient: f64) -> f64 {
    let src_half = (src_hfov / 2.0).to_radians();
    let dst_half = (dst_hfov / 2.0).to_radians();
    if coefficient <= 0.0 {
        return src_half.tan() / dst_half.tan();
    }
    let src_angle = (coefficient * src_half.tan()).atan();
    let dst_angle = (coefficient * dst_half.tan()).atan();
    src_angle / dst_angle
}

/// How the destination sensitivity is chosen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Method {
    /// Same cm/360. FOV is ignored.
    Cm360,
    /// Match a point on screen; needs both horizontal FOVs.
    Fov {
        src_hfov: f64,
        dst_hfov: f64,
        coefficient: f64,
    },
}

/// One side of a conversion.
#[derive(Clone, Copy, Debug)]
pub struct Setup {
    pub yaw: f64,
    pub dpi: f64,
}

/// Result of [`convert`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conversion {
    pub dst_sens: f64,
    pub src_cm360: f64,
    pub dst_cm360: f64,
}

/// Converts `sens` on `src` to the equivalent sensitivity on `dst`.
pub fn convert(src: Setup, dst: Setup, sens: f64, method: Method) -> Conversion {
    let src_cm360 = cm_per_360(sens, src.dpi, src.yaw);
    let dst_cm360 = match method {
        Method::Cm360 => src_cm360,
        Method::Fov {
            src_hfov,
            dst_hfov,
            coefficient,
        } => src_cm360 * fov_cm_ratio(src_hfov, dst_hfov, coefficient),
    };
    Conversion {
        dst_sens: sens_for_cm_per_360(dst_cm360, dst.dpi, dst.yaw),
        src_cm360,
        dst_cm360,
    }
}

/// C ABI for the web page. Plain numbers in and out, so no glue generator is needed.
///
/// `method` 0 is cm/360, anything else is FOV matching. Returns NaN for non-finite or
/// non-positive inputs.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn sc_convert(
    src_yaw: f64,
    dst_yaw: f64,
    sens: f64,
    src_dpi: f64,
    dst_dpi: f64,
    method: i32,
    coefficient: f64,
    src_hfov: f64,
    dst_hfov: f64,
) -> f64 {
    let positive = |v: f64| v.is_finite() && v > 0.0;
    if !(positive(src_yaw)
        && positive(dst_yaw)
        && positive(sens)
        && positive(src_dpi)
        && positive(dst_dpi))
    {
        return f64::NAN;
    }
    let method = if method == 0 {
        Method::Cm360
    } else {
        let fov_ok = |v: f64| v.is_finite() && v > 0.0 && v < 180.0;
        if !(fov_ok(src_hfov) && fov_ok(dst_hfov) && coefficient.is_finite()) {
            return f64::NAN;
        }
        Method::Fov {
            src_hfov,
            dst_hfov,
            coefficient,
        }
    };
    convert(
        Setup {
            yaw: src_yaw,
            dpi: src_dpi,
        },
        Setup {
            yaw: dst_yaw,
            dpi: dst_dpi,
        },
        sens,
        method,
    )
    .dst_sens
}

/// cm/360 for the web page.
#[no_mangle]
pub extern "C" fn sc_cm360(sens: f64, dpi: f64, yaw: f64) -> f64 {
    cm_per_360(sens, dpi, yaw)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    fn setup(yaw: f64, dpi: f64) -> Setup {
        Setup { yaw, dpi }
    }

    #[test]
    fn cs_800dpi_sens_1_is_about_52_cm() {
        assert!(close(cm_per_360(1.0, 800.0, 0.022), 51.96, 0.01));
    }

    #[test]
    fn cs_to_valorant_is_yaw_ratio() {
        let c = convert(setup(0.022, 800.0), setup(0.07, 800.0), 1.0, Method::Cm360);
        assert!(close(c.dst_sens, 0.022 / 0.07, 1e-12));
        assert!(close(c.src_cm360, c.dst_cm360, 1e-12));
    }

    #[test]
    fn dpi_change_scales_sensitivity_inversely() {
        let c = convert(setup(0.022, 800.0), setup(0.022, 1600.0), 2.0, Method::Cm360);
        assert!(close(c.dst_sens, 1.0, 1e-12));
    }

    #[test]
    fn round_trip_is_identity() {
        let (a, b) = (setup(0.0066, 800.0), setup(0.07, 1600.0));
        let there = convert(a, b, 5.0, Method::Cm360).dst_sens;
        let back = convert(b, a, there, Method::Cm360).dst_sens;
        assert!(close(back, 5.0, 1e-9));
    }

    #[test]
    fn same_fov_gives_ratio_one_at_every_coefficient() {
        for c in [0.0, 0.25, 0.75, 1.0] {
            assert!(close(fov_cm_ratio(103.0, 103.0, c), 1.0, 1e-12));
        }
    }

    #[test]
    fn edge_match_is_ratio_of_fov_angles() {
        assert!(close(fov_cm_ratio(90.0, 120.0, 1.0), 90.0 / 120.0, 1e-9));
    }

    #[test]
    fn centre_match_is_ratio_of_fov_tangents() {
        let want = 45f64.to_radians().tan() / 60f64.to_radians().tan();
        assert!(close(fov_cm_ratio(90.0, 120.0, 0.0), want, 1e-12));
    }

    #[test]
    fn wider_destination_fov_turns_faster() {
        // A wider FOV compresses the image, so the same screen point needs more rotation per
        // centimetre: the destination cm/360 shrinks.
        assert!(fov_cm_ratio(90.0, 120.0, 0.75) < 1.0);
        assert!(fov_cm_ratio(120.0, 90.0, 0.75) > 1.0);
    }

    #[test]
    fn fov_method_changes_the_result() {
        let (a, b) = (setup(0.022, 800.0), setup(0.07, 800.0));
        let plain = convert(a, b, 1.0, Method::Cm360).dst_sens;
        let fov = convert(
            a,
            b,
            1.0,
            Method::Fov {
                src_hfov: 106.26,
                dst_hfov: 103.0,
                coefficient: 0.0,
            },
        )
        .dst_sens;
        assert!(!close(fov, plain, 1e-6));
        assert!(fov > plain * 0.9 && fov < plain * 1.2);
    }

    #[test]
    fn c_abi_rejects_bad_input() {
        assert!(sc_convert(0.022, 0.07, 0.0, 800.0, 800.0, 0, 0.0, 0.0, 0.0).is_nan());
        assert!(sc_convert(0.022, 0.07, 1.0, 800.0, 800.0, 1, 0.5, 200.0, 100.0).is_nan());
        assert!(sc_convert(0.022, 0.07, 1.0, 800.0, 800.0, 0, 0.0, 0.0, 0.0).is_finite());
    }
}
