use clvm_utils::{tree_hash_cached, TreeCache};
use clvmr::allocator::Allocator;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Write;
use std::time::Instant;

fn main() {
    // Open (or create) a log file and truncate previous contents
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("tree_hash_analysis.log")
        .expect("Failed to open log file");

    let mut log = BufWriter::new(file);
    writeln!(log, "Starting analysis").unwrap();

    let mut a = Allocator::new();
    let mut atom_vec = Vec::<u8>::new();
    let mut pairs = a.nil();

    let mut total_time_per_pair = 0.0f64;
    let mut total_time_per_byte_cached = 0.0f64;

    let cycles = 50_000;

    for i in 1..cycles {
        // assuming no cache
        let mut cache = TreeCache::default();

        pairs = a.new_pair(a.nil(), pairs).expect("list making");
        atom_vec.push((i % u8::MAX as u32) as u8);
        let atom = a.new_atom(atom_vec.as_slice()).expect("should be ok");
        let byte_len = atom_vec.len() as f64;

        let mut cost_left = u64::MAX;
        let start_cached = Instant::now();
        tree_hash_cached(&a, atom, &mut cache, &mut cost_left);
        let elapsed_cached = start_cached.elapsed().as_secs_f64();
        total_time_per_byte_cached += elapsed_cached / byte_len;

        let mut cost_left = u64::MAX;
        let start_cached = Instant::now();
        tree_hash_cached(&a, pairs, &mut cache, &mut cost_left);
        let elapsed_cached = start_cached.elapsed().as_secs_f64();
        total_time_per_pair += elapsed_cached / i as f64;

        if i % 1000 == 0 {
            writeln!(
                log,
                "After {} items: avg time/pair = {:.12} sec , avg time/byte {:.12} sec",
                i,
                total_time_per_pair / i as f64,
                total_time_per_byte_cached / i as f64
            )
            .unwrap();
            log.flush().unwrap(); // make sure it's written during execution
        }
    }

    writeln!(
        log,
        "Final averages: {:.12} sec/pair, {:.12} sec/byte",
        total_time_per_pair / cycles as f64,
        total_time_per_byte_cached / cycles as f64,
    )
    .unwrap();
    log.flush().unwrap();
}
