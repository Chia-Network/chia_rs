use bitflags::Flags;
use chia_bls::Signature;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::ConsensusFlags;
use chia_consensus::run_block_generator::run_block_generator2;
use clap::Parser;
use std::fs::read_to_string;
use std::time::Instant;

/// Run a block generator program and print the resulting conditions
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to a file containing the generator program as hex
    file: String,

    /// Max cost for the generator run
    #[arg(long, default_value_t = 11_000_000_000)]
    max_cost: u64,

    /// Consensus flags to enable (may be repeated). Use --flag help to list them.
    #[arg(long = "flag")]
    flags: Vec<String>,
}

fn main() {
    #[cfg(debug_assertions)]
    eprintln!("WARNING: running in debug mode, timings will not be representative");

    let args = Args::parse();

    let hex_str =
        read_to_string(&args.file).unwrap_or_else(|e| panic!("failed to read {}: {e}", args.file));
    let program =
        hex::decode(hex_str.trim()).unwrap_or_else(|e| panic!("invalid hex in {}: {e}", args.file));

    let valid_names: Vec<&str> = ConsensusFlags::FLAGS
        .iter()
        .map(bitflags::Flag::name)
        .collect();

    let mut flags = ConsensusFlags::empty();
    for name in &args.flags {
        if name == "help" {
            eprintln!("available flags: {}", valid_names.join(", "));
            std::process::exit(0);
        }
        let Some(flag) = ConsensusFlags::from_name(name) else {
            eprintln!("unknown flag: {name}");
            eprintln!("available flags: {}", valid_names.join(", "));
            std::process::exit(1);
        };
        flags |= flag;
    }

    let block_refs: &[&[u8]] = &[];

    let start = Instant::now();
    let result = run_block_generator2(
        &program,
        block_refs,
        args.max_cost,
        flags,
        &Signature::default(),
        None,
        &TEST_CONSTANTS,
    );
    let elapsed = start.elapsed();

    match result {
        Ok((allocator, conditions)) => {
            let byte_cost = conditions
                .cost
                .saturating_sub(conditions.execution_cost)
                .saturating_sub(conditions.condition_cost);
            println!("spends: {}", conditions.spends.len());
            println!("cost: {}", conditions.cost);
            println!("  execution_cost: {}", conditions.execution_cost);
            println!("  condition_cost: {}", conditions.condition_cost);
            println!("  byte_cost: {byte_cost}");
            println!("allocated_atoms: {}", allocator.allocated_atom_count());
            println!("allocated_pairs: {}", allocator.allocated_pair_count());
            println!("allocated_heap: {}", allocator.allocated_heap_size());
            println!("time: {elapsed:.3?}");
            let nanos = elapsed.as_nanos() as f64;
            let clvm_cost = conditions.execution_cost + conditions.condition_cost;
            if clvm_cost > 0 {
                println!("ns/cost: {:.4}", nanos / clvm_cost as f64);
            }
        }
        Err(err) => {
            eprintln!("error: {err:?}");
            eprintln!("time: {elapsed:.3?}");
            std::process::exit(1);
        }
    }
}
