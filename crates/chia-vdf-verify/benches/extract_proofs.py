#!/usr/bin/env python3
"""Extract VDF proofs that validate with the identity element from a Chia blockchain DB.

Extracts reward-chain infusion-point VDFs, which use the identity element as
input and are self-contained (no block-context needed to verify).

Usage:
    python benches/extract_proofs.py [--db PATH] [--count N] [--output PATH]

Requires: chia_rs, zstd, chiavdf (pip install chia-rs zstd chiavdf)
"""
from __future__ import annotations

import argparse
import json
import os
import sqlite3
import sys
from pathlib import Path

import chiavdf
import zstd
import chia_rs

DISC_BITS = 1024
IDENTITY = bytes([0x08]) + bytes(99)


def extract(db_path: str, n: int) -> list[dict]:
    conn = sqlite3.connect(db_path)
    rows = conn.execute(
        "SELECT block FROM full_blocks ORDER BY height DESC LIMIT ?",
        (n * 100,),
    ).fetchall()

    proofs: list[dict] = []
    for (raw,) in rows:
        block = chia_rs.FullBlock.from_bytes(zstd.decompress(raw))
        rcb = block.reward_chain_block

        rc_ip_vdf = rcb.reward_chain_ip_vdf
        rc_ip_proof = block.reward_chain_ip_proof
        blob = rc_ip_vdf.output.data + bytes(rc_ip_proof.witness)
        challenge = bytes(rc_ip_vdf.challenge)
        disc_str = str(int(chiavdf.create_discriminant(challenge, DISC_BITS), 16))

        if chiavdf.verify_n_wesolowski(
            disc_str, IDENTITY, blob,
            rc_ip_vdf.number_of_iterations, DISC_BITS, rc_ip_proof.witness_type,
        ):
            proofs.append({
                "challenge": challenge.hex(),
                "input_el": IDENTITY.hex(),
                "proof_blob": blob.hex(),
                "iters": rc_ip_vdf.number_of_iterations,
                "witness_type": rc_ip_proof.witness_type,
                "height": block.height,
                "vdf_type": "rc_ip",
            })

        if len(proofs) >= n:
            break

    conn.close()
    return proofs[:n]


def main() -> None:
    default_db = os.path.expanduser("~/.chia/mainnet/db/blockchain_v2_mainnet.sqlite")
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--db", default=default_db, help="blockchain SQLite DB path")
    parser.add_argument("--count", type=int, default=20, help="number of proofs to extract")
    parser.add_argument("--output", default="benches/proofs.json", help="output JSON path")
    args = parser.parse_args()

    if not Path(args.db).exists():
        sys.exit(f"DB not found: {args.db}")

    proofs = extract(args.db, args.count)
    if not proofs:
        sys.exit("No validating VDF proofs found")

    with open(args.output, "w") as f:
        json.dump(proofs, f, indent=2)

    heights = sorted(p["height"] for p in proofs)
    wtypes = sorted(set(p["witness_type"] for p in proofs))
    print(f"Extracted {len(proofs)} validating proofs (heights {heights[0]:,}–{heights[-1]:,}, "
          f"witness types {wtypes})")
    print(f"Wrote {args.output} ({os.path.getsize(args.output):,} bytes)")


if __name__ == "__main__":
    main()
