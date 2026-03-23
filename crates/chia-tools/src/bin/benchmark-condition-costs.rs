use chia_bls::{SecretKey, Signature};
use chia_consensus::conditions::{EmptyVisitor, parse_spends};
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::ConsensusFlags;
use chia_consensus::opcodes;
use chia_consensus::opcodes::ConditionOpcode;
use chia_protocol::Bytes32;
use chia_protocol::Coin;
use chia_sha2::Sha256;
use clvmr::allocator::{Allocator, NodePtr};
use hex_literal::hex;
use std::fs;
use std::io::Write;
use std::time::Instant;

fn opcode_name(op: ConditionOpcode) -> &'static str {
    match op {
        opcodes::AGG_SIG_UNSAFE => "AggSigUnsafe",
        opcodes::AGG_SIG_ME => "AggSigMe",
        opcodes::AGG_SIG_PARENT => "AggSigParent",
        opcodes::AGG_SIG_PUZZLE => "AggSigPuzzle",
        opcodes::AGG_SIG_AMOUNT => "AggSigAmount",
        opcodes::AGG_SIG_PARENT_AMOUNT => "AggSigParentAmount",
        opcodes::AGG_SIG_PARENT_PUZZLE => "AggSigParentPuzzle",
        opcodes::AGG_SIG_PUZZLE_AMOUNT => "AggSigPuzzleAmount",
        opcodes::REMARK => "Remark",
        opcodes::ASSERT_MY_COIN_ID => "AssertMyCoinId",
        opcodes::ASSERT_MY_PARENT_ID => "AssertMyParentId",
        opcodes::ASSERT_MY_PUZZLEHASH => "AssertMyPuzzlehash",
        opcodes::ASSERT_MY_AMOUNT => "AssertMyAmount",
        opcodes::ASSERT_MY_BIRTH_HEIGHT => "AssertMyBirthHeight",
        opcodes::ASSERT_MY_BIRTH_SECONDS => "AssertMyBirthSeconds",
        opcodes::ASSERT_SECONDS_RELATIVE => "AssertSecondsRelative",
        opcodes::ASSERT_SECONDS_ABSOLUTE => "AssertSecondsAbsolute",
        opcodes::ASSERT_HEIGHT_RELATIVE => "AssertHeightRelative",
        opcodes::ASSERT_HEIGHT_ABSOLUTE => "AssertHeightAbsolute",
        opcodes::ASSERT_BEFORE_SECONDS_RELATIVE => "AssertBeforeSecondsRelative",
        opcodes::ASSERT_BEFORE_SECONDS_ABSOLUTE => "AssertBeforeSecondsAbsolute",
        opcodes::ASSERT_BEFORE_HEIGHT_RELATIVE => "AssertBeforeHeightRelative",
        opcodes::ASSERT_BEFORE_HEIGHT_ABSOLUTE => "AssertBeforeHeightAbsolute",
        opcodes::SOFTFORK => "Softfork",
        opcodes::ASSERT_CONCURRENT_SPEND => "AssertConcurrentSpend",
        opcodes::ASSERT_CONCURRENT_PUZZLE => "AssertConcurrentPuzzle",
        opcodes::ASSERT_EPHEMERAL => "AssertEphemeral",
        opcodes::CREATE_COIN_ANNOUNCEMENT => "CreateCoinAnnouncement",
        opcodes::ASSERT_COIN_ANNOUNCEMENT => "AssertCoinAnnouncement",
        opcodes::CREATE_PUZZLE_ANNOUNCEMENT => "CreatePuzzleAnnouncement",
        opcodes::ASSERT_PUZZLE_ANNOUNCEMENT => "AssertPuzzleAnnouncement",
        _ => panic!("unknown opcode: {op}"),
    }
}

struct ConditionTest<'a> {
    opcode: ConditionOpcode,
    args: &'a [NodePtr],
}

const H1: &[u8; 32] = &[1; 32];
const H2: &[u8; 32] = &[2; 32];
const AMOUNT: u64 = 100;

const SECRET_KEY: &[u8; 32] =
    &hex!("6fc9d9a2b05fd1f0e51bc91041a03be8657081f272ec281aff731624f0d1c220");

const FLAGS: ConsensusFlags =
    ConsensusFlags::DONT_VALIDATE_SIGNATURE.union(ConsensusFlags::COST_CONDITIONS);
