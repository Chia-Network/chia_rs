use clap::Parser;

use chia_consensus::consensus_constants::ConsensusConstants;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::DONT_VALIDATE_SIGNATURE;
use chia_consensus::run_block_generator::{run_block_generator, run_block_generator2};
use chia_protocol::{Bytes32, Coin};
use chia_tools::iterate_blocks;
use clvmr::Allocator;
use rusqlite::Connection;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Write;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::available_parallelism;
use std::time::{Duration, Instant};

use hex_literal::hex;

/// Validates a blockchain database (must use v2 schema)
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
struct Args {
    /// Path to blockchain database file to validate
    file: String,

    /// The number of paralell thread to run block generators in
    #[arg(short = 'j', long)]
    num_jobs: Option<usize>,

    /// Start at this block height. Assume all blocks up to this height are
    /// valid. This is meant for resuming validation or re-producing a failure
    #[arg(short, long, default_value_t = 0)]
    start: u32,

    /// Validate blockchain against a height-to-hash file
    #[arg(long)]
    height_to_hash: Option<String>,

    /// Don't validate block signatures (saves time)
    #[arg(long, default_value_t = false)]
    skip_signature_validation: bool,

    /// use testnet 11 constants instead of mainnet. This is required when
    /// validating testnet blockchain database.s
    #[arg(long, default_value_t = false)]
    testnet: bool,
}

const MAINNET_CONSTANTS: ConsensusConstants = TEST_CONSTANTS;
const TESTNET11_CONSTANTS: ConsensusConstants = ConsensusConstants {
    agg_sig_me_additional_data: Bytes32::new(hex!(
        "37a90eb5185a9c4439a91ddc98bbadce7b4feba060d50116a067de66bf236615"
    )),
    agg_sig_parent_additional_data: Bytes32::new(hex!(
        "c0754ae8602c47489b5394af8972c58238c4389d715f0585ca512d9428395e62"
    )),
    agg_sig_puzzle_additional_data: Bytes32::new(hex!(
        "2e63e4ca0796d9ef8e8a748d740f4b8632c4d994ad6cce51bd61a6612d602697"
    )),
    agg_sig_amount_additional_data: Bytes32::new(hex!(
        "cf15f86103bee6260b0e020a1ba02bcf61230fe209592543399dcf9267f8dfcc"
    )),
    agg_sig_puzzle_amount_additional_data: Bytes32::new(hex!(
        "02c0ecb453e75bd77823dd0affd3f224d968012a8c6c6c423801cc30dd5eb347"
    )),
    agg_sig_parent_amount_additional_data: Bytes32::new(hex!(
        "fc5eaa82087943fbee8683d42ae7a2a7aac0d4eecd4c98d71c228b9c62bf9497"
    )),
    agg_sig_parent_puzzle_additional_data: Bytes32::new(hex!(
        "54c3ed8017f77354acca4000b40424396a369740e5a504467784f392b961ab37"
    )),
    difficulty_constant_factor: 10_052_721_566_054,
    difficulty_starting: 30,
    epoch_blocks: 768,
    genesis_challenge: Bytes32::new(hex!(
        "37a90eb5185a9c4439a91ddc98bbadce7b4feba060d50116a067de66bf236615"
    )),
    genesis_pre_farm_farmer_puzzle_hash: Bytes32::new(hex!(
        "08296fc227decd043aee855741444538e4cc9a31772c4d1a9e6242d1e777e42a"
    )),
    genesis_pre_farm_pool_puzzle_hash: Bytes32::new(hex!(
        "3ef7c233fc0785f3c0cae5992c1d35e7c955ca37a423571c1607ba392a9d12f7"
    )),
    mempool_block_buffer: 10,
    min_plot_size_v1: 18,
    sub_slot_iters_starting: 67_108_864,
    // forks activated from the beginning on testnet11
    hard_fork_height: 0,
    plot_filter_128_height: 6_029_568,
    plot_filter_64_height: 11_075_328,
    plot_filter_32_height: 16_121_088,
    ..MAINNET_CONSTANTS
};

