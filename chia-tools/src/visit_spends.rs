use chia::gen::run_block_generator::extract_n;
use chia::gen::validation_error::{first, ErrorCode, ValidationErr};
use chia::generator_rom::CLVM_DESERIALIZER;
use chia_protocol::bytes::Bytes32;
use chia_protocol::FullBlock;
use chia_traits::streamable::Streamable;
use clvmr::allocator::NodePtr;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::{node_from_bytes, node_from_bytes_backrefs};
use clvmr::Allocator;
use sqlite::State;

pub fn iterate_tx_blocks(
    db: &str,
    start_height: u32,
    max_height: Option<u32>,
    callback: impl Fn(u32, FullBlock, Vec<Vec<u8>>),
) {
    let connection = sqlite::open(db).expect("failed to open database file");

    let mut statement = connection
        .prepare(
            "SELECT height, block \
        FROM full_blocks \
        WHERE in_main_chain=1 AND height >= ?\
        ORDER BY height",
        )
        .expect("failed to prepare SQL statement enumerating blocks");
    statement
        .bind((1, start_height as i64))
        .expect("failed to bind start-height to SQL statement");

    let mut block_ref_lookup = connection
        .prepare("SELECT block FROM full_blocks WHERE height=? and in_main_chain=1")
        .expect("failed to prepare SQL statement looking up ref-blocks");

    while let Ok(State::Row) = statement.next() {
        let height: u32 = statement
            .read::<i64, _>(0)
            .expect("missing height")
            .try_into()
            .expect("invalid height in block record");
        if let Some(h) = max_height {
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

        if block.transactions_info.is_none() {
            continue;
        }
        if block.transactions_generator.is_none() {
            continue;
        }

        let mut block_refs = Vec::<Vec<u8>>::new();
        for height in &block.transactions_generator_ref_list {
            block_ref_lookup
                .reset()
                .expect("sqlite reset statement failed");
            block_ref_lookup
                .bind((1, *height as i64))
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

        callback(height, block, block_refs);
    }
}

pub fn visit_spends<GenBuf: AsRef<[u8]>, F: Fn(&mut Allocator, Bytes32, u64, NodePtr, NodePtr)>(
    a: &mut Allocator,
    program: &[u8],
    block_refs: &[GenBuf],
    max_cost: u64,
    callback: F,
) -> Result<(), ValidationErr> {
    let clvm_deserializer = node_from_bytes(a, &CLVM_DESERIALIZER)?;
    let program = node_from_bytes_backrefs(a, program)?;

    // iterate in reverse order since we're building a linked list from
    // the tail
    let mut blocks = a.null();
    for g in block_refs.iter().rev() {
        let ref_gen = a.new_atom(g.as_ref())?;
        blocks = a.new_pair(ref_gen, blocks)?;
    }

    // the first argument to the generator is the serializer, followed by a list
    // of the blocks it requested.
    let mut args = a.new_pair(blocks, a.null())?;
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
        let [parent_id, puzzle, amount, solution, _spend_level_extra] =
            extract_n::<5>(a, spend, ErrorCode::InvalidCondition)?;
        let amount: u64 = a.number(amount).try_into().expect("invalid amount");
        let parent_id = Bytes32::from(a.atom(parent_id));
        callback(a, parent_id, amount, puzzle, solution);
    }
    Ok(())
}