const NUM_CONDITIONS: usize = 500;
const TIMING_REPS: usize = 300;

fn make_list(a: &mut Allocator, items: &[NodePtr]) -> NodePtr {
    let mut list = a.nil();
    for item in items.iter().rev() {
        list = a.new_pair(*item, list).unwrap();
    }
    list
}

fn hash_two(a: &[u8], b: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(a);
    hasher.update(b);
    hasher.finalize()
}

fn compute_coin_id(parent: &[u8], puzzle: &[u8], amount_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(parent);
    hasher.update(puzzle);
    hasher.update(amount_bytes);
    hasher.finalize()
}

fn clone_atom(a: &mut Allocator, arg: NodePtr) -> NodePtr {
    let bytes = a.atom(arg).as_ref().to_vec();
    a.new_atom(&bytes).unwrap()
}

fn make_condition(a: &mut Allocator, opcode: ConditionOpcode, args: &[NodePtr]) -> NodePtr {
    let opcode_node = a.new_small_number(opcode as u32).unwrap();
    let rest = make_list(a, args);
    a.new_pair(opcode_node, rest).unwrap()
}

/// Return the SpendId argument nodes for a 3-bit mode describing a target coin.
fn spend_id_fields(
    a: &mut Allocator,
    mode_bits: u8,
    parent: &[u8],
    puzzle: &[u8],
    amount_bytes: &[u8],
) -> Vec<NodePtr> {
    if mode_bits == 0b111 {
        let cid = compute_coin_id(parent, puzzle, amount_bytes);
        vec![a.new_atom(&cid).unwrap()]
    } else {
        let mut fields = Vec::new();
        if (mode_bits & 0b100) != 0 {
            fields.push(a.new_atom(parent).unwrap());
        }
        if (mode_bits & 0b010) != 0 {
            fields.push(a.new_atom(puzzle).unwrap());
        }
        if (mode_bits & 0b001) != 0 {
            fields.push(a.new_atom(amount_bytes).unwrap());
        }
        fields
    }
}

fn field_name(bits: u8) -> &'static str {
    match bits {
        0 => "00",
        1 => "am",
        2 => "pu",
        3 => "pa",
        4 => "PA",
        5 => "Pa",
        6 => "Pp",
        7 => "id",
        _ => unreachable!(),
    }
}

fn mode_name(mode: u8) -> String {
    let dst = mode & 0b111;
    let src = (mode >> 3) & 0b111;
    format!("{}->{}", field_name(dst), field_name(src))
}

/// Build NUM_CONDITIONS instances of a simple (non-paired) condition,
/// each with freshly allocated argument nodes.
fn build_simple_conditions(a: &mut Allocator, cond: &ConditionTest<'_>) -> (NodePtr, usize) {
    let mut conditions = a.nil();
    for _ in 0..NUM_CONDITIONS {
        let fresh_args: Vec<NodePtr> = cond.args.iter().map(|&arg| clone_atom(a, arg)).collect();
        let node = make_condition(a, cond.opcode, &fresh_args);
        conditions = a.new_pair(node, conditions).unwrap();
    }
    (conditions, NUM_CONDITIONS)
}

/// Build NUM_CONDITIONS CREATE_COIN_ANNOUNCEMENT with unique messages,
/// plus a single ASSERT matching the first one to force all announcements
/// to be hashed.
fn build_create_coin_announcements(
    a: &mut Allocator,
    coin_id_bytes: &[u8; 32],
) -> (NodePtr, usize) {
    let mut conditions = a.nil();
    let first_msg = [0u8; 32];
    let announcement_id = hash_two(coin_id_bytes, &first_msg);
    let id_node = a.new_atom(&announcement_id).unwrap();
    let assert = make_condition(a, opcodes::ASSERT_COIN_ANNOUNCEMENT, &[id_node]);
    conditions = a.new_pair(assert, conditions).unwrap();

    for i in (0..NUM_CONDITIONS).rev() {
        let mut msg_buf = [0u8; 32];
        msg_buf[24..32].copy_from_slice(&(i as u64).to_be_bytes());
        let msg_node = a.new_atom(&msg_buf).unwrap();
        let create = make_condition(a, opcodes::CREATE_COIN_ANNOUNCEMENT, &[msg_node]);
        conditions = a.new_pair(create, conditions).unwrap();
    }
    (conditions, NUM_CONDITIONS + 1)
}

