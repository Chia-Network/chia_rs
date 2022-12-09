use clvmr::allocator::{Allocator, NodePtr};
use clvmr::chia_dialect::ChiaDialect;
use clvmr::node::Node;
use clvmr::run_program::run_program;
use clvmr::serde::{node_from_bytes, node_to_bytes, node_to_bytes_backrefs};

fn wrap_atom_with_decompression_program(
    allocator: &mut Allocator,
    node_ptr: NodePtr,
) -> Result<NodePtr, std::io::Error> {
    let apply_node = allocator.new_atom(&[2])?;
    let quote_node = allocator.one();
    let serialized_backrefs_program = include_bytes!("deserialize_w_backrefs.bin");
    // "(a (q . deserialize_w_backrefs_program) (q . serialized_with_backrefs))"
    let program = node_from_bytes(allocator, serialized_backrefs_program)
        .expect("can't deserialize backref prog");

    let compressed_block = allocator.new_pair(quote_node, node_ptr)?;
    let program = allocator.new_pair(quote_node, program)?;
    let list = allocator.null();
    let list = allocator.new_pair(compressed_block, list)?;
    let list = allocator.new_pair(program, list)?;
    let list = allocator.new_pair(apply_node, list)?;
    Ok(list)
}

pub fn decompress(allocator: &mut Allocator, blob: &[u8]) -> Result<NodePtr, std::io::Error> {
    let max_cost = u64::MAX;
    let node_ptr = allocator.new_atom(blob)?;
    let program = wrap_atom_with_decompression_program(allocator, node_ptr)?;
    let dialect = ChiaDialect::new(0);
    let reduction = run_program(
        allocator,
        &dialect,
        program,
        allocator.null(),
        max_cost,
        None,
    )?;
    Ok(reduction.1)
}

pub fn create_autoextracting_clvm_program(input_program: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut allocator = Allocator::new();
    let node_ptr = decompress(&mut allocator, input_program)?;
    let node = Node {
        allocator: &allocator,
        node: node_ptr,
    };
    let compressed_block = node_to_bytes_backrefs(&node).expect("can't compress");
    let compressed_block_as_atom = allocator.new_atom(&compressed_block)?;
    let decompression_program_ptr =
        wrap_atom_with_decompression_program(&mut allocator, compressed_block_as_atom)?;
    node_to_bytes(&Node::new(&allocator, decompression_program_ptr))
}
