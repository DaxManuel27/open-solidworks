//! Opt-in, bounded feasibility experiments. These are not modeling commands.

#[cxx::bridge(namespace = "ofk::probes")]
pub mod ffi {
    #[derive(Debug, Clone, Copy)]
    struct Measurement {
        valid: bool,
        solids: u32,
        volume: f64,
        expected_volume: f64,
        has_expected_volume: bool,
        max_gap: f64,
        fit_tolerance: f64,
        fit_passed: bool,
        normal_error: f64,
        has_normal_check: bool,
        source_faces: u32,
        retained_source_faces: u32,
        cut_valid: bool,
        cut_volume: f64,
        cut_history_faces: u32,
        fillet_valid: bool,
        elapsed_ms: f64,
    }

    unsafe extern "C++" {
        include!("of-kernel-sys/cpp/probes.h");
        fn kernel_version() -> String;
        /// Cases: 0 four-boundary patch, 1 planar face blend, 2 spherical dome,
        /// 3 cylindrical wrap, 4 auxiliary-guide sweep, 5 guided sections.
        /// Variant 0 is analytic; 1/2 perturb the defining inputs; 3 is invalid.
        fn measure(which: u8, variant: u8, scale: f64) -> Result<Measurement>;
    }
}