/// Build NUM_CONDITIONS CREATE_COIN_ANNOUNCEMENT + ASSERT_COIN_ANNOUNCEMENT pairs,
/// each with a unique message and matching announcement hash.
fn build_assert_coin_announcements(
    a: &mut Allocator,
    coin_id_bytes: &[u8; 32],
) -> (NodePtr, usize) {
    let mut conditions = a.nil();
    for i in (0..NUM_CONDITIONS).rev() {
        let mut msg_buf = [0u8; 32];
        msg_buf[24..32].copy_from_slice(&(i as u64).to_be_bytes());
        let announcement_id = hash_two(coin_id_bytes, &msg_buf);

        let msg_node = a.new_atom(&msg_buf).unwrap();
        let create = make_condition(a, opcodes::CREATE_COIN_ANNOUNCEMENT, &[msg_node]);

        let id_node = a.new_atom(&announcement_id).unwrap();
        let assert = make_condition(a, opcodes::ASSERT_COIN_ANNOUNCEMENT, &[id_node]);

        conditions = a.new_pair(assert, conditions).unwrap();
        conditions = a.new_pair(create, conditions).unwrap();
    }
    (conditions, NUM_CONDITIONS * 2)
}

/// Build NUM_CONDITIONS CREATE_PUZZLE_ANNOUNCEMENT with unique messages,
/// plus a single ASSERT matching the first one to force all announcements
/// to be hashed.
fn build_create_puzzle_announcements(a: &mut Allocator, puzzle_hash: &[u8]) -> (NodePtr, usize) {
    let mut conditions = a.nil();
    let first_msg = [0u8; 32];
    let announcement_id = hash_two(puzzle_hash, &first_msg);
    let id_node = a.new_atom(&announcement_id).unwrap();
    let assert = make_condition(a, opcodes::ASSERT_PUZZLE_ANNOUNCEMENT, &[id_node]);
    conditions = a.new_pair(assert, conditions).unwrap();

    for i in (0..NUM_CONDITIONS).rev() {
        let mut msg_buf = [0u8; 32];
        msg_buf[24..32].copy_from_slice(&(i as u64).to_be_bytes());
        let msg_node = a.new_atom(&msg_buf).unwrap();
        let create = make_condition(a, opcodes::CREATE_PUZZLE_ANNOUNCEMENT, &[msg_node]);
        conditions = a.new_pair(create, conditions).unwrap();
    }
    (conditions, NUM_CONDITIONS + 1)
}

/// Build NUM_CONDITIONS CREATE_PUZZLE_ANNOUNCEMENT + ASSERT_PUZZLE_ANNOUNCEMENT pairs,
/// each with a unique message and matching announcement hash.
fn build_assert_puzzle_announcements(a: &mut Allocator, puzzle_hash: &[u8]) -> (NodePtr, usize) {
    let mut conditions = a.nil();
    for i in (0..NUM_CONDITIONS).rev() {
        let mut msg_buf = [0u8; 32];
        msg_buf[24..32].copy_from_slice(&(i as u64).to_be_bytes());
        let announcement_id = hash_two(puzzle_hash, &msg_buf);

        let msg_node = a.new_atom(&msg_buf).unwrap();
        let create = make_condition(a, opcodes::CREATE_PUZZLE_ANNOUNCEMENT, &[msg_node]);

        let id_node = a.new_atom(&announcement_id).unwrap();
        let assert = make_condition(a, opcodes::ASSERT_PUZZLE_ANNOUNCEMENT, &[id_node]);

        conditions = a.new_pair(assert, conditions).unwrap();
        conditions = a.new_pair(create, conditions).unwrap();
    }
    (conditions, NUM_CONDITIONS * 2)
}

