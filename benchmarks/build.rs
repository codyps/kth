use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(kth_native_simd)");
    println!("cargo:rerun-if-env-changed=KTH_X86_SIMD_SOURCE");
    println!("cargo:rerun-if-changed=native/simd.cpp");
    if env::var_os("CARGO_FEATURE_NATIVE_SIMD").is_none() {
        return;
    }
    let Some(source) = env::var_os("KTH_X86_SIMD_SOURCE") else {
        return;
    };
    let source = PathBuf::from(source);
    assert_eq!(env::var("CARGO_CFG_TARGET_ARCH").unwrap(), "x86_64");
    // Keep the external reference reproducible; updates require deliberate review.
    let revision = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&source)
        .output()
        .unwrap();
    assert!(revision.status.success());
    assert_eq!(
        String::from_utf8(revision.stdout).unwrap().trim(),
        "6e059b14975df2e3c74260ec905167a1be5a34ca"
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let object = out.join("simd.o");
    assert!(Command::new("g++")
        .args([
            "-std=c++17",
            "-O3",
            "-mavx512f",
            "-mavx512dq",
            "-mavx512vl",
            "-mavx512bw",
            "-mavx2",
            "-mpopcnt",
            "-fPIC",
            "-c",
            "native/simd.cpp",
            "-I"
        ])
        .arg(source.join("src"))
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap()
        .success());
    assert!(Command::new("ar")
        .arg("crs")
        .arg(out.join("libkth_experiment_simd.a"))
        .arg(object)
        .status()
        .unwrap()
        .success());
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=kth_experiment_simd");
    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-cfg=kth_native_simd");
}
