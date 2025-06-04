use chia_protocol::SpendBundle;
use chia_traits::streamable::Streamable;
use clap::Parser;
use clvmr::allocator::{Allocator, NodeVisitor};
use clvmr::serde::node_from_bytes_backrefs;
use std::collections::HashMap;
use std::fs::read;

/// collect histogram of atoms of puzzle reveal in spend bundles
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// SpendBundle files to analyze
    pub files: Vec<String>,
}

fn main() {
    let args = Args::parse();

    let mut histogram = HashMap::<u32, i32>::new();
    let mut atom_count = 0;
    let mut hits = 0;

    for filename in args.files {
        let bundle = SpendBundle::from_bytes(&read(filename).expect("read file"))
            .expect("parse SpendBundle");
        for cs in &bundle.coin_spends {
            let mut a = Allocator::new();
            let node = node_from_bytes_backrefs(&mut a, cs.puzzle_reveal.as_slice())
                .expect("parse puzzle");

            let mut vals = vec![node];

            while let Some(v) = vals.pop() {
                match a.node(v) {
                    NodeVisitor::Buffer(_) => {
                        atom_count += 1;
                    }
                    NodeVisitor::U32(val) => {
                        *histogram.entry(val).or_insert(0) += 1;
                        atom_count += 1;
                        if val < 24 {
                            hits += 1;
                        }
                    }
                    NodeVisitor::Pair(left, right) => {
                        vals.push(left);
                        vals.push(right);
                    }
                }
            }
        }
    }

    let mut ordered: Vec<(u32, i32)> = histogram.into_iter().collect();
    ordered.sort_by_key(|e| -i64::from(e.1));
    let cutoff = ordered[0].1 / 1000;
    for (val, count) in ordered {
        if count < cutoff {
            break;
        }
        println!(
            "{val:8}: {count:8} ({:0.2} %)",
            count as f64 * 100.0 / atom_count as f64
        );
    }

    println!(
        "total atoms: {atom_count} hits: {hits} hit-ratio: {:0.2}%",
        (hits as f64) * 100.0 / atom_count as f64
    );
}
