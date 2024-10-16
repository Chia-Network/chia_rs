use crate::coin_spend::CoinSpend;
use crate::Bytes32;
use crate::Coin;
use chia_bls::G2Element;
use chia_streamable_macro::streamable;
use chia_traits::Streamable;
use clvm_traits::FromClvm;
use clvmr::allocator::{NodePtr, SExp};
use clvmr::cost::Cost;
use clvmr::op_utils::{first, rest};
use clvmr::reduction::EvalErr;
use clvmr::Allocator;

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyType;

#[streamable(subclass)]
pub struct SpendBundle {
    coin_spends: Vec<CoinSpend>,
    aggregated_signature: G2Element,
}

impl SpendBundle {
    pub fn aggregate(spend_bundles: &[SpendBundle]) -> SpendBundle {
        let mut coin_spends = Vec::<CoinSpend>::new();
        let mut aggregated_signature = G2Element::default();
        for sb in spend_bundles {
            coin_spends.extend_from_slice(&sb.coin_spends[..]);
            aggregated_signature.aggregate(&sb.aggregated_signature);
        }
        SpendBundle {
            coin_spends,
            aggregated_signature,
        }
    }

    pub fn name(&self) -> Bytes32 {
        self.hash().into()
    }

    pub fn additions(&self) -> Result<Vec<Coin>, EvalErr> {
        const CREATE_COIN_COST: Cost = 1_800_000;
        const CREATE_COIN: u8 = 51;

        let mut ret = Vec::<Coin>::new();
        let mut cost_left = 11_000_000_000;
        let mut a = Allocator::new();
        let checkpoint = a.checkpoint();

        for cs in &self.coin_spends {
            a.restore_checkpoint(&checkpoint);
            let (cost, mut conds) = cs.puzzle_reveal.run(&mut a, 0, cost_left, &cs.solution)?;
            if cost > cost_left {
                return Err(EvalErr(a.nil(), "cost exceeded".to_string()));
            }
            cost_left -= cost;
            let parent_coin_info: Bytes32 = cs.coin.coin_id();

            while let Some((c, tail)) = a.next(conds) {
                conds = tail;
                let op = first(&a, c)?;
                let c = rest(&a, c)?;
                let buf = match a.sexp(op) {
                    SExp::Atom => a.atom(op),
                    SExp::Pair(..) => return Err(EvalErr(op, "invalid condition".to_string())),
                };
                let buf = buf.as_ref();
                if buf.len() != 1 {
                    continue;
                }
                if buf[0] == CREATE_COIN {
                    let (puzzle_hash, (amount, _)) = <(Bytes32, (u64, NodePtr))>::from_clvm(&a, c)
                        .map_err(|_| EvalErr(c, "failed to parse spend".to_string()))?;
                    ret.push(Coin {
                        parent_coin_info,
                        puzzle_hash,
                        amount,
                    });
                    if CREATE_COIN_COST > cost_left {
                        return Err(EvalErr(a.nil(), "cost exceeded".to_string()));
                    }
                    cost_left -= CREATE_COIN_COST;
                }
            }
        }
        Ok(ret)
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
#[allow(clippy::needless_pass_by_value)]
impl SpendBundle {
    #[classmethod]
    #[pyo3(name = "aggregate")]
    fn py_aggregate(
        cls: &Bound<'_, PyType>,
        py: Python<'_>,
        spend_bundles: Vec<Self>,
    ) -> PyResult<PyObject> {
        let aggregated = Bound::new(py, Self::aggregate(&spend_bundles))?;
        if aggregated.is_exact_instance(cls) {
            Ok(aggregated.into_py(py))
        } else {
            let instance = cls.call_method1("from_parent", (aggregated.into_py(py),))?;
            Ok(instance.into_py(py))
        }
    }

    #[classmethod]
    #[pyo3(name = "from_parent")]
    pub fn from_parent(
        cls: &Bound<'_, PyType>,
        py: Python<'_>,
        spend_bundle: Self,
    ) -> PyResult<PyObject> {
        // Convert result into potential child class
        let instance = cls.call(
            (spend_bundle.coin_spends, spend_bundle.aggregated_signature),
            None,
        )?;

        Ok(instance.into_py(py))
    }

    #[pyo3(name = "name")]
    fn py_name(&self) -> Bytes32 {
        self.name()
    }

    fn removals(&self) -> Vec<Coin> {
        let mut ret = Vec::<Coin>::with_capacity(self.coin_spends.len());
        for cs in &self.coin_spends {
            ret.push(cs.coin);
        }
        ret
    }

    #[pyo3(name = "additions")]
    fn py_additions(&self) -> PyResult<Vec<Coin>> {
        self.additions()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Program;
    use rstest::rstest;
    use std::fs;

    #[rstest]
    #[case(
        "e3c0",
        "fd65e4b0f21322f78d1025e8a8ff7a1df77cd40b86885b851f4572e5ce06e4ff",
        "e3c000a395f8f69d5e263a9548f13bffb1c4b701ab8f3faa03f7647c8750d077"
    )]
    #[case(
        "bb13",
        "6b2aaee962cb1de3fdeb1f0506c02df4b9e162e2af3dd1db22048454b5122a87",
        "bb13d1e13438736c7ba0217c7b82ee4db56a7f4fb9d22c703c2152362b2314ee"
    )]
    fn test_additions_ff(
        #[case] spend_file: &str,
        #[case] expect_parent: &str,
        #[case] expect_ph: &str,
    ) {
        let spend_bytes =
            fs::read(format!("../../ff-tests/{spend_file}.spend")).expect("read file");
        let spend = CoinSpend::from_bytes(&spend_bytes).expect("parse CoinSpend");
        let bundle = SpendBundle::new(vec![spend], G2Element::default());

        let additions = bundle.additions().expect("additions");

        assert_eq!(additions.len(), 1);
        assert_eq!(
            additions[0].parent_coin_info.as_ref(),
            &hex::decode(expect_parent).expect("hex::decode")
        );
        assert_eq!(
            additions[0].puzzle_hash.as_ref(),
            &hex::decode(expect_ph).expect("hex::decode")
        );
        assert_eq!(additions[0].amount, 1);
    }

    fn test_impl<F: Fn(Coin, SpendBundle)>(solution: &str, body: F) {
        let solution = hex::decode(solution).expect("hex::decode");
        let test_coin = Coin::new(
            hex::decode("4444444444444444444444444444444444444444444444444444444444444444")
                .unwrap()
                .try_into()
                .unwrap(),
            hex::decode("3333333333333333333333333333333333333333333333333333333333333333")
                .unwrap()
                .try_into()
                .unwrap(),
            1,
        );
        let spend = CoinSpend::new(
            test_coin,
            Program::new(vec![1_u8].into()),
            Program::new(solution.into()),
        );
        let bundle = SpendBundle::new(vec![spend], G2Element::default());
        body(test_coin, bundle);
    }

    // TODO: Once we have condition types that implement ToClvm and an Encoder
    // that serialize directly to bytes, these test solutions can be expressed
    // in a much more readable way
    #[test]
    fn test_single_create_coin() {
        // This is a solution to the identity puzzle:
        // ((CREATE_COIN . (222222..22 . (1 . NIL))) .
        // ))
        let solution = "ff\
ff33\
ffa02222222222222222222222222222222222222222222222222222222222222222\
ff01\
80\
80";
        test_impl(solution, |test_coin: Coin, bundle: SpendBundle| {
            let additions = bundle.additions().expect("additions");

            let new_coin = Coin::new(
                test_coin.coin_id(),
                hex::decode("2222222222222222222222222222222222222222222222222222222222222222")
                    .unwrap()
                    .try_into()
                    .unwrap(),
                1,
            );
            assert_eq!(additions, [new_coin]);
        });
    }

    #[test]
    fn test_invalid_condition() {
        // This is a solution to the identity puzzle:
        // (((1 . CREATE_COIN) . (222222..22 . (1 . NIL))) .
        // ))
        let solution = "ff\
ffff0133\
ffa02222222222222222222222222222222222222222222222222222222222222222\
ff01\
80\
80";

        test_impl(solution, |_test_coin, bundle: SpendBundle| {
            assert_eq!(bundle.additions().unwrap_err().1, "invalid condition");
        });
    }

    #[test]
    fn test_invalid_spend() {
        // This is a solution to the identity puzzle:
        // ((CREATE_COIN . (222222..22 . ((1 . 1) . NIL))) .
        // ))
        let solution = "ff\
ff33\
ffa02222222222222222222222222222222222222222222222222222222222222222\
ffff0101\
80\
80";

        test_impl(solution, |_test_coin, bundle: SpendBundle| {
            assert_eq!(bundle.additions().unwrap_err().1, "failed to parse spend");
        });
    }
}
