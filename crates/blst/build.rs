#![allow(unused_imports)]

extern crate cc;

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

fn assembly(
    file_vec: &mut Vec<PathBuf>,
    base_dir: &Path,
    _arch: &str,
    _is_msvc: bool,
) {
    #[cfg(target_env = "msvc")]
    if _is_msvc {
        let sfx = match _arch {
            "x86_64" => "x86_64",
            "aarch64" => "armv8",
            _ => "unknown",
        };
        let files =
            glob::glob(&format!("{}/win64/*-{}.asm", base_dir.display(), sfx))
                .expect("unable to collect assembly files");
        for file in files {
            file_vec.push(file.unwrap());
        }
        return;
    }

    file_vec.push(base_dir.join("assembly.S"));
}

fn main() {
    if env::var("CARGO_FEATURE_SERDE_SECRET").is_ok() {
        println!(
            "cargo:warning=blst: non-production feature serde-secret enabled"
        );
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();

    let target_no_std = target_os.eq("none")
        || (target_os.eq("unknown") && target_arch.eq("wasm32"))
        || target_os.eq("uefi")
        || env::var("BLST_TEST_NO_STD").is_ok();

    if !target_no_std {
        println!("cargo:rustc-cfg=feature=\"std\"");
        if target_arch.eq("wasm32") || target_os.eq("unknown") {
            println!("cargo:rustc-cfg=feature=\"no-threads\"");
        }
    }
    println!("cargo:rerun-if-env-changed=BLST_TEST_NO_STD");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().and_then(|p| p.parent());

    if Path::new("libblst.a").exists() {
        println!("cargo:rustc-link-search=.");
        println!("cargo:rustc-link-lib=blst");
        println!("cargo:rerun-if-changed=libblst.a");
        return;
    }

    let mut blst_base_dir = manifest_dir.join("blst");
    if !blst_base_dir.exists() {
        blst_base_dir = workspace_root
            .map(|r| r.join("blst-src"))
            .unwrap_or_else(|| {
                manifest_dir
                    .parent()
                    .and_then(|dir| dir.parent())
                    .expect("can't access workspace root or blst repo")
                    .join("blst-src")
            });
    }

    // When vroom feature is enabled, build from a staging copy that uses VROOM's pairing.c
    let use_vroom = env::var("CARGO_FEATURE_VROOM").is_ok();
    if use_vroom {
        if let Some(ws) = workspace_root {
            let vroom_pairing = ws.join("vroom").join("blst").join("pairing.c");
            let blst_src = ws.join("blst-src");
            if vroom_pairing.exists() && blst_src.exists() {
                let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
                let staging = out_dir.join("blst_vroom_build");
                let staging_src = staging.join("src");
                let staging_build = staging.join("build");

                println!("cargo:warning=blst: building with VROOM pairing (vroom/blst/pairing.c)");
                fs::create_dir_all(&staging_src).expect("create staging src");
                fs::create_dir_all(&staging_build).expect("create staging build");

                copy_dir_all(&blst_src.join("src"), &staging_src)
                    .expect("copy blst src to staging");
                copy_dir_all(&blst_src.join("build"), &staging_build)
                    .expect("copy blst build to staging");
                fs::copy(&vroom_pairing, staging_src.join("pairing.c"))
                    .expect("copy VROOM pairing.c");

                println!("cargo:rerun-if-changed={}", vroom_pairing.display());
                blst_base_dir = staging;
            }
        }
    }

    if !blst_base_dir.exists() {
        panic!(
            "blst source not found at {} (set vroom feature and init submodules, or add blst-src)",
            blst_base_dir.display()
        );
    }
    println!("Using blst source directory {}", blst_base_dir.display());

    if target_os.eq("uefi") && env::var("CC").is_err() {
        match std::process::Command::new("clang").arg("--version").output() {
            Ok(_) => env::set_var("CC", "clang"),
            Err(_) => {}
        }
    }

    if target_env.eq("sgx") && env::var("CC").is_err() {
        if let Ok(out) = std::process::Command::new("clang").arg("--version").output() {
            let version = String::from_utf8(out.stdout).unwrap_or_default();
            if let Some(x) = version.find("clang version ") {
                let x = x + 14;
                let y = version[x..].find('.').unwrap_or(0);
                if version[x..x + y].parse::<i32>().unwrap_or(0) >= 11 {
                    env::set_var("CC", "clang");
                }
            }
        }
    }

    if target_env.eq("msvc")
        && env::var("CARGO_CFG_TARGET_POINTER_WIDTH").unwrap().eq("32")
        && env::var("CC").is_err()
    {
        if let Ok(out) =
            std::process::Command::new("clang-cl").args(["-m32", "--version"]).output()
        {
            if String::from_utf8(out.stdout).unwrap_or_default()
                .contains("Target: i386-pc-windows-msvc")
            {
                env::set_var("CC", "clang-cl");
            }
        }
    }

    let mut cc = cc::Build::new();

    let c_src_dir = blst_base_dir.join("src");
    println!("cargo:rerun-if-changed={}", c_src_dir.display());
    let mut file_vec = vec![c_src_dir.join("server.c")];

    if target_arch.eq("x86_64") || target_arch.eq("aarch64") {
        let asm_dir = blst_base_dir.join("build");
        println!("cargo:rerun-if-changed={}", asm_dir.display());
        assembly(
            &mut file_vec,
            &asm_dir,
            &target_arch,
            cc.get_compiler().is_like_msvc(),
        );
    } else {
        cc.define("__BLST_NO_ASM__", None);
    }
    match (cfg!(feature = "portable"), cfg!(feature = "force-adx")) {
        (true, false) => {
            if target_arch.eq("x86_64") && target_env.eq("sgx") {
                panic!("'portable' is not supported on SGX target");
            }
            println!("cargo:warning=blst: compiling in portable mode");
            cc.define("__BLST_PORTABLE__", None);
        }
        (false, true) => {
            if target_arch.eq("x86_64") {
                cc.define("__ADX__", None);
            }
        }
        (false, false) => {
            if target_arch.eq("x86_64") {
                if target_env.eq("sgx") {
                    cc.define("__ADX__", None);
                } else if env::var("CARGO_ENCODED_RUSTFLAGS")
                    .unwrap_or_default()
                    .contains("target-cpu=")
                {
                    let feat_list = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
                    let features: Vec<_> = feat_list.split(',').collect();
                    if !features.contains(&"ssse3") {
                        cc.define("__BLST_PORTABLE__", None);
                    } else if features.contains(&"adx") {
                        cc.define("__ADX__", None);
                    }
                } else {
                    #[cfg(target_arch = "x86_64")]
                    if std::is_x86_feature_detected!("adx") {
                        cc.define("__ADX__", None);
                    }
                }
            }
        }
        (true, true) => panic!("Cannot compile with both `portable` and `force-adx` features"),
    }
    if target_env.eq("msvc") && cc.get_compiler().is_like_msvc() {
        cc.flag("-Zl");
    }
    cc.flag_if_supported("-mno-avx")
        .flag_if_supported("-fno-builtin")
        .flag_if_supported("-Wno-unused-function")
        .flag_if_supported("-Wno-unused-command-line-argument");
    if target_arch.eq("wasm32") || target_family.is_empty() {
        cc.flag("-ffreestanding");
    }
    if target_arch.eq("wasm32") || target_no_std {
        cc.define("SCRATCH_LIMIT", "(45 * 1024)");
    }
    if target_env.eq("sgx") {
        cc.flag_if_supported("-mlvi-hardening");
        cc.define("__SGX_LVI_HARDENING__", None);
        cc.define("__BLST_NO_CPUID__", None);
        cc.define("__ELF__", None);
        cc.define("SCRATCH_LIMIT", "(45 * 1024)");
    }
    if !cfg!(debug_assertions) {
        cc.opt_level(2);
    }
    cc.files(&file_vec).compile("blst");

    let bindings = blst_base_dir.join("bindings");
    if bindings.exists() {
        println!("cargo:BINDINGS={}", bindings.to_string_lossy());
    }
    println!("cargo:C_SRC={}", c_src_dir.to_string_lossy());
}
