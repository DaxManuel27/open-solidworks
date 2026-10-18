//! Compiles the C++ bridge and links OpenCASCADE.
//!
//! OpenCASCADE is found, in order, from `OCCT_INCLUDE_DIR` / `OCCT_LIB_DIR`,
//! an active conda environment (`CONDA_PREFIX`), Homebrew, or the system
//! (`/usr/include/opencascade`).

use std::env;
use std::path::{Path, PathBuf};

/// Libraries OpenForge needs. OpenCASCADE 7.8 merged the data exchange
/// libraries, so older installs use different names for the last group.
const CORE_LIBS: &[&str] = &[
    "TKernel",
    "TKMath",
    "TKG2d",
    "TKG3d",
    "TKGeomBase",
    "TKGeomAlgo",
    "TKBRep",
    "TKTopAlgo",
    "TKPrim",
    "TKBO",
    "TKBool",
    "TKFillet",
    "TKOffset",
    "TKShHealing",
    "TKMesh",
    "TKFeat",
    "TKHLR",
    "TKXSBase",
    "TKCDF",
    "TKLCAF",
    "TKCAF",
    "TKXCAF",
];
const DE_LIBS: &[&str] = &["TKDE", "TKDESTEP", "TKDEIGES", "TKDESTL"];
const LEGACY_DE_LIBS: &[&str] = &[
    "TKSTEP",
    "TKSTEPBase",
    "TKSTEPAttr",
    "TKSTEP209",
    "TKIGES",
    "TKSTL",
    "TKXDESTEP",
];

fn main() {
    let (include, lib) = find_occt().unwrap_or_else(|| {
        panic!(
            "OpenCASCADE not found. Install it (brew install opencascade, \
             conda install -c conda-forge occt, or apt install libocct-*-dev) \
             or set OCCT_INCLUDE_DIR and OCCT_LIB_DIR."
        )
    });

    let probes = env::var_os("CARGO_FEATURE_KERNEL_PROBES").is_some();
    let mut bridges = vec!["src/lib.rs"];
    if probes {
        bridges.push("src/probes.rs");
    }
    let mut build = cxx_build::bridges(bridges);
    if probes {
        build.file("cpp/probes.cpp");
    }
    build
        .file("cpp/ofk.cpp")
        .file("cpp/dome.cpp")
        .file("cpp/wrap.cpp")
        .file("cpp/boundary.cpp")
        .file("cpp/thread.cpp")
        .file("cpp/skin.cpp")
        .file("cpp/assembly.cpp")
        .file("cpp/hlr.cpp")
        .include(&include)
        .std("c++17")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-deprecated-declarations")
        .flag_if_supported("/EHsc")
        .flag_if_supported("/bigobj");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        build.define("WNT", None);
    }
    build.compile("ofk");

    println!("cargo:rustc-link-search=native={}", lib.display());
    let de = if has_lib(&lib, "TKDESTEP") {
        DE_LIBS
    } else {
        LEGACY_DE_LIBS
    };
    for name in CORE_LIBS.iter().chain(de) {
        println!("cargo:rustc-link-lib=dylib={name}");
    }
    println!("cargo:lib_dir={}", lib.display());
    println!("cargo:include_dir={}", include.display());

    println!("cargo:rerun-if-changed=cpp/ofk.h");
    println!("cargo:rerun-if-changed=cpp/ofk.cpp");
    println!("cargo:rerun-if-changed=cpp/dome.cpp");
    println!("cargo:rerun-if-changed=cpp/wrap.cpp");
    println!("cargo:rerun-if-changed=cpp/boundary.cpp");
    println!("cargo:rerun-if-changed=cpp/thread.cpp");
    println!("cargo:rerun-if-changed=cpp/skin.cpp");
    println!("cargo:rerun-if-changed=cpp/assembly.cpp");
    println!("cargo:rerun-if-changed=cpp/hlr.cpp");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/probes.rs");
    println!("cargo:rerun-if-changed=cpp/probes.h");
    println!("cargo:rerun-if-changed=cpp/probes.cpp");
    for var in ["OCCT_INCLUDE_DIR", "OCCT_LIB_DIR", "CONDA_PREFIX"] {
        println!("cargo:rerun-if-env-changed={var}");
    }
}

fn find_occt() -> Option<(PathBuf, PathBuf)> {
    if let (Ok(i), Ok(l)) = (env::var("OCCT_INCLUDE_DIR"), env::var("OCCT_LIB_DIR")) {
        return Some((i.into(), l.into()));
    }
    let mut prefixes: Vec<PathBuf> = Vec::new();
    if let Ok(conda) = env::var("CONDA_PREFIX") {
        prefixes.push(PathBuf::from(&conda).join("Library")); // Windows layout
        prefixes.push(conda.into());
    }
    prefixes.extend(
        [
            "/opt/homebrew/opt/opencascade",
            "/usr/local/opt/opencascade",
            "/usr/local",
            "/usr",
        ]
        .map(PathBuf::from),
    );
    for p in prefixes {
        let include = p.join("include/opencascade");
        if !include.join("Standard.hxx").exists() {
            continue;
        }
        let candidates = [
            p.join("lib"),
            p.join("lib64"),
            p.join("lib/x86_64-linux-gnu"),
            p.join("lib/aarch64-linux-gnu"),
        ];
        if let Some(lib) = candidates.into_iter().find(|l| has_lib(l, "TKernel")) {
            return Some((include, lib));
        }
    }
    None
}

fn has_lib(dir: &Path, name: &str) -> bool {
    [
        format!("lib{name}.dylib"),
        format!("lib{name}.so"),
        format!("{name}.lib"),
    ]
    .iter()
    .any(|f| dir.join(f).exists())
}
