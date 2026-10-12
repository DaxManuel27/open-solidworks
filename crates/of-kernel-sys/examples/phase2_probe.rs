#[cfg(feature = "kernel-probes")]
fn main() {
    use of_kernel_sys::probes::ffi;
    eprintln!("OpenCASCADE {}", ffi::kernel_version());
    println!(
        "case,variant,scale,valid,solids,volume,expected,max_gap,fit_tolerance,fit_passed,normal_error,source_faces,retained_faces,cut_valid,cut_volume,cut_history_faces,fillet_valid,elapsed_ms"
    );
    let mut errors = 0;
    for scale in [0.1, 1.0, 10.0] {
        for (case, name) in [
            "boundary",
            "planar_face_blend",
            "spherical_dome",
            "cylindrical_wrap",
            "auxiliary_sweep",
            "guided_sections",
        ]
        .iter()
        .enumerate()
        {
            for variant in 0..3 {
                match ffi::measure(case as u8, variant, scale) {
                    Ok(m) => println!(
                        "{name},{variant},{scale},{},{},{:.9},{},{:.9},{},{},{},{},{},{},{:.9},{},{},{:.3}",
                        m.valid,
                        m.solids,
                        m.volume,
                        if m.has_expected_volume {
                            m.expected_volume.to_string()
                        } else {
                            "n/a".into()
                        },
                        m.max_gap,
                        m.fit_tolerance,
                        m.fit_passed,
                        if m.has_normal_check {
                            m.normal_error.to_string()
                        } else {
                            "n/a".into()
                        },
                        m.source_faces,
                        m.retained_source_faces,
                        m.cut_valid,
                        m.cut_volume,
                        m.cut_history_faces,
                        m.fillet_valid,
                        m.elapsed_ms,
                    ),
                    Err(e) => {
                        eprintln!("{name}, variant {variant}, scale {scale}: {e}");
                        errors += 1;
                    }
                }
            }
        }
    }
    if errors > 0 {
        std::process::exit(1);
    }
}

#[cfg(not(feature = "kernel-probes"))]
fn main() {
    eprintln!("Run with --features kernel-probes to compile the geometry experiments.");
    std::process::exit(1);
}
