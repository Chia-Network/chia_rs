use chia_protocol::Coin;
use chia_protocol::CoinSpend;
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::serde::{node_from_bytes_backrefs, node_to_bytes, node_to_bytes_backrefs};
use std::io;

// the tuple has the Coin, puzzle-reveal and solution
fn build_generator<BufRef, I>(a: &mut Allocator, spends: I) -> io::Result<NodePtr>
where
    BufRef: AsRef<[u8]>,
    I: IntoIterator<Item = (Coin, BufRef, BufRef)>,
{
    // the generator we produce here is just a quoted list. Nothing fancy.
    // Its format is as follows:
    // (q . ( ( ( parent-id puzzle-reveal amount solution ) ... ) ) )

    let mut spend_list = a.nil();
    for s in spends {
        let item = a.nil();
        // solution
        let solution = node_from_bytes_backrefs(a, s.2.as_ref())?;
        let item = a.new_pair(solution, item)?;
        // amount
        let amount = a.new_number(s.0.amount.into())?;
        let item = a.new_pair(amount, item)?;
        // puzzle reveal
        let puzzle = node_from_bytes_backrefs(a, s.1.as_ref())?;
        let item = a.new_pair(puzzle, item)?;
        // parent-id
        let parent_id = a.new_atom(&s.0.parent_coin_info)?;
        let item = a.new_pair(parent_id, item)?;

        spend_list = a.new_pair(item, spend_list)?;
    }

    // the list of spends is the first (and only) item in an outer list
    spend_list = a.new_pair(spend_list, a.nil())?;

    let quote = a.new_pair(a.one(), spend_list)?;
    Ok(quote)
}

// this function returns the number of bytes the specified
// number is serialized to, in CLVM serialized form
fn clvm_bytes_len(val: u64) -> usize {
    if val < 0x80 {
        1
    } else if val < 0x8000 {
        3
    } else if val < 0x0080_0000 {
        4
    } else if val < 0x8000_0000 {
        5
    } else if val < 0x0080_0000_0000 {
        6
    } else if val < 0x8000_0000_0000 {
        7
    } else if val < 0x0080_0000_0000_0000 {
        8
    } else if val < 0x8000_0000_0000_0000 {
        9
    } else {
        10
    }
}

// calculate the size in bytes of a generator with no backref optimisations
pub fn calculate_generator_length<I>(spends: I) -> usize
where
    I: AsRef<[CoinSpend]>,
{
    let mut size: usize = 5; // (q . (())) => ff01ff8080 => 5 bytes

    for s in spends.as_ref() {
        let puzzle = s.puzzle_reveal.as_ref();
        let solution = s.solution.as_ref();
        // Each spend has the following form:
        // ( parent-id puzzle-reveal amount solution )
        // parent-id is always 32 bytes + 1 byte length prefix = 33
        // + 6 bytes for list extension
        // coin amount is already prepended correctly in clvm_bytes_len()
        size += 39 + puzzle.len() + clvm_bytes_len(s.coin.amount) + solution.len();
    }

    size
}

// the tuple has the Coin, puzzle-reveal and solution
pub fn solution_generator<BufRef, I>(spends: I) -> io::Result<Vec<u8>>
where
    BufRef: AsRef<[u8]>,
    I: IntoIterator<Item = (Coin, BufRef, BufRef)>,
{
    let mut a = Allocator::new();
    let generator = build_generator(&mut a, spends)?;
    node_to_bytes(&a, generator)
}