/// Build two spends where spend 1 has NUM_CONDITIONS ASSERT_CONCURRENT_SPEND
/// referencing spend 2's coin ID.
fn build_assert_concurrent_spend(a: &mut Allocator) -> (NodePtr, usize) {
    let amount_node = a.new_small_number(AMOUNT as u32).unwrap();
    let coin2_id = compute_coin_id(H2, H1, &a.atom(amount_node).as_ref().to_vec());

    let parent1 = a.new_atom(H1).unwrap();
    let puzzle1 = a.new_atom(H2).unwrap();
    let parent2 = a.new_atom(H2).unwrap();
    let puzzle2 = a.new_atom(H1).unwrap();

    let mut conditions1 = a.nil();
    for _ in 0..NUM_CONDITIONS {
        let id_node = a.new_atom(&coin2_id).unwrap();
        let cond = make_condition(a, opcodes::ASSERT_CONCURRENT_SPEND, &[id_node]);
        conditions1 = a.new_pair(cond, conditions1).unwrap();
    }

    let remark = make_condition(a, opcodes::REMARK, &[]);
    let conditions2 = a.new_pair(remark, a.nil()).unwrap();

    let spend1 = make_list(a, &[parent1, puzzle1, amount_node, conditions1]);
    let spend2 = make_list(a, &[parent2, puzzle2, amount_node, conditions2]);

    let nil = a.nil();
    let list = a.new_pair(spend2, nil).unwrap();
    let list = a.new_pair(spend1, list).unwrap();
    let spends = a.new_pair(list, nil).unwrap();
    (spends, NUM_CONDITIONS)
}

/// Build two spends where spend 1 has NUM_CONDITIONS ASSERT_CONCURRENT_PUZZLE
/// referencing spend 2's puzzle hash.
fn build_assert_concurrent_puzzle(a: &mut Allocator) -> (NodePtr, usize) {
    let amount_node = a.new_small_number(AMOUNT as u32).unwrap();

    let parent1 = a.new_atom(H1).unwrap();
    let puzzle1 = a.new_atom(H2).unwrap();
    let parent2 = a.new_atom(H2).unwrap();
    let puzzle2 = a.new_atom(H1).unwrap();

    let mut conditions1 = a.nil();
    for _ in 0..NUM_CONDITIONS {
        let ph_node = a.new_atom(H1).unwrap();
        let cond = make_condition(a, opcodes::ASSERT_CONCURRENT_PUZZLE, &[ph_node]);
        conditions1 = a.new_pair(cond, conditions1).unwrap();
    }

    let remark = make_condition(a, opcodes::REMARK, &[]);
    let conditions2 = a.new_pair(remark, a.nil()).unwrap();

    let spend1 = make_list(a, &[parent1, puzzle1, amount_node, conditions1]);
    let spend2 = make_list(a, &[parent2, puzzle2, amount_node, conditions2]);

    let nil = a.nil();
    let list = a.new_pair(spend2, nil).unwrap();
    let list = a.new_pair(spend1, list).unwrap();
    let spends = a.new_pair(list, nil).unwrap();
    (spends, NUM_CONDITIONS)
}

/// Build two spends where spend 1 creates a coin (via CREATE_COIN) and
/// spend 2 is that ephemeral coin with NUM_CONDITIONS ASSERT_EPHEMERAL.
fn build_assert_ephemeral(a: &mut Allocator) -> (NodePtr, usize) {
    let amount_node = a.new_small_number(AMOUNT as u32).unwrap();

    let parent1 = a.new_atom(H1).unwrap();
    let puzzle1 = a.new_atom(H2).unwrap();

    // Spend 1 creates coin 2 via CREATE_COIN with puzzle H1 and amount AMOUNT
    let create_ph = a.new_atom(H1).unwrap();
    let create_amt = a.new_small_number(AMOUNT as u32).unwrap();
    let create_coin = make_condition(a, opcodes::CREATE_COIN, &[create_ph, create_amt]);
    let conditions1 = a.new_pair(create_coin, a.nil()).unwrap();

    // Coin 2 is ephemeral: parent is coin 1's ID, puzzle is H1, amount is AMOUNT
    let amount_bytes = a.atom(amount_node).as_ref().to_vec();
    let coin1_id = compute_coin_id(H1, H2, &amount_bytes);
    let parent2 = a.new_atom(&coin1_id).unwrap();
    let puzzle2 = a.new_atom(H1).unwrap();

    let mut conditions2 = a.nil();
    for _ in 0..NUM_CONDITIONS {
        let cond = make_condition(a, opcodes::ASSERT_EPHEMERAL, &[]);
        conditions2 = a.new_pair(cond, conditions2).unwrap();
    }

    let spend1 = make_list(a, &[parent1, puzzle1, amount_node, conditions1]);
    let spend2 = make_list(a, &[parent2, puzzle2, amount_node, conditions2]);

    let nil = a.nil();
    let list = a.new_pair(spend2, nil).unwrap();
    let list = a.new_pair(spend1, list).unwrap();
    let spends = a.new_pair(list, nil).unwrap();
    (spends, NUM_CONDITIONS)
}

