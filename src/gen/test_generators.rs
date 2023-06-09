use super::conditions::{NewCoin, Spend, SpendBundleConditions};
use super::run_block_generator::{run_block_generator, run_block_generator2};
use crate::allocator::make_allocator;
use crate::gen::flags::{ALLOW_BACKREFS, MEMPOOL_MODE};
use chia_protocol::{Bytes, Bytes48};
use clvmr::Allocator;
use std::iter::zip;
use std::string::String;
use text_diff::diff;
use text_diff::Difference;

use rstest::rstest;

fn print_conditions(a: &Allocator, c: &SpendBundleConditions) -> String {
    let mut ret = String::new();
    if c.reserve_fee > 0 {
        ret += &format!("RESERVE_FEE: {}\n", c.reserve_fee);
    }

    if c.height_absolute > 0 {
        ret += &format!("ASSERT_HEIGHT_ABSOLUTE {}\n", c.height_absolute);
    }
    if c.seconds_absolute > 0 {
        ret += &format!("ASSERT_SECONDS_ABSOLUTE {}\n", c.seconds_absolute);
    }
    if let Some(val) = c.before_seconds_absolute {
        ret += &format!("ASSERT_BEFORE_SECONDS_ABSOLUTE {val}\n");
    }
    if let Some(val) = c.before_height_absolute {
        ret += &format!("ASSERT_BEFORE_HEIGHT_ABSOLUTE {val}\n");
    }
    let mut agg_sigs = Vec::<(Bytes48, Bytes)>::new();
    for (pk, msg) in &c.agg_sig_unsafe {
        agg_sigs.push((a.atom(*pk).into(), a.atom(*msg).into()));
    }
    agg_sigs.sort();
    for (pk, msg) in agg_sigs {
        ret += &format!(
            "AGG_SIG_UNSAFE pk: {} msg: {}\n",
            hex::encode(pk),
            hex::encode(msg)
        );
    }
    ret += "SPENDS:\n";

    let mut spends: Vec<Spend> = c.spends.clone();
    spends.sort_by_key(|s| *s.coin_id);
    for s in spends {
        ret += &format!(
            "- coin id: {} ph: {}\n",
            hex::encode(*s.coin_id),
            hex::encode(a.atom(s.puzzle_hash))
        );

        if let Some(val) = s.height_relative {
            ret += &format!("  ASSERT_HEIGHT_RELATIVE {val}\n");
        }
        if let Some(val) = s.seconds_relative {
            ret += &format!("  ASSERT_SECONDS_RELATIVE {val}\n");
        }
        if let Some(val) = s.before_height_relative {
            ret += &format!("  ASSERT_BEFORE_HEIGHT_RELATIVE {val}\n");
        }
        if let Some(val) = s.before_seconds_relative {
            ret += &format!("  ASSERT_BEFORE_SECONDS_RELATIVE {val}\n");
        }
        let mut create_coin: Vec<&NewCoin> = s.create_coin.iter().collect();
        create_coin.sort_by_key(|cc| (cc.puzzle_hash, cc.amount));
        for cc in create_coin {
            if cc.hint != -1 {
                ret += &format!(
                    "  CREATE_COIN: ph: {} amount: {} hint: {}\n",
                    hex::encode(cc.puzzle_hash),
                    cc.amount,
                    hex::encode(a.atom(cc.hint))
                );
            } else {
                ret += &format!(
                    "  CREATE_COIN: ph: {} amount: {}\n",
                    hex::encode(cc.puzzle_hash),
                    cc.amount
                );
            }
        }

        let mut agg_sigs = Vec::<(Bytes48, Bytes)>::new();
        for (pk, msg) in s.agg_sig_me {
            agg_sigs.push((a.atom(pk).into(), a.atom(msg).into()));
        }
        agg_sigs.sort();
        for (pk, msg) in agg_sigs {
            ret += &format!(
                "  AGG_SIG_ME pk: {} msg: {}\n",
                hex::encode(pk),
                hex::encode(msg)
            );
        }
    }

    ret += &format!("cost: {}\n", c.cost);
    ret += &format!("removal_amount: {}\n", c.removal_amount);
    ret += &format!("addition_amount: {}\n", c.addition_amount);
    ret
}

