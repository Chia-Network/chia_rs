use chia_tools::iterate_blocks;
use clap::Parser;
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};

/// Analyze the blocks in a chia blockchain database
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to blockchain database file to analyze
    file: String,
}

const MAX_SINGLE_BYTE: u8 = 0x7f;
const BACK_REFERENCE: u8 = 0xfe;
const CONS_BOX_MARKER: u8 = 0xff;

pub fn decode_size<R: Read>(f: &mut R, initial_b: u8) -> u64 {
    assert!((initial_b & 0x80) != 0);

    let atom_start_offset = initial_b.leading_ones() as usize;
    assert!(atom_start_offset < 8);
    let bit_mask: u8 = 0xff >> atom_start_offset;
    let b = initial_b & bit_mask;
    let mut stack_allocation = [0_u8; 8];
    let size_blob = &mut stack_allocation[..atom_start_offset];
    size_blob[0] = b;
    if atom_start_offset > 1 {
        let remaining_buffer = &mut size_blob[1..];
        f.read_exact(remaining_buffer).expect("read");
    }
    // need to convert size_blob to an int
    let mut atom_size: u64 = 0;
    assert!(size_blob.len() <= 6);
    for b in size_blob {
        atom_size <<= 8;
        atom_size += *b as u64;
    }
    assert!(atom_size < 0x4_0000_0000);
    atom_size
}

pub fn record_backreference_length(b: &[u8], histogram: &mut HashMap<u32, u32>) {
    let mut f = Cursor::new(b);
    let mut ops_counter = 1;
    let mut b = [0; 1];
    while ops_counter > 0 {
        ops_counter -= 1;
        f.read_exact(&mut b).expect("read()");
        if b[0] == CONS_BOX_MARKER {
            // we expect to parse two more items from the stream
            // the left and right sub tree
            ops_counter += 2;
        } else if b[0] == BACK_REFERENCE {
            // This is a back-ref. We don't actually need to resolve it, just
            // parse the path and move on
            let mut first_byte = [0; 1];
            f.read_exact(&mut first_byte).expect("read");
            if first_byte[0] > MAX_SINGLE_BYTE {
                let path_size = decode_size(&mut f, first_byte[0]);
                f.seek(SeekFrom::Current(path_size as i64)).expect("seek");
                assert!(f.get_ref().len() as u64 >= f.position());
                histogram
                    .entry(path_size as u32 + 1)
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
            } else {
                histogram.entry(1).and_modify(|c| *c += 1).or_insert(1);
            }
        } else if b[0] == 0x80 || b[0] <= MAX_SINGLE_BYTE {
            // This one byte we just read was the whole atom.
            // or the special case of NIL
        } else {
            let blob_size = decode_size(&mut f, b[0]);
            f.seek(SeekFrom::Current(blob_size as i64)).expect("seek");
            assert!(f.get_ref().len() as u64 >= f.position());
        }
    }
}

fn main() {
    let args = Args::parse();

    // count the number of back references by length (in bytes).
    // maps reference length to the number of occurrances
    let mut backrefs_histogram = HashMap::<u32, u32>::new();
    iterate_blocks(
        &args.file,
        // there are no back references before the hard fork
        5_496_000,
        None,
        |_height, block, _block_refs| {
            if block.transactions_generator.is_none() {
                return;
            }
            let generator = block
                .transactions_generator
                .as_ref()
                .expect("transactions_generator");

            record_backreference_length(generator, &mut backrefs_histogram);
        },
    );

    let mut result: Vec<(u32, u32)> = backrefs_histogram.into_iter().collect();
    result.sort_unstable();
    for (length, count) in result {
        println!("{length:3}: {count}");
    }
}
