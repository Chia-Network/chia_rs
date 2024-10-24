from chia_rs import MerkleBlob
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import int64


def test_merkle_blob():
    blob = bytes.fromhex(
        "00000e4a8b1ecee43f457bbe2b30e94ac2afc0d3a6536f891a2ced5e96ce07fe9932ffffffff000000010000000200000000000000000100d8ddfc94e7201527a6a93ee04aed8c5c122ac38af6dbf6e5f1caefba2597230d000000000001020304050607101112131415161701000f980325ebe9426fa295f3f69cc38ef8fe6ce8f3b9f083556c0f927e67e566510000000020212223242526273031323334353637"
    )
    merkle_blob = MerkleBlob(blob)
    print(merkle_blob)
    print(dir(merkle_blob))
    assert len(merkle_blob) == len(blob)


def test_just_insert_a_bunch() -> None:
    HASH = bytes32(range(12, 44))

    import pathlib

    path = pathlib.Path("~/tmp/mbt/").expanduser()
    path.joinpath("py").mkdir(parents=True, exist_ok=True)
    path.joinpath("rs").mkdir(parents=True, exist_ok=True)

    merkle_blob = MerkleBlob(blob=bytearray())
    import time

    total_time = 0.0
    for i in range(100000):
        start = time.monotonic()
        merkle_blob.insert(int64(i), int64(i), HASH)
        end = time.monotonic()
        total_time += end - start
