use std::path::PathBuf;
use std::str::FromStr;
use std::{env, fs};

use cmake::Config;

macro_rules! p {
    ($($tokens: tt)*) => {
        println!("cargo:warning={}", format!($($tokens)*))
    }
}

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=../src/c_bindings/c_wrapper.h");
    println!("cargo:rerun-if-changed=../src/c_bindings/c_wrapper.cpp");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let mut src_dir = manifest_dir.join("cpp");
    if !src_dir.exists() {
        src_dir = manifest_dir
            .parent()
            .expect("can't access ../")
            .join("src")
            .to_path_buf();
    }

    let dst = Config::new(src_dir.as_path())
        .build_target("chiavdfc_static")
        .define("BUILD_CHIAVDFC", "ON")
        .env("BUILD_VDF_CLIENT", "N")
        .define("BUILD_PYTHON", "OFF")
        .build();

    let search = PathBuf::from_str(dst.display().to_string().as_str())
        .unwrap()
        .join("build")
        .join("lib")
        .join("static");

    let search = search.to_str().unwrap();

    p!("{search}");

    fs::read_dir(search).unwrap().for_each(|path| {
        p!("{:?}", path.unwrap().path());
    });

    println!("cargo:rustc-link-search=native={}", search);
    println!("cargo:rustc-link-lib=static=chiavdfc");
    println!("cargo:rustc-link-lib=gmp");

    let bindings = bindgen::Builder::default()
        .header(manifest_dir.join("wrapper.h").to_str().unwrap())
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg(format!(
            "-I{}",
            src_dir.join("c_bindings").to_str().unwrap()
        ))
        .clang_arg("-std=c++14")
        .allowlist_function("verify_n_wesolowski_wrapper")
        .allowlist_function("create_discriminant_wrapper")
        .allowlist_function("prove_wrapper")
        .allowlist_function("free")
        .allowlist_function("delete_byte_array")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
