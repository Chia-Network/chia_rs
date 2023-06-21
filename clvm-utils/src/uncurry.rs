use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::op_utils::nullp;

fn destructure<const N: usize>(a: &Allocator, mut node: NodePtr) -> Option<[NodePtr; N]> {
    let mut counter = 0;
    let mut ret: [NodePtr; N] = [0; N];
    while let Some((first, rest)) = a.next(node) {
        node = rest;
        if counter == N {
            return None;
        }
        ret[counter] = first;
        counter += 1;
    }
    if counter != N {
        None
    } else {
        Some(ret)
    }
}

fn check(a: &Allocator, n: NodePtr, atom: &[u8]) -> Option<()> {
    match a.sexp(n) {
        SExp::Atom() => {
            if a.atom(n) == atom {
                Some(())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn unwrap_quote(a: &Allocator, n: NodePtr) -> Option<NodePtr> {
    match a.sexp(n) {
        SExp::Pair(first, rest) => {
            check(a, first, &[1_u8])?;
            Some(rest)
        }
        _ => None,
    }
}

// matches
// (2 (1 . self) rest)
// returning (self, rest)
fn match_wrapper(a: &Allocator, node: NodePtr) -> Option<(NodePtr, NodePtr)> {
    let [ev, quoted_inner, args_list] = destructure::<3>(a, node)?;
    check(a, ev, &[2_u8])?;
    let inner = unwrap_quote(a, quoted_inner)?;
    Some((inner, args_list))
}

// returns the inner puzzle and the list of arguments, or Err, in case the node
// is not conforming to the standard curry format
pub fn uncurry(a: &Allocator, node: NodePtr) -> Option<(NodePtr, Vec<NodePtr>)> {
    let mut ret_args = Vec::<NodePtr>::new();

    let (inner, mut args) = match_wrapper(a, node)?;

    while !nullp(a, args) {
        // match
        // (4 (1 . <arg>) <rest>)
        let [cons, quoted_arg, r] = destructure::<3>(a, args)?;
        args = r;
        let arg = unwrap_quote(a, quoted_arg)?;
        check(a, cons, &[4_u8])?;
        ret_args.push(arg);
    }
    Some((inner, ret_args))
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
    assert_eq!(unwrap_quote(&a, quoted_foobar).unwrap(), foobar);
    assert_eq!(unwrap_quote(&a, quoted_foobar).unwrap(), foobar);
    assert_eq!(
        unwrap_quote(&a, double_quoted_foobar).unwrap(),
        quoted_foobar
    );

    // negative tests
    assert!(unwrap_quote(&a, foobar).is_none());
    assert!(unwrap_quote(&a, invalid_quote).is_none());
    assert!(unwrap_quote(&a, a.null()).is_none());
}

#[test]
fn test_check() {
    let mut a = Allocator::new();
    let quote = a.one();
    let foobar = a.new_atom(b"foobar").unwrap();
    let quoted_foobar = a.new_pair(quote, foobar).unwrap();

    assert!(check(&a, quote, &[1_u8]).is_some());
    assert!(check(&a, foobar, b"foobar").is_some());

    // the wrong atom value
    assert!(check(&a, foobar, &[1_u8]).is_none());
    assert!(check(&a, quote, b"foobar").is_none());

    // pairs alwaus fail
    assert!(check(&a, quoted_foobar, b"foobar").is_none());
    assert!(check(&a, quoted_foobar, &[1_u8]).is_none());
}

#[test]
fn test_destructure() {
    let mut a = Allocator::new();
    let foobar = a.new_atom(b"foobar").unwrap();
    let list1 = a.new_pair(foobar, a.null()).unwrap();
    let list2 = a.new_pair(foobar, list1).unwrap();
    let list3 = a.new_pair(foobar, list2).unwrap();
    let list4 = a.new_pair(foobar, list3).unwrap();

    // negative tests
    assert!(destructure::<3>(&a, foobar).is_none());
    assert!(destructure::<3>(&a, list1).is_none());
    assert!(destructure::<3>(&a, list2).is_none());
    assert!(destructure::<3>(&a, list4).is_none());

    // positive test
    let foobar_array = destructure::<3>(&a, list3).unwrap();
    assert!(foobar_array[0] == foobar);
    assert!(foobar_array[1] == foobar);
    assert!(foobar_array[2] == foobar);
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
    let matched = match_wrapper(&a, input).unwrap();
    assert!(matched.0 == inner);
    assert!(matched.1 == rest);

    // negative tests
    assert!(match_wrapper(&a, long_input).is_none());
    assert!(match_wrapper(&a, invalid_input).is_none());
    assert!(match_wrapper(&a, quoted_inner).is_none());
    assert!(match_wrapper(&a, apply).is_none());
    assert!(match_wrapper(&a, rest).is_none());
    assert!(match_wrapper(&a, inner).is_none());
    assert!(match_wrapper(&a, input2).is_none());
    assert!(match_wrapper(&a, input1).is_none());
}
