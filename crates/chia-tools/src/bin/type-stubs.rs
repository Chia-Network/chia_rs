use std::{env, fs, path::PathBuf};

use chia_bls::{BlsCache, G1Element, G2Element, GTElement, SecretKey};
use chia_traits::StubBuilder;

fn main() -> anyhow::Result<()> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let path = PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("wheel")
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

    fs::write(path, initial_stubs)?;

    Ok(())
}

fn stubs() -> String {
    let builder = StubBuilder::default();

    builder.stub::<BlsCache>();
    builder.stub::<G1Element>();
    builder.stub::<G2Element>();
    builder.stub::<GTElement>();
    builder.stub::<SecretKey>();

    builder.generate()
}
