#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * C API mirroring chiavdf's src/c_bindings/c_wrapper.h for VDF verification.
 *
 * Discriminant buffers are the unsigned big-endian magnitude of |D|
 * (same as chiavdf mpz_export / mpz_import). The verifier treats D as
 * negative, matching CheckProofOfTimeNWesolowski(-discriminant, ...).
 *
 * prove_wrapper is present for ABI compatibility but always fails:
 * this library only implements verification.
 */

bool create_discriminant_wrapper(
    const uint8_t* seed,
    size_t seed_size,
    size_t size_bits,
    uint8_t* result);

typedef struct {
    uint8_t* data;
    size_t length;
} ByteArray;

ByteArray prove_wrapper(
    const uint8_t* challenge_hash,
    size_t challenge_size,
    const uint8_t* x_s,
    size_t x_s_size,
    size_t discriminant_size_bits,
    uint64_t num_iterations);

/*
 * Verify an n-Wesolowski proof.
 *
 * discriminant_bytes / discriminant_size: |D| as big-endian bytes
 * x_s: 100-byte BQFC input form (length is implicit, as in chiavdf)
 * proof_blob / proof_blob_size: y || proof [|| segments...]
 * recursion: n-Wesolowski depth (witness_type)
 */
bool verify_n_wesolowski_wrapper(
    const uint8_t* discriminant_bytes,
    size_t discriminant_size,
    const uint8_t* x_s,
    const uint8_t* proof_blob,
    size_t proof_blob_size,
    uint64_t num_iterations,
    uint64_t recursion);

void delete_byte_array(ByteArray array);

#ifdef __cplusplus
}
#endif