/// Build a two-spend structure with NUM_CONDITIONS SEND/RECEIVE pairs.
/// Coin 1 (H1/H2) sends, coin 2 (H2/H1) receives.
fn build_message_spends(a: &mut Allocator, mode: u8) -> (NodePtr, usize) {
    let send_dst_mode = mode & 0b111;
    let recv_src_mode = (mode >> 3) & 0b111;

    let amount_node = a.new_small_number(AMOUNT as u32).unwrap();
    let amount_bytes: Vec<u8> = a.atom(amount_node).as_ref().to_vec();

    let parent1_node = a.new_atom(H1).unwrap();
    let puzzle1_node = a.new_atom(H2).unwrap();
    let parent2_node = a.new_atom(H2).unwrap();
    let puzzle2_node = a.new_atom(H1).unwrap();

    let mut send_conditions = a.nil();
    let mut recv_conditions = a.nil();

    for i in (0..NUM_CONDITIONS).rev() {
        let mut msg_buf = [0u8; 32];
        msg_buf[24..32].copy_from_slice(&(i as u64).to_be_bytes());

        // SEND on coin 1, destination fields describe coin 2
        let s_mode = a.new_small_number(mode as u32).unwrap();
        let s_msg = a.new_atom(&msg_buf).unwrap();
        let dst_fields = spend_id_fields(a, send_dst_mode, H2, H1, &amount_bytes);
        let mut send_args = vec![s_mode, s_msg];
        send_args.extend(dst_fields);
        let send = make_condition(a, opcodes::SEND_MESSAGE, &send_args);

        // RECEIVE on coin 2, source fields describe coin 1
        let r_mode = a.new_small_number(mode as u32).unwrap();
        let r_msg = a.new_atom(&msg_buf).unwrap();
        let src_fields = spend_id_fields(a, recv_src_mode, H1, H2, &amount_bytes);
        let mut recv_args = vec![r_mode, r_msg];
        recv_args.extend(src_fields);
        let recv = make_condition(a, opcodes::RECEIVE_MESSAGE, &recv_args);

        send_conditions = a.new_pair(send, send_conditions).unwrap();
        recv_conditions = a.new_pair(recv, recv_conditions).unwrap();
    }

    let spend1 = make_list(
        a,
        &[parent1_node, puzzle1_node, amount_node, send_conditions],
    );
    let spend2 = make_list(
        a,
        &[parent2_node, puzzle2_node, amount_node, recv_conditions],
    );

    let nil = a.nil();
    let spends_list = a.new_pair(spend2, nil).unwrap();
    let spends_list = a.new_pair(spend1, spends_list).unwrap();
    let spends = a.new_pair(spends_list, nil).unwrap();

    (spends, NUM_CONDITIONS * 2)
}

fn run_benchmark(
    allocator: &Allocator,
    spends: NodePtr,
    num_total: usize,
) -> (Vec<f64>, f64, f64, u64) {
    // warmup
    let result = parse_spends::<EmptyVisitor>(
        allocator,
        spends,
        11_000_000_000,
        0,
        FLAGS,
        &Signature::default(),
        None,
        &TEST_CONSTANTS,
    )
    .expect("parse_spends");
    let condition_cost = result.condition_cost;

    let mut samples = Vec::<f64>::new();
    for _ in 0..TIMING_REPS {
        let start = Instant::now();
        parse_spends::<EmptyVisitor>(
            allocator,
            spends,
            11_000_000_000,
            0,
            FLAGS,
            &Signature::default(),
            None,
            &TEST_CONSTANTS,
        )
        .expect("parse_spends");
        let elapsed = start.elapsed();
        samples.push(elapsed.as_nanos() as f64 / num_total as f64);
    }

    let avg = samples.iter().sum::<f64>() / samples.len() as f64;
    let mut sorted_samples = samples.clone();
    sorted_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = if sorted_samples.len() % 2 == 0 {
        (sorted_samples[sorted_samples.len() / 2 - 1] + sorted_samples[sorted_samples.len() / 2])
            / 2.0
    } else {
        sorted_samples[sorted_samples.len() / 2]
    };

    (samples, avg, median, condition_cost)
}

