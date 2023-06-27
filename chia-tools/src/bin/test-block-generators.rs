use clap::Parser;

use chia_protocol::FullBlock;
use chia_protocol::Streamable;

use sqlite::State;

use chia::gen::flags::MEMPOOL_MODE;
use chia::gen::run_block_generator::run_block_generator;
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

    while let Ok(State::Row) = statement.next() {
        let height: u32 = statement
            .read::<i64, _>(0)
            .expect("missing height")
            .try_into()
            .expect("invalid height in block record");
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

            let consensus = run_block_generator(&mut a, prg.as_ref(), &block_refs, ti.cost, 0)
                .expect("failed to run block generator");

            let mempool =
                run_block_generator(&mut a, prg.as_ref(), &block_refs, ti.cost, MEMPOOL_MODE)
                    .expect("failed to run block generator");

            println!("height: {height}");
            assert_eq!(consensus.cost, ti.cost);
            assert_eq!(mempool.cost, ti.cost);

            // now ensure the outputs are the same
            assert_eq!(&consensus, &mempool);
        });

        assert_eq!(pool.panic_count(), 0);
    }

    pool.join();
}
