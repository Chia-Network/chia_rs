use clvmr::allocator::{Allocator, NodePtr};
use clvmr::node::Node;

fn unwrap3(node: Node) -> Option<(Node, Node, Node)> {
    let mut i = node;
    let n1 = i.next()?;
    let n2 = i.next()?;
    let n3 = i.next()?;
    if i.next().is_some() {
        return None;
    }
    Some((n1, n2, n3))
}

fn check(n: &Node, atom: &[u8]) -> Option<()> {
    if n.atom()? == atom {
        Some(())
    } else {
        None
    }
}

fn unwrap_quote(node: Node) -> Option<Node> {
    let p = node.pair()?;
    check(&p.0, &[1_u8])?;
    Some(p.1)
}

// matches
// (2 (1 . self) rest)
// returning (self, rest)
fn match_wrapper(node: Node) -> Option<(Node, Node)> {
    let (ev, quoted_inner, args_list) = unwrap3(node)?;
    check(&ev, &[2_u8])?;
    let inner = unwrap_quote(quoted_inner)?;
    Some((inner, args_list))
}

// returns the inner puzzle and the list of arguments, or Err, in case the node
// is not conforming to the standard curry format
pub fn uncurry(a: &Allocator, node: NodePtr) -> Option<(NodePtr, Vec<NodePtr>)> {
    let mut ret_args = Vec::<NodePtr>::new();

    let n = Node::new(a, node);
    let (inner, args) = match_wrapper(n)?;

    let mut rest = args;
    while rest.pair().is_some() {
        // match
        // (4 (1 . <arg>) <rest>)
        let (cons, quoted_arg, r) = unwrap3(rest.clone())?;
        rest = r;
        let arg = unwrap_quote(quoted_arg)?;
        check(&cons, &[4_u8])?;
        ret_args.push(arg.node);
    }
    Some((inner.node, ret_args))
}

// ==== tests ===

#[test]
fn simple_uncurry() {
    let mut a = Allocator::new();
    let inner = a.new_atom(b"abcdefghijklmnopqrstuvwxyz012345").unwrap();
    let null = a.null();
    let quote = a.one();
    let apply = a.new_atom(&[2_u8]).unwrap();
    let cons = a.new_atom(&[4_u8]).unwrap();
    let foobar = a.new_atom(b"foobar").unwrap();

    // (q . "foobar")
    let quoted_foobar = a.new_pair(quote, foobar).unwrap();
    // (c (q . "fobar") ())
    // 3 times, for 3 foobar arguments
    let args = a.new_pair(null, null).unwrap();
    let args = a.new_pair(quoted_foobar, args).unwrap();
    let args = a.new_pair(cons, args).unwrap();

    let args = a.new_pair(args, null).unwrap();
    let args = a.new_pair(quoted_foobar, args).unwrap();
    let args = a.new_pair(cons, args).unwrap();

    let args = a.new_pair(args, null).unwrap();
    let args = a.new_pair(quoted_foobar, args).unwrap();
    let args = a.new_pair(cons, args).unwrap();

    let quoted_inner = a.new_pair(quote, inner).unwrap();

    let wrapper = a.new_pair(args, null).unwrap();
    let wrapper = a.new_pair(quoted_inner, wrapper).unwrap();
    let wrapper = a.new_pair(apply, wrapper).unwrap();

    assert!(uncurry(&a, wrapper).unwrap() == (inner, vec![foobar, foobar, foobar]));
}

#[test]
fn test_unwrap_quote() {
    let mut a = Allocator::new();
    let quote = a.one();
    let foobar = a.new_atom(b"foobar").unwrap();
    let quoted_foobar = a.new_pair(quote, foobar).unwrap();
    let double_quoted_foobar = a.new_pair(quote, quoted_foobar).unwrap();
    let invalid_quote = a.new_pair(a.null(), foobar).unwrap();

    // positive tests
    assert_eq!(
        unwrap_quote(Node::new(&a, quoted_foobar)).unwrap().node,
        foobar
    );
    assert_eq!(
        unwrap_quote(Node::new(&a, double_quoted_foobar))
            .unwrap()
            .node,
        quoted_foobar
    );

    // negative tests
    assert!(unwrap_quote(Node::new(&a, foobar)).is_none());
    assert!(unwrap_quote(Node::new(&a, invalid_quote)).is_none());
    assert!(unwrap_quote(Node::new(&a, a.null())).is_none());
}

#[test]
fn test_check() {
    let mut a = Allocator::new();
    let quote = a.one();
    let foobar = a.new_atom(b"foobar").unwrap();
    let quoted_foobar = a.new_pair(quote, foobar).unwrap();

    assert!(check(&Node::new(&a, quote), &[1_u8]).is_some());
    assert!(check(&Node::new(&a, foobar), b"foobar").is_some());

    // the wrong atom value
    assert!(check(&Node::new(&a, foobar), &[1_u8]).is_none());
    assert!(check(&Node::new(&a, quote), b"foobar").is_none());

    // pairs alwaus fail
    assert!(check(&Node::new(&a, quoted_foobar), b"foobar").is_none());
    assert!(check(&Node::new(&a, quoted_foobar), &[1_u8]).is_none());
}

#[test]
fn test_unwrap3() {
    let mut a = Allocator::new();
    let foobar = a.new_atom(b"foobar").unwrap();
    let list1 = a.new_pair(foobar, a.null()).unwrap();
    let list2 = a.new_pair(foobar, list1).unwrap();
    let list3 = a.new_pair(foobar, list2).unwrap();
    let list4 = a.new_pair(foobar, list3).unwrap();

    // negative tests
    assert!(unwrap3(Node::new(&a, foobar)).is_none());
    assert!(unwrap3(Node::new(&a, list1)).is_none());
    assert!(unwrap3(Node::new(&a, list2)).is_none());
    assert!(unwrap3(Node::new(&a, list4)).is_none());

    // positive test
    let foobar_tuple = unwrap3(Node::new(&a, list3)).unwrap();
    assert!(foobar_tuple.0.node == foobar);
    assert!(foobar_tuple.1.node == foobar);
    assert!(foobar_tuple.2.node == foobar);
}

#[test]
fn test_match_wrapper() {
    let mut a = Allocator::new();
    let apply = a.new_atom(&[2_u8]).unwrap();
    let rest = a.new_atom(b"args").unwrap();
    let inner = a.new_atom(b"inner").unwrap();
    let quoted_inner = a.new_pair(a.one(), inner).unwrap();

    let input2 = a.new_pair(rest, a.null()).unwrap();
    let input1 = a.new_pair(quoted_inner, input2).unwrap();
    let input = a.new_pair(apply, input1).unwrap();
    let invalid_input = a.new_pair(a.one(), input1).unwrap();
    let long_input = a.new_pair(apply, input).unwrap();

    // input: (2 (1 . self) rest)
    // returns: (self, rest)
    let matched = match_wrapper(Node::new(&a, input)).unwrap();
    assert!(matched.0.node == inner);
    assert!(matched.1.node == rest);

    // negative tests
    assert!(match_wrapper(Node::new(&a, long_input)).is_none());
    assert!(match_wrapper(Node::new(&a, invalid_input)).is_none());
    assert!(match_wrapper(Node::new(&a, quoted_inner)).is_none());
    assert!(match_wrapper(Node::new(&a, apply)).is_none());
    assert!(match_wrapper(Node::new(&a, rest)).is_none());
    assert!(match_wrapper(Node::new(&a, inner)).is_none());
    assert!(match_wrapper(Node::new(&a, input2)).is_none());
    assert!(match_wrapper(Node::new(&a, input1)).is_none());
}
