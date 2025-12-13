use crate::merkle::error::Error;
use crate::{BLOCK_SIZE, Block, Node, TreeIndex, try_get_block};
use std::collections::{HashSet, VecDeque};

struct LeftChildFirstIteratorItem {
    visited: bool,
    index: TreeIndex,
}

pub struct LeftChildFirstIterator<'a> {
    blob: &'a [u8],
    stack: Vec<LeftChildFirstIteratorItem>,
    already_queued: HashSet<TreeIndex>,
    predicate: Option<fn(&Block) -> bool>,
    from_index: TreeIndex,
}

impl<'a> LeftChildFirstIterator<'a> {
    pub fn new(blob: &'a [u8], from_index: Option<TreeIndex>) -> Self {
        Self::new_with_block_predicate(blob, from_index, None)
    }

    pub fn new_with_block_predicate(
        blob: &'a [u8],
        from_index: Option<TreeIndex>,
        predicate: Option<fn(&Block) -> bool>,
    ) -> Self {
        let mut stack = Vec::new();
        let from_index = from_index.unwrap_or(TreeIndex(0));
        if blob.len() / BLOCK_SIZE > 0 {
            stack.push(LeftChildFirstIteratorItem {
                visited: false,
                index: from_index,
            });
        }

        Self {
            blob,
            stack,
            already_queued: HashSet::new(),
            predicate,
            from_index,
        }
    }
}

impl Iterator for LeftChildFirstIterator<'_> {
    type Item = Result<(TreeIndex, Block), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // left sibling first, children before parents

        loop {
            let item = self.stack.pop()?;
            let block = match try_get_block(self.blob, item.index) {
                Ok(block) => block,
                Err(e) => return Some(Err(e)),
            };

            if let Some(predicate) = self.predicate {
                if !predicate(&block) {
                    continue;
                }
            }

            match block.node.parent().0 {
                Some(index) => {
                    if item.index == TreeIndex(0) {
                        return Some(Err(Error::RootHasParent()));
                    } else if item.index == self.from_index {
                        match try_get_block(self.blob, index) {
                            Ok(Block {
                                node: Node::Internal(node),
                                ..
                            }) => {
                                if item.index != node.left && item.index != node.right {
                                    return Some(Err(Error::ParentDisagreesWithChild()));
                                }
                            }
                            Ok(Block {
                                node: Node::Leaf(_),
                                ..
                            }) => {
                                return Some(Err(Error::LeafCannotBeParent()));
                            }
                            Err(Error::BlockIndexOutOfBounds(_)) => {
                                return Some(Err(Error::ReferenceToUnknownParent()));
                            }
                            Err(e) => return Some(Err(e)),
                        }
                    } else if !self.already_queued.contains(&index) {
                        return Some(Err(Error::ReferenceToUnknownParent()));
                    }
                }
                None => {
                    if item.index.0 != 0 {
                        return Some(Err(Error::UnexpectedParentlessNode()));
                    }
                }
            }

            match block.node {
                Node::Leaf(..) => {
                    if block.metadata.dirty {
                        return Some(Err(Error::DirtyLeaf(item.index)));
                    }
                    return Some(Ok((item.index, block)));
                }
                Node::Internal(ref node) => {
                    if item.visited {
                        return Some(Ok((item.index, block)));
                    }

                    if node.left == node.right
                        || self.already_queued.contains(&node.left)
                        || self.already_queued.contains(&node.right)
                    {
                        return Some(Err(Error::InvalidChildren()));
                    }

                    if self.already_queued.contains(&item.index) {
                        return Some(Err(Error::CycleFound()));
                    }
                    self.already_queued.insert(item.index);

                    self.stack.push(LeftChildFirstIteratorItem {
                        visited: true,
                        index: item.index,
                    });
                    self.stack.push(LeftChildFirstIteratorItem {
                        visited: false,
                        index: node.right,
                    });
                    self.stack.push(LeftChildFirstIteratorItem {
                        visited: false,
                        index: node.left,
                    });
                }
            }
        }
    }
}

pub struct ParentFirstIterator<'a> {
    blob: &'a [u8],
    deque: VecDeque<TreeIndex>,
    already_queued: HashSet<TreeIndex>,
}

impl<'a> ParentFirstIterator<'a> {
    pub fn new(blob: &'a [u8], from_index: Option<TreeIndex>) -> Self {
        let mut deque = VecDeque::new();
        let from_index = from_index.unwrap_or(TreeIndex(0));
        if blob.len() / BLOCK_SIZE > 0 {
            deque.push_back(from_index);
        }

        Self {
            blob,
            deque,
            already_queued: HashSet::new(),
        }
    }
}

impl Iterator for ParentFirstIterator<'_> {
    type Item = Result<(TreeIndex, Block), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // left sibling first, parents before children

        let index = self.deque.pop_front()?;
        let block = match try_get_block(self.blob, index) {
            Ok(block) => block,
            Err(e) => return Some(Err(e)),
        };

        if let Node::Internal(ref node) = block.node {
            if self.already_queued.contains(&index) {
                return Some(Err(Error::CycleFound()));
            }
            self.already_queued.insert(index);

            self.deque.push_back(node.left);
            self.deque.push_back(node.right);
        }

        Some(Ok((index, block)))
    }
}

