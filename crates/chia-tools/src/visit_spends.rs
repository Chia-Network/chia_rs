use chia_consensus::error_code::{ErrorCode, first};
use chia_protocol::Bytes32;
use chia_protocol::FullBlock;
use chia_puzzles::CHIALISP_DESERIALISATION;
use chia_traits::streamable::Streamable;
use clvm_traits::{FromClvm, destructure_list, match_list};
use clvmr::Allocator;
use clvmr::allocator::NodePtr;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::{node_from_bytes, node_from_bytes_backrefs};
use rusqlite::Connection;

pub fn iterate_blocks(
    db: &str,
    start_height: u32,
    max_height: Option<u32>,
    mut callback: impl FnMut(u32, FullBlock, Vec<Vec<u8>>),
) {
    let connection = Connection::open(db).expect("failed to open database file");

    let mut statement = connection
        .prepare(
            "SELECT height, block \
        FROM full_blocks \
        WHERE in_main_chain=1 AND height >= ?\
        ORDER BY height",
        )
        .expect("failed to prepare SQL statement enumerating blocks");

    let mut block_ref_lookup = connection
        .prepare("SELECT block FROM full_blocks WHERE height=? and in_main_chain=1")
        .expect("failed to prepare SQL statement looking up ref-blocks");

    let mut rows = statement
        .query([start_height])
        .expect("failed to query blocks");
    while let Ok(Some(row)) = rows.next() {
        let height = row.get::<_, u32>(0).expect("missing height");
        if let Some(h) = max_height {
            if height > h {
                break;
            }
        }

        let block_buffer: Vec<u8> = row.get(1).expect("invalid block blob");

        let block_buffer =
            zstd::stream::decode_all(&mut std::io::Cursor::<Vec<u8>>::new(block_buffer))
                .expect("failed to decompress block");
        let block =
            FullBlock::from_bytes_unchecked(&block_buffer).expect("failed to parse FullBlock");

        if block.transactions_generator.is_none() {
            callback(height, block, vec![]);
            continue;
        }

        let mut block_refs = Vec::<Vec<u8>>::new();
        for height in &block.transactions_generator_ref_list {
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
            block_refs.push(ref_gen.as_ref().into());
        }

        callback(height, block, block_refs);
    }
}

pub fn visit_spends<
    GenBuf: AsRef<[u8]>,
    F: FnMut(&mut Allocator, Bytes32, u64, NodePtr, NodePtr),
>(
    a: &mut Allocator,
    program: &[u8],
    block_refs: &[GenBuf],
    max_cost: u64,
    mut callback: F,
) -> Result<(), ErrorCode> {
    let clvm_deserializer = node_from_bytes(a, &CHIALISP_DESERIALISATION)?;
    let program = node_from_bytes_backrefs(a, program)?;

    // iterate in reverse order since we're building a linked list from
    // the tail
    let mut blocks = a.nil();
    for g in block_refs.iter().rev() {
        let ref_gen = a.new_atom(g.as_ref())?;
        blocks = a.new_pair(ref_gen, blocks)?;
    }

    // the first argument to the generator is the serializer, followed by a list
    // of the blocks it requested.
    let mut args = a.new_pair(blocks, a.nil())?;
    args = a.new_pair(clvm_deserializer, args)?;

    let dialect = ChiaDialect::new(0);

    let Reduction(_, mut all_spends) = run_program(a, &dialect, program, args, max_cost)?;

    all_spends = first(a, all_spends)?;

    // at this point all_spends is a list of:
    // (parent-coin-id puzzle-reveal amount solution . extra)
    // where extra may be nil, or additional extension data

    while let Some((spend, rest)) = a.next(all_spends) {
        all_spends = rest;
        // process the spend
        let destructure_list!(parent_id, puzzle, amount, solution, _spend_level_extra) =
            <match_list!(Bytes32, NodePtr, u64, NodePtr, NodePtr)>::from_clvm(a, spend)
                .map_err(|_| ErrorCode::InvalidCondition(spend))?;
        callback(a, parent_id, amount, puzzle, solution);
    }
    Ok(())
}
