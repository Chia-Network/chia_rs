use std::{env, fs, path::PathBuf};

use chia_rs::bindings;
use chia_traits::StubBuilder;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = PathBuf::from(manifest_dir)
        .join("python")
        .join("chia_rs")
        .join("chia_rs.pyi");

    let initial_stubs = stubs();

    for _ in 0..1000 {
        let new_stubs = stubs();
        assert_eq!(
            initial_stubs, new_stubs,
            "the order of the generated type stubs is not stable between generations"
        );
    }

    fs::write(path, initial_stubs).unwrap();
}

fn stubs() -> String {
    let builder = StubBuilder::default();
    bindings(&builder).unwrap();
    builder.generate()
}
