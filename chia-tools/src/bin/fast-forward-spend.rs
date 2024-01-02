use clap::Parser;
use std::fs;
use std::io::Cursor;

use chia::fast_forward::fast_forward_singleton;
use chia_protocol::bytes::Bytes32;
use chia_protocol::{coin::Coin, coin_spend::CoinSpend, program::Program};
use chia_traits::streamable::Streamable;
use clvm_traits::{FromClvm, ToClvm};
use clvm_utils::tree_hash;
use clvmr::allocator::Allocator;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to CoinSpend file
    #[arg(short, long)]
    spend: String,

    /// fast-forward the CoinSpend to spend a coin whose parent's parent is this
    /// coin ID.
    #[arg(short, long)]
    new_parents_parent: String,

    /// Save resulting CoinSpend to this file
    #[arg(short, long)]
    output_file: String,
}

fn main() {
    let args = Args::parse();

    let spend_bytes = fs::read(args.spend).expect("read file");
    let spend = CoinSpend::parse(&mut Cursor::new(&spend_bytes)).expect("parse CoinSpend");

    let new_parents_parent: Bytes32 = hex::decode(args.new_parents_parent)
        .expect("invalid hex")
        .as_slice()
        .into();

    let mut a = Allocator::new_limited(500000000, 62500000, 62500000);
    let puzzle = spend.puzzle_reveal.to_clvm(&mut a).expect("to_clvm");
    let solution = spend.solution.to_clvm(&mut a).expect("to_clvm");
    let puzzle_hash = Bytes32::from(tree_hash(&a, puzzle));

    let new_parent_coin = Coin {
        parent_coin_info: new_parents_parent,
        puzzle_hash,
        amount: spend.coin.amount,
    };

    let new_coin = Coin {
        parent_coin_info: new_parent_coin.coin_id().into(),
        puzzle_hash,
        amount: spend.coin.amount,
    };

    let new_solution = fast_forward_singleton(
        &mut a,
        puzzle,
        solution,
        &spend.coin,
        &new_coin,
        &new_parent_coin,
    )
    .expect("fast-forward");

    let new_spend = CoinSpend {
        coin: new_parent_coin,
        puzzle_reveal: spend.puzzle_reveal,
        solution: Program::from_clvm(&a, new_solution).expect("new solution"),
    };
    let mut bytes = Vec::<u8>::new();
    new_spend.stream(&mut bytes).expect("stream CoinSpend");
    fs::write(args.output_file, bytes).expect("write");
}
