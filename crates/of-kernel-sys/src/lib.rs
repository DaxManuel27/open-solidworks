//! The single bridge between Rust and the C++ libraries OpenForge depends on:
//! OpenCASCADE (geometry kernel) and PlaneGCS (sketch solver).
//!
//! This is the only crate that touches C++. Everything above it uses safe Rust.