pub struct BreadthFirstIterator<'a> {
    blob: &'a [u8],
    deque: VecDeque<TreeIndex>,
    already_queued: HashSet<TreeIndex>,
}

impl<'a> BreadthFirstIterator<'a> {
    #[allow(unused)]
    pub fn new(blob: &'a [u8], from_index: Option<TreeIndex>) -> Self {
        let mut deque = VecDeque::new();
        let from_index = from_index.unwrap_or(TreeIndex(0));
        if blob.len() / BLOCK_SIZE > 0 {
            deque.push_back(from_index);
        }

        Self {
            blob,
            deque,
            already_queued: HashSet::new(),
        }
    }
}

impl Iterator for BreadthFirstIterator<'_> {
    type Item = Result<(TreeIndex, Block), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // left sibling first, parent depth before child depth

        loop {
            let index = self.deque.pop_front()?;
            let block = match try_get_block(self.blob, index) {
                Ok(block) => block,
                Err(e) => return Some(Err(e)),
            };

            match block.node {
                Node::Leaf(..) => return Some(Ok((index, block))),
                Node::Internal(node) => {
                    if self.already_queued.contains(&index) {
                        return Some(Err(Error::CycleFound()));
                    }
                    self.already_queued.insert(index);

                    self.deque.push_back(node.left);
                    self.deque.push_back(node.right);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::test_util::open_dot;
    use crate::merkle::test_util::traversal_blob;
    use crate::{Hash, MerkleBlob, NodeType};
    use expect_test::{Expect, expect};
    use rstest::rstest;

    fn iterator_test_reference(index: TreeIndex, block: &Block) -> (u32, NodeType, i64, i64, Hash) {
        match block.node {
            Node::Leaf(leaf) => (
                index.0,
                block.metadata.node_type,
                leaf.key.0,
                leaf.value.0,
                block.node.hash(),
            ),
            Node::Internal(internal) => (
                index.0,
                block.metadata.node_type,
                internal.left.0 as i64,
                internal.right.0 as i64,
                block.node.hash(),
            ),
        }
    }

    #[rstest]
    // expect-test is adding them back
    #[allow(clippy::needless_raw_string_hashes)]
    #[case::left_child_first(
        "left child first",
        LeftChildFirstIterator::new,
        Some(TreeIndex(0)),
        expect![[r#"
            [
                (
                    1,
                    Leaf,
                    2315169217770759719,
                    3472611983179986487,
                    Hash(
                        0f980325ebe9426fa295f3f69cc38ef8fe6ce8f3b9f083556c0f927e67e56651,
                    ),
                ),
                (
                    3,
                    Leaf,
                    103,
                    204,
                    Hash(
                        2d47301cff01acc863faa5f57e8fbc632114f1dc764772852ed0c29c0f248bd3,
                    ),
                ),
                (
                    5,
                    Leaf,
                    307,
                    404,
                    Hash(
                        97148f80dd9289a1b67527c045fd47662d575ccdb594701a56c2255ac84f6113,
                    ),
                ),
                (
                    6,
                    Internal,
                    3,
                    5,
                    Hash(
                        b946284149e4f4a0e767ef2feb397533fb112bf4d99c887348cec4438e38c1ce,
                    ),
                ),
                (
                    4,
                    Internal,
                    1,
                    6,
                    Hash(
                        547b5bd537270427e570df6e43dda7c4ef23e6c3bec72cf19d912c3fe864f549,
                    ),
                ),
                (
                    2,
                    Leaf,
                    283686952306183,
                    1157726452361532951,
                    Hash(
                        d8ddfc94e7201527a6a93ee04aed8c5c122ac38af6dbf6e5f1caefba2597230d,
                    ),
                ),
                (
                    0,
                    Internal,
                    4,
                    2,
                    Hash(
                        cc7f12227cc5d96a631963804544872d67aef8b3a86ef9fbc798f7c5dfdbac2b,
                    ),
                ),
            ]
        "#]],
    )]
    #[allow(clippy::needless_raw_string_hashes)]
    #[case::left_child_first(
        "left child first - from non-root internal",
        LeftChildFirstIterator::new,
        Some(TreeIndex(4)),
        expect![[r#"
            [
                (
                    1,
                    Leaf,
                    2315169217770759719,
                    3472611983179986487,
                    Hash(
                        0f980325ebe9426fa295f3f69cc38ef8fe6ce8f3b9f083556c0f927e67e56651,
                    ),
                ),
                (
                    3,
                    Leaf,
                    103,
                    204,
                    Hash(
                        2d47301cff01acc863faa5f57e8fbc632114f1dc764772852ed0c29c0f248bd3,
                    ),
                ),
                (
                    5,
                    Leaf,
                    307,
                    404,
                    Hash(
                        97148f80dd9289a1b67527c045fd47662d575ccdb594701a56c2255ac84f6113,
                    ),
                ),
                (
                    6,
                    Internal,
                    3,
                    5,
                    Hash(
                        b946284149e4f4a0e767ef2feb397533fb112bf4d99c887348cec4438e38c1ce,
                    ),
                ),
                (
                    4,
                    Internal,
                    1,
                    6,
                    Hash(
                        547b5bd537270427e570df6e43dda7c4ef23e6c3bec72cf19d912c3fe864f549,
                    ),
                ),
            ]
        "#]],
    )]
    #[allow(clippy::needless_raw_string_hashes)]
    #[case::left_child_first(
        "left child first - from non-root leaf",
        LeftChildFirstIterator::new,
        Some(TreeIndex(3)),
        expect![[r#"
            [
                (
                    3,
                    Leaf,
                    103,
                    204,
                    Hash(
                        2d47301cff01acc863faa5f57e8fbc632114f1dc764772852ed0c29c0f248bd3,
                    ),
                ),
            ]
        "#]],
    )]
    // expect-test is adding them back
    #[allow(clippy::needless_raw_string_hashes)]
    #[case::parent_first(
        "parent first",
        ParentFirstIterator::new,
        Some(TreeIndex(0)),
        expect![[r#"
            [
                (
                    0,
                    Internal,
                    4,
                    2,
                    Hash(
                        cc7f12227cc5d96a631963804544872d67aef8b3a86ef9fbc798f7c5dfdbac2b,
                    ),
                ),
                (
                    4,
                    Internal,
                    1,
                    6,
                    Hash(
                        547b5bd537270427e570df6e43dda7c4ef23e6c3bec72cf19d912c3fe864f549,
                    ),
                ),
                (
                    2,
                    Leaf,
                    283686952306183,
                    1157726452361532951,
                    Hash(
                        d8ddfc94e7201527a6a93ee04aed8c5c122ac38af6dbf6e5f1caefba2597230d,
                    ),
                ),
                (
                    1,
                    Leaf,
                    2315169217770759719,
                    3472611983179986487,
                    Hash(
                        0f980325ebe9426fa295f3f69cc38ef8fe6ce8f3b9f083556c0f927e67e56651,
                    ),
                ),
                (
                    6,
                    Internal,
                    3,
                    5,
                    Hash(
                        b946284149e4f4a0e767ef2feb397533fb112bf4d99c887348cec4438e38c1ce,
                    ),
                ),
                (
                    3,
                    Leaf,
                    103,
                    204,
                    Hash(
                        2d47301cff01acc863faa5f57e8fbc632114f1dc764772852ed0c29c0f248bd3,
                    ),
                ),
                (
                    5,
                    Leaf,
                    307,
                    404,
                    Hash(
                        97148f80dd9289a1b67527c045fd47662d575ccdb594701a56c2255ac84f6113,
                    ),
                ),
            ]
        "#]])]
    // expect-test is adding them back
    #[allow(clippy::needless_raw_string_hashes)]
    #[case::breadth_first(
        "breadth first",
        BreadthFirstIterator::new,
        Some(TreeIndex(0)),
        expect![[r#"
            [
                (
                    2,
                    Leaf,
                    283686952306183,
                    1157726452361532951,
                    Hash(
                        d8ddfc94e7201527a6a93ee04aed8c5c122ac38af6dbf6e5f1caefba2597230d,
                    ),
                ),
                (
                    1,
                    Leaf,
                    2315169217770759719,
                    3472611983179986487,
                    Hash(
                        0f980325ebe9426fa295f3f69cc38ef8fe6ce8f3b9f083556c0f927e67e56651,
                    ),
                ),
                (
                    3,
                    Leaf,
                    103,
                    204,
                    Hash(
                        2d47301cff01acc863faa5f57e8fbc632114f1dc764772852ed0c29c0f248bd3,
                    ),
                ),
                (
                    5,
                    Leaf,
                    307,
                    404,
                    Hash(
                        97148f80dd9289a1b67527c045fd47662d575ccdb594701a56c2255ac84f6113,
                    ),
                ),
            ]
        "#]])]
    fn test_iterators<'a, F, T>(
        #[case] note: &str,
        #[case] iterator_new: F,
        #[case] from_index: Option<TreeIndex>,
        #[case] expected: Expect,
        #[by_ref] traversal_blob: &'a MerkleBlob,
    ) where
        F: Fn(&'a [u8], Option<TreeIndex>) -> T,
        T: Iterator<Item = Result<(TreeIndex, Block), Error>>,
    {
        let mut dot_actual = traversal_blob.to_dot().unwrap();
        dot_actual.set_note(note);

        let mut actual = vec![];
        {
            let blob: &[u8] = &traversal_blob.blob;
            for item in iterator_new(blob, from_index) {
                let (index, block) = item.unwrap();
                actual.push(iterator_test_reference(index, &block));
                dot_actual.push_traversal(index);
            }
        }

        traversal_blob.to_dot().unwrap();

        open_dot(&mut dot_actual);

        expected.assert_debug_eq(&actual);
    }
}
