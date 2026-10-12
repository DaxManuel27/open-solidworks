#![cfg(feature = "kernel-probes")]

use of_kernel_sys::probes::ffi;

#[test]
fn measured_solids_and_downstream_consumers() {
    for scale in [0.1_f64, 1.0, 10.0] {
        for case in 0..6 {
            for variant in 0..3 {
                let m = ffi::measure(case, variant, scale).unwrap();
                assert!(m.valid && m.solids == 1, "{case}/{variant}/{scale}: {m:?}");
                assert!(m.volume.is_finite() && m.volume > 0.0);
                if m.has_expected_volume {
                    assert!(
                        (m.volume - m.expected_volume).abs() < 2e-6 * m.expected_volume,
                        "{case}/{variant}/{scale}: {m:?}"
                    );
                }
                assert!(m.cut_valid && m.cut_volume > 0.0 && m.cut_volume < m.volume);
                assert!(m.cut_history_faces > 0);
                assert!(
                    m.fillet_valid,
                    "no downstream fillet: {case}/{variant}/{scale}"
                );
                assert!(m.max_gap.is_finite() && m.max_gap >= 0.0);
                if case == 5 && variant == 1 {
                    // A valid solid is NOT evidence of guide interpolation. Preserve
                    // this counterexample until a constrained algorithm replaces it.
                    assert!(
                        !m.fit_passed,
                        "coarse guide sampling unexpectedly fits: {m:?}"
                    );
                } else {
                    assert!(
                        m.fit_passed,
                        "boundary/guide mismatch: {case}/{variant}/{scale}: {m:?}"
                    );
                }
                if m.has_normal_check {
                    assert!(m.normal_error.is_finite() && m.normal_error < 1e-7);
                }
            }
        }
    }
}

#[test]
fn both_boundary_directions_control_the_result() {
    let flat = ffi::measure(0, 0, 1.0).unwrap();
    for direction in [1, 2] {
        let curved = ffi::measure(0, direction, 1.0).unwrap();
        assert!(curved.volume > flat.volume + 30.0);
        assert_eq!(curved.source_faces, 6);
        assert_eq!(curved.retained_source_faces, 6);
    }
}

#[test]
fn invalid_geometry_and_probe_arguments_are_rejected() {
    for case in 0..6 {
        assert!(ffi::measure(case, 3, 1.0).is_err(), "invalid case {case}");
    }
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY, 101.0] {
        assert!(ffi::measure(0, 0, scale).is_err());
    }
    assert!(ffi::measure(6, 0, 1.0).is_err());
    assert!(ffi::measure(0, 4, 1.0).is_err());
}
