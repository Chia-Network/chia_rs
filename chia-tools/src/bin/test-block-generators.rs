use clap::Parser;

use chia_protocol::FullBlock;
use chia_protocol::Streamable;

use sqlite::State;

use chia::gen::flags::MEMPOOL_MODE;
use chia::gen::run_block_generator::{run_block_generator, run_block_generator2};
use clvmr::Allocator;
use std::thread::available_parallelism;
use threadpool::ThreadPool;

/// Analyze the blocks in a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file to analyze
    #[arg(short, long)]
    file: String,

    /// The number of paralell thread to run block generators in
    #[arg(short, long)]
    num_jobs: Option<usize>,

    /// Run all block generators in mempool mode
    #[arg(short, long, default_value_t = false)]
    mempool: bool,

    /// Compare the output from the default ROM running in consensus mode.
    #[arg(short, long, default_value_t = false)]
    validate: bool,

    /// stop running block generators when reaching this height
    #[arg(short, long)]
    max_height: Option<u32>,

    /// when enabled, run the rust port of the ROM generator
    #[arg(short, long, default_value_t = false)]
    rust_generator: bool,
}

fn main() {
    let args = Args::parse();

    let connection = sqlite::open(args.file).expect("failed to open database file");

    let mut statement = connection
        .prepare(
            "SELECT height, block \
        FROM full_blocks \
        WHERE in_main_chain=1 \
        ORDER BY height",
        )
        .expect("failed to prepare SQL statement enumerating blocks");

    let mut block_ref_lookup = connection
        .prepare("SELECT block FROM full_blocks WHERE height=? and in_main_chain=1")
        .expect("failed to prepare SQL statement looking up ref-blocks");

    let pool = ThreadPool::new(
        args.num_jobs
            .unwrap_or(available_parallelism().unwrap().into()),
    );

    if args.validate && !(args.mempool || args.rust_generator) {
        panic!("it doesn't make sense to validate the output against identical runs. Specify --mempool or --rust-generator");
    }

    let flags = if args.mempool { MEMPOOL_MODE } else { 0 };

    let block_runner = if args.rust_generator {
        run_block_generator2
    } else {
        run_block_generator
    };

    while let Ok(State::Row) = statement.next() {
        let height: u32 = statement
            .read::<i64, _>(0)
            .expect("missing height")
            .try_into()
            .expect("invalid height in block record");
        if let Some(h) = args.max_height {
            if height > h {
                break;
            }
        }

        let block_buffer = statement.read::<Vec<u8>, _>(1).expect("invalid block blob");

        let block_buffer =
            zstd::stream::decode_all(&mut std::io::Cursor::<Vec<u8>>::new(block_buffer))
                .expect("failed to decompress block");
        let block = FullBlock::parse(&mut std::io::Cursor::<&[u8]>::new(&block_buffer))
            .expect("failed to parse FullBlock");

        let ti = match block.transactions_info {
            Some(ti) => ti,
            None => {
                continue;
            }
        };

        let prg = match block.transactions_generator {
            Some(prg) => prg,
            None => {
                continue;
            }
        };

        // iterate in reverse order since we're building a linked list from
        // the tail
        let mut block_refs = Vec::<Vec<u8>>::new();
        for height in block.transactions_generator_ref_list {
            block_ref_lookup
                .reset()
                .expect("sqlite reset statement failed");
            block_ref_lookup
                .bind((1, height as i64))
                .expect("failed to look up ref-block");

            block_ref_lookup
                .next()
                .expect("failed to fetch block-ref row");
            let ref_block = block_ref_lookup
                .read::<Vec<u8>, _>(0)
                .expect("failed to lookup block reference");

            let ref_block =
                zstd::stream::decode_all(&mut std::io::Cursor::<Vec<u8>>::new(ref_block))
                    .expect("failed to decompress block");

            let ref_block = FullBlock::parse(&mut std::io::Cursor::<&[u8]>::new(&ref_block))
                .expect("failed to parse ref-block");
            let ref_gen = ref_block
                .transactions_generator
                .expect("block ref has no generator");
            block_refs.push(ref_gen.as_ref().into());
        }

        pool.execute(move || {
            let mut a = Allocator::new_limited(500000000, 62500000, 62500000);

            let mut conditions = block_runner(&mut a, prg.as_ref(), &block_refs, ti.cost, flags)
                .expect("failed to run block generator2");
            if args.rust_generator {
                assert!(conditions.cost <= ti.cost);
                assert!(conditions.cost > 0);

                // in order for the comparison below the hold, we need to
                // patch up the cost of the rust generator to look like the
                // baseline
                conditions.cost = ti.cost;
            } else {
                assert_eq!(conditions.cost, ti.cost);
            }

            if args.validate {
                let mut baseline =
                    run_block_generator(&mut a, prg.as_ref(), &block_refs, ti.cost, 0)
                        .expect("failed to run block generator");
                assert_eq!(baseline.cost, ti.cost);

                baseline.spends.sort();
                conditions.spends.sort();

                // now ensure the outputs are the same
                assert_eq!(&baseline, &conditions);
            }
        });

        assert_eq!(pool.panic_count(), 0);
    }

    pool.join();
}
