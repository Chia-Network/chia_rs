use std::{env, fs, path::PathBuf};

use chia_bls::{BlsCache, G1Element, G2Element, GTElement};
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

    let mut builder = StubBuilder::default();
    builder.stub::<BlsCache>();
    builder.stub::<G1Element>();
    builder.stub::<G2Element>();
    builder.stub::<GTElement>();

    fs::write(path, builder.generate())?;

    Ok(())
}