pub fn solution_generator_backrefs<BufRef, I>(spends: I) -> io::Result<Vec<u8>>
where
    BufRef: AsRef<[u8]>,
    I: IntoIterator<Item = (Coin, BufRef, BufRef)>,
{
    let mut a = Allocator::new();
    let generator = build_generator(&mut a, spends)?;
    node_to_bytes_backrefs(&a, generator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chia_protocol::Program;
    use chia_traits::Streamable;
    use clvmr::{run_program, ChiaDialect};
    use hex_literal::hex;
    use rstest::rstest;

    const PUZZLE1: [u8; 291] = hex!(
        "
        ff02ffff01ff02ffff01ff02ffff03ff0bffff01ff02ffff03ffff09ff05ffff
        1dff0bffff1effff0bff0bffff02ff06ffff04ff02ffff04ff17ff8080808080
        808080ffff01ff02ff17ff2f80ffff01ff088080ff0180ffff01ff04ffff04ff
        04ffff04ff05ffff04ffff02ff06ffff04ff02ffff04ff17ff80808080ff8080
        8080ffff02ff17ff2f808080ff0180ffff04ffff01ff32ff02ffff03ffff07ff
        0580ffff01ff0bffff0102ffff02ff06ffff04ff02ffff04ff09ff80808080ff
        ff02ff06ffff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff0580
        80ff0180ff018080ffff04ffff01b08cf5533a94afae0f4613d3ea565e47abc5
        373415967ef5824fd009c602cb629e259908ce533c21de7fd7a68eb96c52d0ff
        018080"
    );

    const SOLUTION1: [u8; 47] = hex!(
        "
        ff80ffff01ffff3dffa080115c1c71035a2cd60a49499fb9e5cb55be8d6e25e8
        680bfc0409b7acaeffd48080ff8080"
    );

    const PUZZLE2: [u8; 99] = hex!(
        "
        ff01ffff33ffa01b7ab2079fa635554ad9bd4812c622e46ee3b1875a7813afba
        127bb0cc9794f9ff887f808e9291e6c00080ffff33ffa06f184a7074c925ef86
        88ce56941eb8929be320265f824ec7e351356cc745d38aff887f808e9291e6c0
        008080"
    );

    const SOLUTION2: [u8; 1] = hex!("80");

    fn run_generator(program: &[u8]) -> Vec<u8> {
        let dialect = ChiaDialect::new(0);
        let mut a = Allocator::new();
        let program = node_from_bytes_backrefs(&mut a, program).expect("node_from_bytes");
        let env = a.nil();
        let generator_output = run_program(&mut a, &dialect, program, env, 11_000_000_000)
            .expect("run_program")
            .1;
        node_to_bytes(&a, generator_output).expect("node_to_bytes")
    }

    const EXPECTED_GENERATOR_OUTPUT: [u8; 536] = hex!(
        "
        ffffffa0ccd5bb71183532bff220ba46c268991a000000000000000000000000
        00000000ffff01ffff33ffa01b7ab2079fa635554ad9bd4812c622e46ee3b187
        5a7813afba127bb0cc9794f9ff887f808e9291e6c00080ffff33ffa06f184a70
        74c925ef8688ce56941eb8929be320265f824ec7e351356cc745d38aff887f80
        8e9291e6c0008080ff8900ff011d2523cd8000ff8080ffffa0ccd5bb71183532
        bff220ba46c268991a00000000000000000000000000036840ffff02ffff01ff
        02ffff01ff02ffff03ff0bffff01ff02ffff03ffff09ff05ffff1dff0bffff1e
        ffff0bff0bffff02ff06ffff04ff02ffff04ff17ff8080808080808080ffff01
        ff02ff17ff2f80ffff01ff088080ff0180ffff01ff04ffff04ff04ffff04ff05
        ffff04ffff02ff06ffff04ff02ffff04ff17ff80808080ff80808080ffff02ff
        17ff2f808080ff0180ffff04ffff01ff32ff02ffff03ffff07ff0580ffff01ff
        0bffff0102ffff02ff06ffff04ff02ffff04ff09ff80808080ffff02ff06ffff
        04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff058080ff0180ff01
        8080ffff04ffff01b08cf5533a94afae0f4613d3ea565e47abc5373415967ef5
        824fd009c602cb629e259908ce533c21de7fd7a68eb96c52d0ff018080ff8601
        977420dc00ffff80ffff01ffff3dffa080115c1c71035a2cd60a49499fb9e5cb
        55be8d6e25e8680bfc0409b7acaeffd48080ff8080808080"
    );

    #[test]
    fn test_solution_generator() {
        let coin1: Coin = Coin::new(
            hex!("ccd5bb71183532bff220ba46c268991a00000000000000000000000000036840").into(),
            hex!("fcc78a9e396df6ceebc217d2446bc016e0b3d5922fb32e5783ec5a85d490cfb6").into(),
            1_750_000_000_000,
        );
        let coin2: Coin = Coin::new(
            hex!("ccd5bb71183532bff220ba46c268991a00000000000000000000000000000000").into(),
            hex!("d23da14695a188ae5708dd152263c4db883eb27edeb936178d4d988b8f3ce5fc").into(),
            18_375_000_000_000_000_000,
        );

        let spends = [
            (coin1, PUZZLE1.as_ref(), SOLUTION1.as_ref()),
            (coin2, PUZZLE2.as_ref(), SOLUTION2.as_ref()),
        ];

        let result = solution_generator(spends).expect("solution_generator");

        assert_eq!(
            result.len(),
            calculate_generator_length([
                CoinSpend {
                    coin: coin1,
                    puzzle_reveal: Program::from(PUZZLE1.as_ref()),
                    solution: Program::from(SOLUTION1.as_ref())
                },
                CoinSpend {
                    coin: coin2,
                    puzzle_reveal: Program::from(PUZZLE2.as_ref()),
                    solution: Program::from(SOLUTION2.as_ref())
                },
            ])
        );

        assert_eq!(
            result,
            hex!(
                "
            ff01ffffffa0
            ccd5bb71183532bff220ba46c268991a00000000000000000000000000000000

            ff

            ff01ffff33ffa01b7ab2079fa635554ad9bd4812c622e46ee3b1875a7813afba
            127bb0cc9794f9ff887f808e9291e6c00080ffff33ffa06f184a7074c925ef86
            88ce56941eb8929be320265f824ec7e351356cc745d38aff887f808e9291e6c0
            008080

            ff8900ff011d2523cd8000ff

            80

            80ffffa0

            ccd5bb71183532bff220ba46c268991a00000000000000000000000000036840

            ff

            ff02ffff01ff02ffff01ff02ffff03ff0bffff01ff02ffff03ffff09ff05ffff
            1dff0bffff1effff0bff0bffff02ff06ffff04ff02ffff04ff17ff8080808080
            808080ffff01ff02ff17ff2f80ffff01ff088080ff0180ffff01ff04ffff04ff
            04ffff04ff05ffff04ffff02ff06ffff04ff02ffff04ff17ff80808080ff8080
            8080ffff02ff17ff2f808080ff0180ffff04ffff01ff32ff02ffff03ffff07ff
            0580ffff01ff0bffff0102ffff02ff06ffff04ff02ffff04ff09ff80808080ff
            ff02ff06ffff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff0580
            80ff0180ff018080ffff04ffff01b08cf5533a94afae0f4613d3ea565e47abc5
            373415967ef5824fd009c602cb629e259908ce533c21de7fd7a68eb96c52d0ff
            018080

            ff8601977420dc00ff

            ff80ffff01ffff3dffa080115c1c71035a2cd60a49499fb9e5cb55be8d6e25e8
            680bfc0409b7acaeffd48080ff8080

            808080"
            )
        );

        let generator_output = run_generator(&result);
        assert_eq!(generator_output, EXPECTED_GENERATOR_OUTPUT);

        let spends = [(coin2, PUZZLE2.as_ref(), SOLUTION2.as_ref())];
        let result = solution_generator(spends).expect("solution_generator");

        assert_eq!(
            result.len(),
            calculate_generator_length([CoinSpend {
                coin: coin2,
                puzzle_reveal: Program::from(PUZZLE2.as_ref()),
                solution: Program::from(SOLUTION2.as_ref())
            },])
        );

        assert_eq!(
            result,
            hex!(
                "
            ff01ffffffa0
            ccd5bb71183532bff220ba46c268991a00000000000000000000000000000000

            ff

            ff01ffff33ffa01b7ab2079fa635554ad9bd4812c622e46ee3b1875a7813afba
            127bb0cc9794f9ff887f808e9291e6c00080ffff33ffa06f184a7074c925ef86
            88ce56941eb8929be320265f824ec7e351356cc745d38aff887f808e9291e6c0
            008080

            ff8900ff011d2523cd8000ff

            80

            808080"
            )
        );
    }

    #[test]
    fn test_length_calculator() {
        let mut spends: Vec<(Coin, &[u8], &[u8])> = Vec::new();
        let mut coin_spends = Vec::<CoinSpend>::new();
        for i in [
            0,
            1,
            128,
            129,
            256,
            257,
            4_294_967_296,
            4_294_967_297,
            0x8000_0001,
            0x8000_0000_0001,
            0x8000_0000_0000_0001,
        ] {
            let coin: Coin = Coin::new(
                hex!("ccd5bb71183532bff220ba46c268991a00000000000000000000000000036840").into(),
                hex!("fcc78a9e396df6ceebc217d2446bc016e0b3d5922fb32e5783ec5a85d490cfb6").into(),
                i,
            );
            spends.push((coin, PUZZLE1.as_ref(), SOLUTION1.as_ref()));
            coin_spends.push(CoinSpend {
                coin,
                puzzle_reveal: Program::from(PUZZLE1.as_ref()),
                solution: Program::from(SOLUTION1.as_ref()),
            });
            let result = solution_generator(spends.clone()).expect("solution_generator");

            assert_eq!(result.len(), calculate_generator_length(&coin_spends));
        }
    }

    #[rstest]
    #[case(hex!("f800000000").as_ref(), SOLUTION1.as_ref())]
    #[case(hex!("fffffe0000ff41ff013a").as_ref(), SOLUTION1.as_ref())]
    #[case(PUZZLE1.as_ref(), hex!("00").as_ref())]
    fn test_length_calculator_edge_case(#[case] puzzle: &[u8], #[case] solution: &[u8]) {
        let mut spends: Vec<(Coin, &[u8], &[u8])> = Vec::new();
        let mut coin_spends = Vec::<CoinSpend>::new();
        let mut a = Allocator::new();
        let mut discrepancy: i64 = 0;

        let coin: Coin = Coin::new(
            hex!("ccd5bb71183532bff220ba46c268991a00000000000000000000000000036840").into(),
            hex!("fcc78a9e396df6ceebc217d2446bc016e0b3d5922fb32e5783ec5a85d490cfb6").into(),
            100,
        );
        spends.push((coin, puzzle, solution));
        coin_spends.push(CoinSpend {
            coin,
            puzzle_reveal: Program::from_bytes(puzzle).expect("puzzle_reveal"),
            solution: Program::from_bytes(solution).expect("solution"),
        });
        let node = node_from_bytes_backrefs(&mut a, puzzle).expect("puzzle");
        let puz = node_to_bytes(&a, node).expect("bytes");
        discrepancy += puzzle.len() as i64 - puz.len() as i64;
        let node = node_from_bytes_backrefs(&mut a, solution).expect("solution");
        let sol = node_to_bytes(&a, node).expect("bytes");
        discrepancy += solution.len() as i64 - sol.len() as i64;
        let result = solution_generator(spends.clone()).expect("solution_generator");

        assert_eq!(
            result.len() as i64,
            calculate_generator_length(&coin_spends) as i64 - discrepancy
        );
    }

    #[test]
    fn test_solution_generator_backre() {
        let coin1: Coin = Coin::new(
            hex!("ccd5bb71183532bff220ba46c268991a00000000000000000000000000036840").into(),
            hex!("fcc78a9e396df6ceebc217d2446bc016e0b3d5922fb32e5783ec5a85d490cfb6").into(),
            1_750_000_000_000,
        );
        let coin2: Coin = Coin::new(
            hex!("ccd5bb71183532bff220ba46c268991a00000000000000000000000000000000").into(),
            hex!("d23da14695a188ae5708dd152263c4db883eb27edeb936178d4d988b8f3ce5fc").into(),
            18_375_000_000_000_000_000,
        );

        let result = solution_generator_backrefs([
            (coin1, PUZZLE1.as_ref(), SOLUTION1.as_ref()),
            (coin2, PUZZLE2.as_ref(), SOLUTION2.as_ref()),
        ])
        .expect("solution_generator");

        assert_eq!(
            result,
            hex!(
                "
                ff01ffffffa0

                ccd5bb71183532bff220ba46c268991a00000000000000000000000000000000

                ff

                ff01ffff33ffa01b7ab2079fa635554ad9bd4812c622e46ee3b1875a7813afba
                127bb0cc9794f9ff887f808e9291e6c00080ffff33ffa06f184a7074c925ef86
                88ce56941eb8929be320265f824ec7e351356cc745d38a

                fe3b

                80ff8900ff011d2523cd8000ff8080ffffa0

                ccd5bb71183532bff220ba46c268991a00000000000000000000000000036840

                ff

                ff02ffff01ff02ffff01ff02ffff03ff0bffff01ff02ffff03ffff09ff05ffff
                1dff0bffff1effff0bff0bffff02ff06ffff04ff02ffff04ff17ff8080808080
                808080ffff01ff02ff17ff2f80ffff01ff088080ff0180ffff01ff04ffff04ff
                04ffff04ff05ffff04ff

                fe8401

                6b6b7fff80808080ff

                fe820d

                b78080

                fe81f6

                ffff04ffff01ff32ff02ffff03ffff07ff0580ffff01ff0bffff0102ffff02ff
                06ffff04ff02ffff04ff09ff80808080ffff02ff06ffff04ff02ffff04ff0dff
                8080808080ffff01ff0bffff0101

                fe6f

                80ff0180

                fe3e

                80ffff04ffff01b08cf5533a94afae0f4613d3ea565e47abc5373415967ef582
                4fd009c602cb629e259908ce533c21de7fd7a68eb96c52d0

                fe7f

                80ff8601977420dc00ffff80ffff01ffff3dffa080115c1c71035a2cd60a4949
                9fb9e5cb55be8d6e25e8680bfc0409b7acaeffd48080ff8080808080"
            )
        );

        let generator_output = run_generator(&result);
        assert_eq!(generator_output, EXPECTED_GENERATOR_OUTPUT);
    }

    #[rstest]
    #[case(0)]
    #[case(1)]
    #[case(0x7f)]
    #[case(0x80)]
    #[case(0xff)]
    #[case(0x7fff)]
    #[case(0x8000)]
    #[case(0xffff)]
    #[case(0x7f_ffff)]
    #[case(0x80_0000)]
    #[case(0xff_ffff)]
    #[case(0x7fff_ffff)]
    #[case(0x8000_0000)]
    #[case(0xffff_ffff)]
    #[case(0x7f_ffff_ffff)]
    #[case(0x80_0000_0000)]
    #[case(0xff_ffff_ffff)]
    #[case(0x7fff_ffff_ffff)]
    #[case(0x8000_0000_0000)]
    #[case(0xffff_ffff_ffff)]
    #[case(0x7f_ffff_ffff_ffff)]
    #[case(0x80_0000_0000_0000)]
    #[case(0xff_ffff_ffff_ffff)]
    #[case(0x7fff_ffff_ffff_ffff)]
    #[case(0x8000_0000_0000_0000)]
    #[case(0xffff_ffff_ffff_ffff)]
    #[case(0x7)]
    #[case(0x8)]
    #[case(0xf)]
    #[case(0x7ff)]
    #[case(0x800)]
    #[case(0xfff)]
    #[case(0x7ffff)]
    #[case(0x80000)]
    #[case(0xfffff)]
    #[case(0x7ff_ffff)]
    #[case(0x800_0000)]
    #[case(0xfff_ffff)]
    #[case(0x7_ffff_ffff)]
    #[case(0x8_0000_0000)]
    #[case(0xf_ffff_ffff)]
    #[case(0x7ff_ffff_ffff)]
    #[case(0x800_0000_0000)]
    #[case(0xfff_ffff_ffff)]
    #[case(0x7_ffff_ffff_ffff)]
    #[case(0x8_0000_0000_0000)]
    #[case(0xf_ffff_ffff_ffff)]
    #[case(0x7ff_ffff_ffff_ffff)]
    #[case(0x800_0000_0000_0000)]
    #[case(0xfff_ffff_ffff_ffff)]
    fn test_clvm_bytes_len(#[case] n: u64) {
        let len = clvm_bytes_len(n);
        let mut a = Allocator::new();
        let atom = a.new_number(n.into()).expect("new_number");
        let bytes = node_to_bytes(&a, atom).expect("node_to_bytes");
        assert_eq!(bytes.len(), len);
    }
}