fn main() {
    let args = Args::parse();

    let constants = if args.testnet {
        &TESTNET11_CONSTANTS
    } else {
        &MAINNET_CONSTANTS
    };

    let num_cores = args
        .num_jobs
        .unwrap_or_else(|| available_parallelism().unwrap().into());

    let pool = blocking_threadpool::Builder::new()
        .num_threads(num_cores)
        .queue_len(num_cores + 5)
        .build();

    let error_count = Arc::new(AtomicUsize::new(0));
    let mut last_height = args.start;
    let mut last_time = Instant::now();
    println!(
        r"THIS TOOL DOES NOT VALIDATE ALL ASPECTS OF A BLOCKCHAIN DATABASE
features that are validated:
  * block hashes and heights
  * some conditions
  * block signatures (unless disabled by command line option)
  * the coin_record table
"
    );
    println!("opening blockchain database file: {}", args.file);

    let connection = Connection::open(&args.file).expect("failed to open database file");
    let mut select_spends = connection
        .prepare("SELECT coin_name FROM coin_record WHERE spent_index == ?;")
        .expect("failed to prepare SQL statement finding spent coins");
    let mut select_created = connection
        .prepare(
            "SELECT coin_name, coinbase, puzzle_hash, coin_parent, amount FROM coin_record WHERE confirmed_index == ?;",
        )
        .expect("failed to prepare SQL statement finding created coins");
    let mut select_peak = connection
        .prepare("SELECT hash FROM current_peak WHERE key == 0;")
        .expect("failed to prepare SQL statement finding peak");

    let mut peak_row = select_peak.query([]).expect("failed to query current peak");
    let peak_hash = peak_row
        .next()
        .expect("missing peak")
        .expect("missing peak")
        .get::<_, [u8; 32]>(0)
        .expect("missing peak");

    let mut prev_hash = constants.genesis_challenge;
    let mut prev_height: i64 = args.start as i64 - 1;

    let height_to_hash: Option<Vec<Bytes32>> = args.height_to_hash.map(|hth| {
        std::fs::read(hth)
            .expect("failed to read height-to-hash")
            .chunks(32)
            .map(|v| -> Bytes32 { v.try_into().unwrap() })
            .collect()
    });

    println!("iterating over blocks starting at height {}", args.start);
    iterate_blocks(&args.file, args.start, None, |height, block, block_refs| {
        // If we don't start validation from height 0, we need to initialize the
        // expected prev-hash based on the first block we pull from the DB
        if args.start != 0 && prev_hash == constants.genesis_challenge {
            prev_hash = block.prev_header_hash();
        }

        if block.prev_header_hash() != prev_hash {
            println!("at height {height} the previous header hash mismatches. {} expected {} from height {}",
                block.prev_header_hash(),
                prev_hash,
                prev_height,
            );
            error_count.fetch_add(1, Ordering::Relaxed);
        }
        if block.height() != height {
            println!(
                "at height {height} the height recorded in the block mismatches, {}",
                block.height(),
            );
            error_count.fetch_add(1, Ordering::Relaxed);
        }
        if height != (prev_height + 1) as u32 {
            println!("at height {height} the the block height did not increment by 1, from previous block (at height {prev_height})");
            error_count.fetch_add(1, Ordering::Relaxed);
        }
        prev_hash = block.header_hash();
        prev_height = height as i64;
        if let Some(hth) = &height_to_hash {
            if hth.len() > height as usize && hth[height as usize] != prev_hash {
                println!("at height {height} the block hash ({prev_hash}) does not match the height-to-hash file ({})", hth[height as usize]);
                error_count.fetch_add(1, Ordering::Relaxed);
            }
        }
        let mut removals = HashSet::<[u8; 32]>::new();
        // height 0 is not a transaction block so unspent coins have a
        // spent_index of 0 to indicate that they have not been spent.
        if height != 0 {
            let mut removals_rows = select_spends
                .query([height])
                .expect("failed to query spent coins");
            while let Ok(Some(row)) = removals_rows.next() {
                removals.insert(row.get::<_, [u8; 32]>(0).expect("missing coin_name"));
            }
        }
        let mut additions_rows = select_created
            .query([height])
            .expect("failed to query created coins");
        // coin-id -> (puzzle-hash, parent-coin, amount, reward)
        let mut additions = HashMap::<[u8; 32], ([u8; 32], [u8; 32], u64, bool)>::new();
        while let Ok(Some(row)) = additions_rows.next() {
            let coin_name = row.get::<_, [u8; 32]>(0).expect("missing coin_name");
            let reward = row.get::<_, bool>(1).expect("missing coinbase");
            let ph = row.get::<_, [u8; 32]>(2).expect("missing puzzle_hash");
            let parent = row.get::<_, [u8; 32]>(3).expect("missing parent");
            let amount = u64::from_be_bytes(row.get::<_, [u8; 8]>(4).expect("missing amount"));
            additions.insert(coin_name, (ph, parent, amount, reward));
        }

        // first ensure that the reward coins for this block are all included in
        // the coin record table.
        let rewards = block.get_included_reward_coins();
        for add in &rewards {
            let new_coin_id = add.coin_id();
            let Some((ph, _parent, amount, coin_base)) = additions.get(new_coin_id.as_slice())
            else {
                println!("at height {height} the block created a reward coin {new_coin_id} that's not in the coin_record table");
                error_count.fetch_add(1, Ordering::Relaxed);
                continue;
            };
            // TODO: ensure the parent coin ID is set correctly
            if ph != add.puzzle_hash.as_slice() {
                println!("at height {height} the reward coin {new_coin_id} has an incorrect puzzle hash in the coin_record table {} expected {}",
                    hex::encode(ph),
                    add.puzzle_hash
                );
                error_count.fetch_add(1, Ordering::Relaxed);
            }
            // ensure the parent hash has the expected look
            if *amount != add.amount {
                println!("at height {height} reward coin {new_coin_id} has amount {} in coin_record table, but the block has amount {}", amount, add.amount);
                error_count.fetch_add(1, Ordering::Relaxed);
            }
            // this is a reward coin
            if !coin_base {
                println!("at height {height} the reward coin {new_coin_id} is not marked as coin-base in the database");
                error_count.fetch_add(1, Ordering::Relaxed);
            }
            additions.remove(new_coin_id.as_slice());
        }
        if block.transactions_generator.is_none() {
            // this is not a transaction block
            // there should be no coins in the coin table spent at this height.
            if !removals.is_empty() {
                println!("block at height {height} is not a transaction block, but the coin_record table has coins spent at this block height");
                for coin_id in removals {
                    println!("  id: {}", hex::encode(coin_id));
                }
                error_count.fetch_add(1, Ordering::Relaxed);
            }
            // there should not be any non-reward coins created in this block
            if !additions.is_empty() {
                println!("block at height {height} is not a transaction block, but the coin_record table has coins created at this block height");
                for (coin_id, (ph, parent, amount, reward)) in additions {
                    println!(
                        "  id: {} - {} {} {amount} {}",
                        hex::encode(coin_id),
                        hex::encode(ph),
                        hex::encode(parent),
                        if reward { "(coinbase)" } else { "" }
                    );
                }
                error_count.fetch_add(1, Ordering::Relaxed);
            }
            return;
        }
        let cnt = error_count.clone();
        pool.execute(move || {
                let mut a = Allocator::new_limited(500_000_000);

                let ti = block.transactions_info.as_ref().expect("transactions_info");
                let generator = block
                    .transactions_generator
                    .as_ref()
                    .expect("transactions_generator");

                // after the hard fork, we run blocks without paying for the
                // CLVM generator ROM
                let block_runner = if height >= constants.hard_fork_height {
                    run_block_generator2
                } else {
                    run_block_generator
                };
                let flags = if args.skip_signature_validation {
                        DONT_VALIDATE_SIGNATURE
                    } else {
                        0
                    };
                let conditions = block_runner(
                    &mut a,
                    generator,
                    &block_refs,
                    ti.cost,
                    flags,
                    &ti.aggregated_signature,
                    None,
                    constants,
                )
                .expect("failed to run block generator");

                if conditions.cost != ti.cost {
                    println!("at height {height} block header has cost of {}, expected {}", ti.cost, conditions.cost);
                    cnt.fetch_add(1, Ordering::Relaxed);
                }

                for spend in &conditions.spends {
                    let coin_name = spend.coin_id;
                    if !removals.remove(coin_name.as_slice()) {
                        println!("at height {height} could not find coin {coin_name} in coin_record table, which is being spent at height {height}");
                        cnt.fetch_add(1, Ordering::Relaxed);
                    }
                    for add in &spend.create_coin {
                        let new_coin_id = Coin::new(coin_name, add.puzzle_hash, add.amount).coin_id();
                        let Some((ph, parent, amount, coin_base)) = additions.get(new_coin_id.as_slice()) else {
                            println!("at height {height} the block created a coin {new_coin_id} that's not in the coin_record table");
                            cnt.fetch_add(1, Ordering::Relaxed);
                            continue;
                        };
                        if ph != add.puzzle_hash.as_slice() {
                            println!("at height {height} the spent coin with id {new_coin_id} has a mismatching puzzle hash {} expected {}", add.puzzle_hash, Bytes32::from(ph));
                            cnt.fetch_add(1, Ordering::Relaxed);
                        }
                        if parent != coin_name.as_slice() {
                            println!("at height {height} the spent coin with id {new_coin_id} has a mismatching parent {} expected {}", coin_name, Bytes32::from(parent));
                            cnt.fetch_add(1, Ordering::Relaxed);
                        }
                        if *amount != add.amount {
                            println!("at height {height} the spent coin with id {new_coin_id} has a mismatching amount {} expected {}", *amount, add.amount);
                            cnt.fetch_add(1, Ordering::Relaxed);
                        }
                        // this is not a reward coin
                        if *coin_base {
                            println!("at height {height}, the created coin {new_coin_id} is incorrectly marked as coin-base in the database");
                            cnt.fetch_add(1, Ordering::Relaxed);
                        }
                        additions.remove(new_coin_id.as_slice());
                    }
                }
                if !removals.is_empty() {
                    println!("at height {height} the coin_table has {} extra spends", removals.len());
                    for coin_id in removals {
                        println!("  id: {}", hex::encode(coin_id));
                    }
                    cnt.fetch_add(1, Ordering::Relaxed);
                }

                if !additions.is_empty() {
                    println!("at height {height} the coin_table has {} extra coin additions", additions.len());
                    for (coin_id, (ph, parent, amount, reward)) in additions {
                        println!("  id: {} - {} {} {amount} {}",
                            hex::encode(coin_id),
                            hex::encode(ph),
                            hex::encode(parent),
                            if reward { "(coinbase)" } else {""}
                        );
                    }
                    cnt.fetch_add(1, Ordering::Relaxed);
                }
            });

        assert_eq!(pool.panic_count(), 0);
        if last_time.elapsed() > Duration::new(2, 0) {
            let rate = f64::from(height - last_height) / last_time.elapsed().as_secs_f64();
            print!("\rheight: {height} ({rate:0.1} blocks/s)   ");
            let _ = std::io::stdout().flush();
            last_height = height;
            last_time = Instant::now();
        }
    });

    pool.join();
    assert_eq!(pool.panic_count(), 0);

    if peak_hash != prev_hash.as_slice() {
        println!(
            "peak hash (in database) does not match the chain {}, expected {}",
            Bytes32::from(peak_hash),
            prev_hash
        );
        error_count.fetch_add(1, Ordering::Relaxed);
    }

    assert_eq!(
        error_count.load(Ordering::Relaxed),
        0,
        "exiting with failures"
    );

    println!("\nALL DONE, success!");
}
