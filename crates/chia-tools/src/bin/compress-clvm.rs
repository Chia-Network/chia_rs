use clap::Parser;
use clvmr::Allocator;
use clvmr::serde::{node_from_bytes, node_to_bytes_backrefs};
use std::fs::{File, read_to_string};
use std::io::Write;

/// Analyze the blocks in a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to a CLVM program, in hex form
    #[arg(short, long)]
    input: String,

    /// The file to write the compressed CLVM program to, in hex form
    #[arg(short, long)]
    output: String,
}

fn main() {
    let args = Args::parse();

    let input = hex::decode(
        read_to_string(args.input)
            .expect("failed to read input file")
            .trim(),
    )
    .expect("invalid hex in input file");
    let mut output = File::create(args.output).expect("failed to create output file");

    let mut a = Allocator::new();
    let input = node_from_bytes(&mut a, &input[..]).expect("failed to parse input file");

    let compressed = node_to_bytes_backrefs(&a, input).expect("failed to compress input file");
    write!(output, "{}", &hex::encode(compressed)).expect("failed to write to output file");
}
