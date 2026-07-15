"""Interop tests: chia_vdf_verify (Rust) vs chiavdf (C++).

Skipped automatically when chiavdf is not installed.  To run:

    # In a venv that has both wheels (e.g. the chia-blockchain venv after
    # pip-installing chia-vdf-verify into it):
    /path/to/.venv/bin/pytest tests/test_interop.py -v
"""

import pytest

chiavdf = pytest.importorskip("chiavdf", reason="chiavdf C++ wheel not installed")

from chiavdf import verify_n_wesolowski as cpp_verify  # noqa: E402
from chia_vdf_verify import create_discriminant  # noqa: E402
from chia_vdf_verify import verify_n_wesolowski as rust_verify  # noqa: E402

# ---------------------------------------------------------------------------
# Test vectors — identical to the constants in tests/integration_tests.rs
# ---------------------------------------------------------------------------

DISC_BITS = 512

SEED_1 = b"test_seed_chia"
ITERS_1 = 100
X_S_HEX = "08000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
PROOF_BLOB_1_HEX = "020020417eb39c4e14954a817af644fc13d086c26dddab8afea12415b5e685f7883f5740ba01cb75220081c8aba7854cbd52010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"

SEED_2 = b"chia-vdf-rust"
ITERS_2 = 200
# D_BYTES2_HEX is the big-endian magnitude; discriminant = -int(hex, 16)
D_BYTES2_HEX = "c3ef34d02017540ef26d88057bbfc778da12ed572b99f8707834ed344577c210b1f9287f54a536913177bf5880a4a51b6bfa42445f3fbcd082b695e38c2066d7"
PROOF_BLOB_2_HEX = "030033205ea6d1ab367757073029f1462eb2fcc79749871d0b576f7a392adac84f56f46100e477d59353376f82a3eb56720d010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"

# Active BQFC bytes for a 512-bit discriminant with g_size=0:
#   2 header + 32 (a) + 16 (t) + 1 (g) + 1 (b0) = 52
# Bytes 52-99 of the y or pi form are zero padding.
BQFC_ACTIVE_512 = 52

# ---------------------------------------------------------------------------
# Mainnet test vector for the b0-field discrepancy (1024-bit discriminant).
# Extracted from mainnet block height=309155, label=cc_ip.
# The y form's b0 field (byte 99) = 0x01.  Flipping bits 2+ of b0 (e.g. ^0x04)
# causes chiavdf to return True while Rust returns False — chiavdf's C++ BQFC
# decoder appears to ignore the upper bits of b0 and recovers the same (a, b)
# regardless.  Rust's canonical round-trip check catches the inconsistency.
# ---------------------------------------------------------------------------
DISC_1024_STR = (
    "-146212091130374364448271598629912687111631974722846603227183769906935970876"
    "483871782840562162445571052154480975719448767769767557905129461524079902394"
    "315542354994269060181795718055043487735056120915916768273200138311940357886"
    "024014124174476991145983171370265799623472241486347111977874193600694306566"
    "545523111"
)
X_S_1024_HEX = "08" + "00" * 99
OUTPUT_1024_HEX = (
    "0300d8262c430e78e7c06cf60c9b2049968f604f3b506a85bfe4fff319f8176760e06cab8a"
    "b45524458bf558101f9b4ce8c23cc1e053263272b808b76c6f26493a113b62ded5707b28d9"
    "eedc0503ac2efcd32be670726725be0fa7ea01f0ef3f602502010000c3625953c111ba28de"
    "77d3e63846cc1063596d44cc8a2cd57a44a60c96b072ba8254485ece15a98b52bdd1907d13"
    "59e33929861be12346815a38e083872e5b03c75e2a4d48fdc787f44244f78769c44d186e84"
    "46daa45f22e4997f1ded3b96030703"
)
ITERS_1024 = 64670218


def _disc_str(seed: bytes, bits: int) -> str:
    return str(int(create_discriminant(seed, bits), 16))


@pytest.fixture(scope="module")
def disc1():
    return _disc_str(SEED_1, DISC_BITS)


@pytest.fixture(scope="module")
def disc2():
    # D_BYTES2_HEX is the big-endian magnitude; discriminant is negative
    return str(-int(D_BYTES2_HEX, 16))


@pytest.fixture(scope="module")
def x_s():
    return bytes.fromhex(X_S_HEX)


@pytest.fixture(scope="module")
def proof1():
    return bytes.fromhex(PROOF_BLOB_1_HEX)


@pytest.fixture(scope="module")
def proof2():
    return bytes.fromhex(PROOF_BLOB_2_HEX)


# ---------------------------------------------------------------------------
# Agreement on valid proofs
# ---------------------------------------------------------------------------


def test_valid_proof_both_accept(disc1, x_s, proof1):
    """Both verifiers must accept the known-good proof (seed=test_seed_chia, iters=100)."""
    assert rust_verify(disc1, x_s, proof1, ITERS_1, DISC_BITS, 0), "Rust rejected valid proof"
    assert cpp_verify(disc1, x_s, proof1, ITERS_1, DISC_BITS, 0), "chiavdf rejected valid proof"


