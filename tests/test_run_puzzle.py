from chia_rs import run_puzzle, run_chia_program
from hashlib import sha256
import pytest
from run_gen import print_spend_bundle_conditions
from clvm.SExp import SExp
from clvm.casts import int_from_bytes
from clvm_tools import binutils
import os

def test_block_834752() -> None:
    block = bytes.fromhex(open(f"{os.path.dirname(__file__)}/generators/block-834752.hex").read())
    cost, ret = run_chia_program(block, b"\xff\x80\x80", 11000000000, 0)
    ret = ret.pair[0]
    puzzles = []

    while ret.pair is not None:
        spend = ret.pair[0]
        parent = spend.pair[0].atom
        spend = spend.pair[1]

        puzzle = SExp.to(spend.pair[0])
        spend = spend.pair[1]

        amount = int_from_bytes(spend.pair[0].atom)
        spend = spend.pair[1]

        solution = SExp.to(spend.pair[0])
        spend = spend.pair[1]

        puzzles.append((parent, amount, puzzle.as_bin(), solution.as_bin()))
        ret = ret.pair[1]

    output = ""
    for parent, amount, puzzle, solution in puzzles:
        conds = run_puzzle(puzzle, solution, parent, amount, 11000000000, 0)
        output += print_spend_bundle_conditions(conds)

    assert output == """\
SPENDS:
- coin id: afd297097757a8f5a3f3266933a6c29a7674c71028825562e7e4cac02b9228f6 ph: b78c1c1c0fe082b9c7f18d7e7c716b1607fd62dbdf3eef18f79e2717789ac55f
  CREATE_COIN: ph: dca1429bcffab70d2218df91683ebe292305925337c0fffa91d5244838ffbd80 amount: 1
  AGG_SIG_ME pk: 8116639d853ecd6109277a9d83d3acc7e53a18d3524262ec9b99df923d22a390cbf0f632bced556dd9886bbf53f444b6 msg: d497b66589f5d8bdc0631678cc488e25360d114eee51b84a3c1685771a7daa2d
cost: 3729870
removal_amount: 1
addition_amount: 1
SPENDS:
- coin id: 8522228562997c720038e6dca3721c36b054d766e0de80087ae9d7b4b229df29 ph: 1dc30429a17e6ee7d5b18a795da6bf838b7c87d0cf65cfb000fca5bab1ccbe19
  CREATE_COIN: ph: c30e2a348a6a4f56fc8d4ce44cebda06f72420d68e454dd765ed40b782fca307 amount: 1
  AGG_SIG_ME pk: 8a505367d099210c24f1be8945a83de7db6d9396a9742c8f609aebf7ba56bbaacb038819b122741211bbc9227c903573 msg: 6cbb9e5def3ed896de9651b54145afc27bcbba9c61ca853b1e3bf0e6b8a78e9a
cost: 3729870
removal_amount: 1
addition_amount: 1
SPENDS:
- coin id: b4fa0835dd9d595cf3d3c5e574ce081c7cca37452c4c336caaf8fbf644c5b268 ph: 0ac6539fede8c4cd50610af58f82b8cc74c774577f0a098db8ab2f42e5e90a9f
  CREATE_COIN: ph: 948a5a1b3367d80aebc23a7ae772b140b8626f908d5b921f9980a0f01280e3c8 amount: 1
  AGG_SIG_ME pk: ab6824901d856c5a8c1664c990d1ef94a19ed7b4ab28a6b8e064f1a11e07f5e75bdb6ff8242f517534df36ae03c81da0 msg: 38442a894ff28abb1225d3698c2e09aaea12827c9141f9e4c85267a38cec3e6f
cost: 3729868
removal_amount: 1
addition_amount: 1
SPENDS:
- coin id: 3c3412900b156403b13bb4191f1d6818619f73c97337829a4f821012b24d88eb ph: bc0e759db02410acb193d0c1c0a6841a2a821c9322570e2f23dfe220d9e6ae8f
  ASSERT_HEIGHT_RELATIVE 32
  CREATE_COIN: ph: 013beb3fa36f6d3f221253f7ef380e4d1197246b57b453ce32d339e9be4b2eec amount: 1
  AGG_SIG_ME pk: afdbc8d2811665196a20931b06ffe981a2ec64aebd2d917478bf8441d77cb2b62f96194277d91983c5ca9edf0a17fdcc msg: f19e77711df9e26a0e1fda2279bd73d6dcb9afe91dafadb4c3c20b1fe441a1b3
cost: 3792779
removal_amount: 1
addition_amount: 1
SPENDS:
- coin id: a66d7b064c9fdfbcffe0755766c1a5d66899fab9f9a6cb4f93d614d676bc8292 ph: ba5089cb215c3f37dbc5718820d16d454694869e756653a516d2abb3faacd843
  ASSERT_HEIGHT_RELATIVE 32
  CREATE_COIN: ph: 87c64d9ef085869b1bf272816a752d093e272fa69d6840181cd48fa8eb86dcc3 amount: 1
  AGG_SIG_ME pk: 8e2ab4bd0f4b65f0e6e1cc1f54fd3a953a36afc98ec25a741958bf1f19d0a416f2b39b89d4bd9870f11d6bf09030780e msg: 01f207c53eb38d9a0ca38981ddedf93c9d62ac0073f5706c1d513cc109206a00
cost: 3792843
removal_amount: 1
addition_amount: 1
SPENDS:
- coin id: ed418e8b3f49d86a0ab42343e1dc864f796026b8646c953a3bd54329a3843c1f ph: 87864b58be87e5f0fe566955088597ea0f5aafaad1ce0ed8dd02f5c84d257aa6
  ASSERT_HEIGHT_RELATIVE 32
  CREATE_COIN: ph: 70aea9db5a7c74a2dfa632ddac0e7cd3283c6439c1feae0417645b6392104ded amount: 1
  AGG_SIG_ME pk: a5383a3b9a6c1a94a85e4f982e1fa3af2c99087e5f6df8b887d30c109f71043671683a1ae985d7d874fbe07dfa6d88b7 msg: 5d53d6baf98f40ce52d588135fa97fc7c6756c6fb6315298902d631548704d8c
cost: 3792835
removal_amount: 1
addition_amount: 1
"""

def test_failure() -> None:

    output = ""
    parent = b"1" * 32
    amount = 1337
    # (mod (solution) (if (= solution (q . 1)) () (x solution)))
    puzzle = binutils.assemble("(a (i (= 2 (q . 1)) () (q 8 2)) 1)").as_bin()
    solution1 = binutils.assemble("(1)").as_bin()
    solution2 = binutils.assemble("(2)").as_bin()

    # the puzzle expects (1)
    conds = run_puzzle(puzzle, solution1, parent, amount, 11000000000, 0)
    output += print_spend_bundle_conditions(conds)
    print(output)
    assert output == """\
SPENDS:
- coin id: 7767e945d8b73704d3ed84277b3df4572cec7d418629dc2f0325385e708c7724 ph: 51f3dbcff4ada9fe7030d6c017f243b903f9baf503e2d0590b3b78b5c2589674
cost: 465
removal_amount: 1337
addition_amount: 0
"""

    with pytest.raises(ValueError, match="ValidationError"):
        # the puzzle does not expect (2)
        run_puzzle(puzzle, solution2, parent, amount, 11000000000, 0)
