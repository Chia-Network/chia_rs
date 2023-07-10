use clvmr::{allocator::NodePtr, reduction::EvalErr, Allocator};

pub fn new_list(a: &mut Allocator, items: &[NodePtr]) -> Result<NodePtr, EvalErr> {
    let mut result = a.null();
    for &item in items.into_iter().rev() {
        result = a.new_pair(item, result)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use clvmr::serde::node_to_bytes;
    use hex::ToHex;

    use super::*;

    #[test]
    fn test_list() {
        let mut a = Allocator::new();
        let x = a.new_number(5.into()).unwrap();
        let y = a.new_number(8.into()).unwrap();
        let z = a.null();
        let list = new_list(&mut a, &[x, y, z]).unwrap();
        assert_eq!(
            node_to_bytes(&a, list).unwrap().encode_hex::<String>(),
            "ff05ff08ff8080"
        );
    }
}