def test_valid_proof2_both_accept(disc2, x_s, proof2):
    """Both verifiers must accept the second known-good proof (seed=chia-vdf-rust, iters=200)."""
    assert rust_verify(disc2, x_s, proof2, ITERS_2, DISC_BITS, 0), "Rust rejected valid proof 2"
    assert cpp_verify(disc2, x_s, proof2, ITERS_2, DISC_BITS, 0), "chiavdf rejected valid proof 2"


# ---------------------------------------------------------------------------
# Agreement on rejection
# ---------------------------------------------------------------------------


def _cpp_result(disc, x_s, proof, iters, disc_bits=DISC_BITS):
    """Call chiavdf, treating exceptions as False (chiavdf raises on malformed forms)."""
    try:
        return bool(cpp_verify(disc, x_s, proof, iters, disc_bits, 0))
    except Exception:
        return False


def _both_reject(disc, x_s, proof, iters, label):
    r = rust_verify(disc, x_s, proof, iters, DISC_BITS, 0)
    c = _cpp_result(disc, x_s, proof, iters)
    assert not r, f"Rust unexpectedly accepted {label}"
    assert not c, f"chiavdf unexpectedly accepted {label}"


def test_wrong_iters_both_reject(disc1, x_s, proof1):
    _both_reject(disc1, x_s, proof1, ITERS_1 + 1, "wrong iters")


def test_corrupted_y_both_reject(disc1, x_s, proof1):
    bad = bytearray(proof1)
    bad[5] ^= 0x01
    _both_reject(disc1, x_s, bytes(bad), ITERS_1, "corrupted y byte 5")


def test_corrupted_pi_flag_both_reject(disc1, x_s, proof1):
    # The pi form in this vector is IS_1 (byte 100 = 0x04), so bytes 101-199
    # are ignored by both decoders (IS_1 early return, no canonical check).
    # Flipping the flag byte itself (byte 100) breaks the IS_1 interpretation
    # and both verifiers reject the resulting malformed form.
    bad = bytearray(proof1)
    bad[100] ^= 0x04  # clear IS_1 flag -> misinterpreted as ordinary form
    _both_reject(disc1, x_s, bytes(bad), ITERS_1, "corrupted pi flag byte")


def test_wrong_disc_both_reject(disc2, x_s, proof1):
    """A proof generated under disc1 should fail verification against disc2."""
    _both_reject(disc2, x_s, proof1, ITERS_1, "proof under wrong discriminant")


# ---------------------------------------------------------------------------
# The canonical-check discrepancy (documented in tests/integration_tests.rs)
#
# For a 512-bit discriminant the active BQFC region is 52 bytes; bytes 52-99
# of the y form are zero padding.  Rust re-serializes the decoded form and
# checks it matches byte-for-byte, so any non-zero padding byte is rejected.
# chiavdf's C++ decoder does not perform this check and accepts the blob.
# ---------------------------------------------------------------------------


def test_b0_upper_bits_rust_rejects():
    """
    Rust rejects a mutation to the upper bits of b0 (y-form byte 99, 1024-bit disc).
    Mainnet height=309155, cc_ip — the exact case found by differential fuzzing.
    """
    x_s = bytes.fromhex(X_S_1024_HEX)
    output = bytes.fromhex(OUTPUT_1024_HEX)
    bad = bytearray(output)
    bad[99] ^= 0x04  # flip bits 2+ of b0; original value = 0x01

    assert not rust_verify(DISC_1024_STR, x_s, bytes(bad), ITERS_1024, 1024, 0), \
        "Rust should reject: canonical check catches changed b0"


def test_b0_upper_bits_chiavdf_should_also_reject():
    """
    chiavdf 1.1.14 BUG: accepts a proof with corrupted b0 upper bits.

    Flipping bits 2+ of b0 (y-form byte 99) produces a non-canonical encoding.
    Rust rejects it via the canonical round-trip check.  chiavdf ignores the
    upper bits of b0 when reconstructing b and returns True — this test fails
    until chiavdf fixes the bug.
    """
    x_s = bytes.fromhex(X_S_1024_HEX)
    output = bytes.fromhex(OUTPUT_1024_HEX)
    bad = bytearray(output)
    bad[99] ^= 0x04

    assert not _cpp_result(DISC_1024_STR, x_s, bytes(bad), ITERS_1024, disc_bits=1024), \
        "chiavdf accepted a proof with non-canonical b0 upper bits (known bug in chiavdf 1.1.14)"


def test_is_gen_trailing_bytes_both_accept(disc1, x_s, proof1):
    """
    IS_GEN (0x08) and IS_1 (0x04) forms skip the canonical check entirely.
    Bytes 1-99 of x and bytes 101-199 of the IS_1 pi form are don't-cares
    for both verifiers.
    """
    bad_x = bytearray(x_s)
    bad_x[99] ^= 0xff  # last byte of IS_GEN input — ignored

    bad_proof = bytearray(proof1)
    bad_proof[199] ^= 0xff  # last byte of IS_1 pi form — ignored

    assert rust_verify(disc1, bytes(bad_x), bytes(bad_proof), ITERS_1, DISC_BITS, 0), \
        "Rust should accept (IS_GEN/IS_1 trailing bytes are don't-cares)"
    assert cpp_verify(disc1, bytes(bad_x), bytes(bad_proof), ITERS_1, DISC_BITS, 0), \
        "chiavdf should accept (IS_GEN/IS_1 trailing bytes are don't-cares)"
