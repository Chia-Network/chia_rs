from chia_rs import MerkleBlob
from chia_rs.sized_bytes import bytes32
from chia_rs.sized_ints import uint64


def test_merkle_blob():
    blob = bytes.fromhex(
        "0001ffffffff00000001000000020c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000000000405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b0100000000001415161718191a1b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b"
    )
    merkle_blob = MerkleBlob(blob)
    print(merkle_blob)
    print(dir(merkle_blob))
    assert len(merkle_blob) == len(blob)


def test_just_insert_a_bunch() -> None:
    HASH = bytes32(
        [
            12,
            13,
            14,
            15,
            16,
            17,
            18,
            19,
            20,
            21,
            22,
            23,
            24,
            25,
            26,
            27,
            28,
            29,
            30,
            31,
            32,
            33,
            34,
            35,
            36,
            37,
            38,
            39,
            40,
            41,
            42,
            43,
        ]
    )

    import pathlib

    path = pathlib.Path("~/tmp/mbt/").expanduser()
    path.joinpath("py").mkdir(parents=True, exist_ok=True)
    path.joinpath("rs").mkdir(parents=True, exist_ok=True)

    merkle_blob = MerkleBlob(blob=bytearray())
    import time

    total_time = 0.0
    for i in range(100000):
        start = time.monotonic()
        merkle_blob.insert(uint64(i), HASH)
        end = time.monotonic()
        total_time += end - start

        # kv_count = i + 1
        # if kv_count == 2:
        #     assert len(merkle_blob.blob) == 3 * spacing
        # elif kv_count == 3:
        #     assert len(merkle_blob.blob) == 5 * spacing
        #
        # with path.joinpath("py", f"{i:04}").open(mode="w") as file:
        #     for offset in range(0, len(merkle_blob.blob), spacing):
        #         file.write(merkle_blob.blob[offset:offset + spacing].hex())
        #         file.write("\n")
        # path.joinpath("py", f"{i:04}").write_bytes(merkle_blob.blob)

    # rs = pathlib.Path("~/repos/chia_rs/crates/chia-datalayer/src/test_just_insert_a_bunch_reference").expanduser().read_bytes()
    # b = bytes(merkle_blob.blob)
    # assert b == rs, 'not the same'

    # assert False, f"total time: {total_time}"
