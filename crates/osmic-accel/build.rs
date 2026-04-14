//! Build script for compiling Metal geometry compute shaders.
//!
//! Compiles .metal files in src/kernels/ to a .metallib embedded in the binary.
//! Only runs on macOS; on other platforms this is a no-op.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let shaders_dir = manifest_dir.join("src").join("kernels");

    // Verify Metal toolchain
    let xcrun_ok = Command::new("xcrun").args(["--find", "metal"]).output();
    match xcrun_ok {
        Ok(output) if output.status.success() => {}
        _ => {
            println!("cargo:warning=Metal compiler not found, skipping shader compilation");
            return;
        }
    }

    // Find .metal files
    let metal_files: Vec<PathBuf> = std::fs::read_dir(&shaders_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()?.to_str()? == "metal" {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    if metal_files.is_empty() {
        println!("cargo:warning=No .metal files found in {:?}", shaders_dir);
        return;
    }

    // Compile each .metal to .air
    let mut air_files = Vec::new();
    for metal_file in &metal_files {
        let stem = metal_file.file_stem().unwrap().to_str().unwrap();
        let air_file = out_dir.join(format!("{}.air", stem));

        println!("cargo:rerun-if-changed={}", metal_file.display());

        let output = Command::new("xcrun")
            .args([
                "-sdk", "macosx", "metal",
                "-std=metal3.1",
                "-O3",
                "-ffast-math",
                "-target", "air64-apple-macos14.0",
                "-c", metal_file.to_str().unwrap(),
                "-o", air_file.to_str().unwrap(),
            ])
            .output()
            .expect("Failed to run Metal compiler");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "Failed to compile {}\n{}",
                metal_file.display(),
                stderr
            );
        }

        air_files.push(air_file);
    }

    // Link into .metallib
    let metallib = out_dir.join("osmic_geometry.metallib");
    let mut cmd = Command::new("xcrun");
    cmd.args(["-sdk", "macosx", "metallib"]);
    for air in &air_files {
        cmd.arg(air.to_str().unwrap());
    }
    cmd.args(["-o", metallib.to_str().unwrap()]);

    let output = cmd.output().expect("Failed to run metallib linker");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("Failed to link metallib\n{}", stderr);
    }

    println!("cargo:rerun-if-changed=src/kernels");
    println!(
        "cargo:warning=Compiled {} Metal shaders to osmic_geometry.metallib",
        metal_files.len()
    );
}