fn print_diff(output: &str, expected: &str) {
    println!("\x1b[102m \x1b[0m - output from test");
    println!("\x1b[101m \x1b[0m - expected output");
    for diff in diff(expected, &output, "\n").1 {
        match diff {
            Difference::Same(s) => {
                let lines: Vec<&str> = s.split("\n").collect();
                if lines.len() <= 6 {
                    for l in &lines {
                        println!(" {l}");
                    }
                } else {
                    for l in &lines[0..3] {
                        println!(" {l}");
                    }
                    println!(" ...");
                    for l in &lines[lines.len() - 3..] {
                        println!(" {l}");
                    }
                }
            }
            Difference::Rem(s) => {
                println!("\x1b[91m");
                for l in s.split("\n") {
                    println!("-{l}");
                }
                println!("\x1b[0m");
            }
            Difference::Add(s) => {
                println!("\x1b[92m");
                for l in s.split("\n") {
                    println!("+{l}");
                }
                println!("\x1b[0m");
            }
        }
    }
}

#[cfg(test)]
#[rstest]
#[case("block-225758")]
#[case("assert-puzzle-announce-fail")]
#[case("block-834752")]
#[case("block-834752-compressed")]
#[case("block-834760")]
#[case("block-834761")]
#[case("block-834765")]
#[case("block-834766")]
#[case("block-834768")]
#[case("create-coin-different-amounts")]
#[case("create-coin-hint-duplicate-outputs")]
#[case("create-coin-hint")]
#[case("create-coin-hint2")]
#[case("deep-recursion-plus")]
#[case("double-spend")]
#[case("duplicate-coin-announce")]
#[case("duplicate-create-coin")]
#[case("duplicate-height-absolute-div")]
#[case("duplicate-height-absolute-substr-tail")]
#[case("duplicate-height-absolute-substr")]
#[case("duplicate-height-absolute")]
#[case("duplicate-height-relative")]
#[case("duplicate-outputs")]
#[case("duplicate-reserve-fee")]
#[case("duplicate-seconds-absolute")]
#[case("duplicate-seconds-relative")]
#[case("height-absolute-ladder")]
#[case("infinite-recursion1")]
#[case("infinite-recursion2")]
#[case("infinite-recursion3")]
#[case("infinite-recursion4")]
#[case("invalid-conditions")]
#[case("just-puzzle-announce")]
#[case("many-create-coin")]
#[case("many-large-ints-negative")]
#[case("many-large-ints")]
#[case("max-height")]
#[case("multiple-reserve-fee")]
#[case("negative-reserve-fee")]
#[case("recursion-pairs")]
#[case("unknown-condition")]
fn run_generator(#[case] name: &str) {
    use std::fs::read_to_string;

    let filename = format!("generator-tests/{name}.txt");
    println!("file: {filename}");
    let test_file = read_to_string(filename).expect("test file not found");
    let (generator, expected) = test_file.split_once("\n").expect("invalid test file");
    let generator = hex::decode(generator).expect("invalid hex encoded generator");

    let expected = match expected.split_once("STRICT:\n") {
        Some((c, m)) => [c, m],
        None => [expected, expected],
    };

    let mut block_refs = Vec::<Vec<u8>>::new();

    let filename = format!("generator-tests/{name}.env");
    if let Ok(env_hex) = std::fs::read_to_string(&filename) {
        println!("block-ref file: {filename}");
        block_refs.push(hex::decode(env_hex).expect("hex decode env-file"));
    }

    for (flags, expected) in zip(&[ALLOW_BACKREFS, ALLOW_BACKREFS | MEMPOOL_MODE], expected) {
        println!("flags: {:x}", flags);
        let mut a = make_allocator(*flags);
        let (expected_cost, output) =
            match run_block_generator(&mut a, &generator, &block_refs, 11000000000, *flags) {
                Ok(conditions) => (conditions.cost, print_conditions(&a, &conditions)),
                Err(code) => (0, format!("FAILED: {}\n", u32::from(code.1))),
            };

        let output_hard_fork =
            match run_block_generator2(&mut a, &generator, &block_refs, 11000000000, *flags) {
                Ok(mut conditions) => {
                    // in the hard fork, the cost of running the genrator +
                    // puzzles should never be higher than before the hard-fork
                    // but it's likely less.
                    assert!(conditions.cost <= expected_cost);
                    assert!(conditions.cost > 0);
                    // update the cost we print here, just to be compatible with
                    // the test cases we have. We've already ensured the cost is
                    // lower
                    conditions.cost = expected_cost;
                    print_conditions(&a, &conditions)
                }
                Err(code) => {
                    format!("FAILED: {}\n", u32::from(code.1))
                }
            };

        if output != output_hard_fork {
            print_diff(&output, &output_hard_fork);
            panic!("run_block_generator2 produced a different result!");
        }

        if output != expected {
            print_diff(&output, &expected);
            panic!("mismatching generator output");
        }
    }
}