fn write_samples(samples: &[f64], path: &str) {
    let mut file = fs::File::create(path).expect("create data file");
    writeln!(file, "# nanos_per_condition").unwrap();
    for ns in samples {
        writeln!(file, "{ns:.3}").unwrap();
    }
}

pub fn main() {
    let mut allocator = Allocator::new();
    let one = allocator.new_small_number(1).expect("number");
    let hundred = allocator.new_small_number(AMOUNT as u32).expect("number");
    let sk = SecretKey::from_bytes(SECRET_KEY).expect("secret key");
    let pk = sk.public_key();
    let parent_id = allocator.new_atom(H1).expect("atom");
    let puzzle_hash = Bytes32::from(clvm_utils::tree_hash_from_bytes(&[1_u8]).expect("tree_hash"));
    let puz_hash_node_ptr = allocator.new_atom(puzzle_hash.as_slice()).expect("bytes");
    let coin = Coin {
        parent_coin_info: H1.into(),
        puzzle_hash,
        amount: AMOUNT,
    };
    let coin_id = allocator.new_atom(coin.coin_id().as_slice()).expect("atom");
    let coin_id_bytes: [u8; 32] = coin.coin_id().into();
    let h1_pointer = allocator.new_atom(H1).expect("atom");
    let pk_ptr = allocator.new_atom(&pk.to_bytes()).expect("pubkey");
    let msg_ptr = allocator.new_atom(&[3u8; 13]).expect("msg");

    let cond_tests = [
        ConditionTest {
            opcode: opcodes::AGG_SIG_UNSAFE,
            args: &[pk_ptr, msg_ptr],
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_ME,
            args: &[pk_ptr, msg_ptr],
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_PARENT,
            args: &[pk_ptr, msg_ptr],
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_PUZZLE,
            args: &[pk_ptr, msg_ptr],
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_AMOUNT,
            args: &[pk_ptr, msg_ptr],
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_PARENT_AMOUNT,
            args: &[pk_ptr, msg_ptr],
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_PARENT_PUZZLE,
            args: &[pk_ptr, msg_ptr],
        },
        ConditionTest {
            opcode: opcodes::AGG_SIG_PUZZLE_AMOUNT,
            args: &[pk_ptr, msg_ptr],
        },
        ConditionTest {
            opcode: opcodes::REMARK,
            args: &[h1_pointer],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_COIN_ID,
            args: &[coin_id],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_PARENT_ID,
            args: &[h1_pointer],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_PUZZLEHASH,
            args: &[puz_hash_node_ptr],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_AMOUNT,
            args: &[hundred],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_BIRTH_HEIGHT,
            args: &[hundred],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_MY_BIRTH_SECONDS,
            args: &[hundred],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_SECONDS_RELATIVE,
            args: &[one],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_SECONDS_ABSOLUTE,
            args: &[one],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_HEIGHT_RELATIVE,
            args: &[one],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_HEIGHT_ABSOLUTE,
            args: &[one],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_SECONDS_RELATIVE,
            args: &[one],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_SECONDS_ABSOLUTE,
            args: &[one],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_HEIGHT_RELATIVE,
            args: &[one],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_BEFORE_HEIGHT_ABSOLUTE,
            args: &[one],
        },
        ConditionTest {
            opcode: opcodes::SOFTFORK,
            args: &[hundred, h1_pointer],
        },
        ConditionTest {
            opcode: opcodes::CREATE_COIN_ANNOUNCEMENT,
            args: &[],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_COIN_ANNOUNCEMENT,
            args: &[],
        },
        ConditionTest {
            opcode: opcodes::CREATE_PUZZLE_ANNOUNCEMENT,
            args: &[],
        },
        ConditionTest {
            opcode: opcodes::ASSERT_PUZZLE_ANNOUNCEMENT,
            args: &[],
        },
    ];

    fs::create_dir_all("data").expect("create data directory");

    for cond in cond_tests {
        let cp = allocator.checkpoint();

        let (conditions, num_total) = match cond.opcode {
            opcodes::CREATE_COIN_ANNOUNCEMENT => {
                build_create_coin_announcements(&mut allocator, &coin_id_bytes)
            }
            opcodes::ASSERT_COIN_ANNOUNCEMENT => {
                build_assert_coin_announcements(&mut allocator, &coin_id_bytes)
            }
            opcodes::CREATE_PUZZLE_ANNOUNCEMENT => {
                build_create_puzzle_announcements(&mut allocator, puzzle_hash.as_slice())
            }
            opcodes::ASSERT_PUZZLE_ANNOUNCEMENT => {
                build_assert_puzzle_announcements(&mut allocator, puzzle_hash.as_slice())
            }
            _ => build_simple_conditions(&mut allocator, &cond),
        };

        let spend = make_list(
            &mut allocator,
            &[parent_id, puz_hash_node_ptr, hundred, conditions],
        );
        let nil = allocator.nil();
        let spends_list = allocator.new_pair(spend, nil).unwrap();
        let spends = allocator.new_pair(spends_list, nil).unwrap();

        let (samples, avg, median, condition_cost) = run_benchmark(&allocator, spends, num_total);
        let label = opcode_name(cond.opcode);
        if condition_cost > 0 {
            let ns_per_cost = median * num_total as f64 / condition_cost as f64;
            println!(
                "{label:<33} avg: {avg:8.0} ns  median: {median:8.0} ns  ns/cost: {ns_per_cost:.3}",
            );
        } else {
            println!("{label:<33} avg: {avg:8.0} ns  median: {median:8.0} ns",);
        }

        let path = format!("data/{label}.dat");
        write_samples(&samples, &path);

        allocator.restore_checkpoint(&cp);
    }

    // Benchmark conditions that require two spends.
    let two_spend_tests: &[(ConditionOpcode, fn(&mut Allocator) -> (NodePtr, usize))] = &[
        (
            opcodes::ASSERT_CONCURRENT_SPEND,
            build_assert_concurrent_spend,
        ),
        (
            opcodes::ASSERT_CONCURRENT_PUZZLE,
            build_assert_concurrent_puzzle,
        ),
        (opcodes::ASSERT_EPHEMERAL, build_assert_ephemeral),
    ];

    for &(opcode, builder) in two_spend_tests {
        let cp = allocator.checkpoint();

        let (spends, num_total) = builder(&mut allocator);

        let (samples, avg, median, condition_cost) = run_benchmark(&allocator, spends, num_total);
        let label = opcode_name(opcode);
        if condition_cost > 0 {
            let ns_per_cost = median * num_total as f64 / condition_cost as f64;
            println!(
                "{label:<33} avg: {avg:8.0} ns  median: {median:8.0} ns  ns/cost: {ns_per_cost:.3}",
            );
        } else {
            println!("{label:<33} avg: {avg:8.0} ns  median: {median:8.0} ns",);
        }

        let path = format!("data/{label}.dat");
        write_samples(&samples, &path);

        allocator.restore_checkpoint(&cp);
    }

    // Benchmark every SEND_MESSAGE/RECEIVE_MESSAGE mode combination.
    // Mode is 6 bits: lower 3 = destination id, upper 3 = source id.
    // Uses two spends: coin 1 (H1/H2) sends, coin 2 (H2/H1) receives.
    for mode in 0u8..=63 {
        let cp = allocator.checkpoint();

        let (spends, num_total) = build_message_spends(&mut allocator, mode);

        let name = mode_name(mode);
        let (samples, avg, median, condition_cost) = run_benchmark(&allocator, spends, num_total);
        if condition_cost > 0 {
            let ns_per_cost = median * num_total as f64 / condition_cost as f64;
            println!(
                "Message 0x{mode:02x} {name:20} avg: {avg:8.0} ns  median: {median:8.0} ns  ns/cost: {ns_per_cost:.3}",
            );
        } else {
            println!("Message 0x{mode:02x} {name:20} avg: {avg:8.0} ns  median: {median:8.0} ns",);
        }

        let path = format!("data/SendMessage_0x{mode:02x}.dat");
        write_samples(&samples, &path);

        allocator.restore_checkpoint(&cp);
    }
}
