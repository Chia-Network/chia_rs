use clap::Parser;

use chia_protocol::FullBlock;
use chia_traits::Streamable;
use std::io::Write;
use std::time::SystemTime;

use rusqlite::Connection;

use chia_consensus::gen::conditions::{parse_spends, MempoolVisitor};
use chia_consensus::gen::flags::MEMPOOL_MODE;
use chia_consensus::generator_rom::{COST_PER_BYTE, GENERATOR_ROM};
use clvmr::reduction::Reduction;
use clvmr::run_program_with_counters;
use clvmr::serde::node_from_bytes;
use clvmr::Allocator;
use clvmr::ChiaDialect;

/// Analyze the blocks in a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file to analyze
    #[arg(short, long)]
    file: String,

    /// Specifies whether to run the blocks in the stricter mempool mode or not
    #[arg(short, long, default_value_t = false)]
    mempool_mode: bool,

    /// Start at this block height
    #[arg(short, long, default_value_t = 225694)]
    start: u32,

    /// The height to stop at
    #[arg(short, long, default_value_t = 0xffffffff)]
    end: u32,
}

fn main() {
    let args = Args::parse();

    let connection = Connection::open(args.file).expect("failed to open database file");

    let mut statement = connection
        .prepare(
            "SELECT height, block \
        FROM full_blocks \
        WHERE height >= ? AND height <= ? AND in_main_chain=1 \
        ORDER BY height",
        )
        .expect("failed to prepare SQL statement enumerating blocks");

    let mut block_ref_lookup = connection
        .prepare("SELECT block FROM full_blocks WHERE height=? and in_main_chain=1")
        .expect("failed to prepare SQL statement looking up ref-blocks");

    let mut output =
        std::fs::File::create("chain-resource-usage.log").expect("failed to open output file");

    // We only create a single allocator, load it with the generator ROM and
    // then we keep reusing it
    let mut a = Allocator::new_limited(500000000);
    let generator_rom =
        node_from_bytes(&mut a, &GENERATOR_ROM).expect("failed to parse generator ROM");
    let allocator_checkpoint = a.checkpoint();

    let mut prev_timestamp = 0;

    let mut rows = statement
        .query([args.start, args.end])
        .expect("failed to query blocks");
    while let Ok(Some(row)) = rows.next() {
        let height: u32 = row.get::<_, u32>(0).expect("missing height");
        let block_buffer: Vec<u8> = row.get(1).expect("invalid block blob");

        let start_parse = SystemTime::now();
        let block_buffer =
            zstd::stream::decode_all(&mut std::io::Cursor::<Vec<u8>>::new(block_buffer))
                .expect("failed to decompress block");
        let block =
            FullBlock::from_bytes_unchecked(&block_buffer).expect("failed to parse FullBlock");

        let Some(ti) = block.transactions_info else {
            continue;
        };
        let Some(ftb) = block.foliage_transaction_block else {
            continue;
        };

        if prev_timestamp == 0 {
            prev_timestamp = ftb.timestamp;
        }
        let time_delta = ftb.timestamp - prev_timestamp;
        prev_timestamp = ftb.timestamp;

        let Some(program) = block.transactions_generator else {
            continue;
        };

        a.restore_checkpoint(&allocator_checkpoint);

        let generator =
            node_from_bytes(&mut a, program.as_ref()).expect("failed to parse block generator");

        let parse_timing = start_parse.elapsed().expect("failed to get system time");

        let mut args = a.nil();

        let start_ref_lookup = SystemTime::now();
        // iterate in reverse order since we're building a linked list from
        // the tail
        for height in block.transactions_generator_ref_list.iter().rev() {
            let mut rows = block_ref_lookup
                .query(rusqlite::params![height])
                .expect("failed to look up ref-block");

            let row = rows
                .next()
                .expect("failed to fetch block-ref row")
                .expect("get None block-ref row");
            let ref_block = row
                .get::<_, Vec<u8>>(0)
                .expect("failed to lookup block reference");

            let ref_block =
                zstd::stream::decode_all(&mut std::io::Cursor::<Vec<u8>>::new(ref_block))
                    .expect("failed to decompress block");

            let ref_block =
                FullBlock::from_bytes_unchecked(&ref_block).expect("failed to parse ref-block");
            let ref_gen = ref_block
                .transactions_generator
                .expect("block ref has no generator");

            let ref_gen = a
                .new_atom(ref_gen.as_ref())
                .expect("failed to allocate atom for ref_block");
            args = a.new_pair(ref_gen, args).expect("failed to allocate pair");
        }
        let ref_lookup_timing = start_ref_lookup
            .elapsed()
            .expect("failed to get system time");

        let byte_cost = program.len() as u64 * COST_PER_BYTE;

        args = a.new_pair(args, a.nil()).expect("failed to allocate pair");
        let args = a.new_pair(args, a.nil()).expect("failed to allocate pair");
        let args = a
            .new_pair(generator, args)
            .expect("failed to allocate pair");

        let start_execute = SystemTime::now();
        let dialect = ChiaDialect::new(0);
        let (counters, result) =
            run_program_with_counters(&mut a, &dialect, generator_rom, args, ti.cost - byte_cost);
        let execute_timing = start_execute.elapsed().expect("failed to get system time");

        let Reduction(clvm_cost, generator_output) = result.expect("block generator failed");

        let start_conditions = SystemTime::now();
        // we pass in what's left of max_cost here, to fail early in case the
        // cost of a condition brings us over the cost limit
        let Ok(conds) =
            parse_spends::<MempoolVisitor>(&a, generator_output, ti.cost - clvm_cost, MEMPOOL_MODE)
        else {
            panic!("failed to parse conditions in block {height}");
        };
        let conditions_timing = start_conditions
            .elapsed()
            .expect("failed to get system time");

        assert!(clvm_cost + byte_cost + conds.cost == ti.cost);
        output
            .write_fmt(format_args!(
                "{} val_stack: {} \
            env_stack: {} \
            op_stack: {} \
            atoms: {} \
            pairs: {} \
            heap: {} \
            block_cost: {} \
            clvm_cost: {} \
            cond_cost: {} \
            parse_time: {} \
            ref_lookup_time: {} \
            execute_time: {} \
            conditions_time: {} \
            timestamp: {} \
            time_delta: {} \
            \n",
                height,
                counters.val_stack_usage,
                counters.env_stack_usage,
                counters.op_stack_usage,
                counters.atom_count,
                counters.pair_count,
                counters.heap_size,
                ti.cost,
                clvm_cost,
                conds.cost,
                parse_timing.as_micros(),
                ref_lookup_timing.as_micros(),
                execute_timing.as_micros(),
                conditions_timing.as_micros(),
                ftb.timestamp,
                time_delta,
            ))
            .expect("failed to write to output file");
    }
}
