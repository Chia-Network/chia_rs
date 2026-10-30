use chia_protocol::{BlockRecord, Bytes32, ClassgroupElement, FullBlock, VDFInfo, VDFProof};
use chia_streamable_macro::Streamable;
use chia_traits::Streamable;
use clap::Parser;
use rusqlite::{Connection, OpenFlags};
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

// Shared by mainnet, testnet11, and testneta.
const MIN_BLOCKS_PER_CHALLENGE_BLOCK: u8 = 16;
const MAX_SUB_SLOT_BLOCKS: u32 = 128;

/// Export all VDF proofs, including orphans, from a v2 blockchain database.
/// The binary corpus can be concatenated with exports from other networks and
/// checked by chiavdf's verify_vdf_corpus tool.
#[derive(Parser)]
struct Args {
    database: PathBuf,
    /// Output file; must not already exist. Discard it if extraction fails.
    output: PathBuf,
}

// Concatenated Streamable records, with no file header or record count.
// Integers are big-endian; Bytes have u32 length prefixes.
// Keep this field order in sync with chiavdf's verify_vdf_corpus.cpp.
#[derive(Streamable, Debug, PartialEq)]
struct CorpusEntry {
    height: u32,
    input: ClassgroupElement,
    // number_of_iterations is the resolved proof segment length, not
    // necessarily the committed count in the original block's VDFInfo.
    info: VDFInfo,
    proof: VDFProof,
}

struct VdfEntry<'a> {
    info: &'a VDFInfo,
    proof: &'a VDFProof,
    input: ClassgroupElement,
    iterations: u64,
}

impl VdfEntry<'_> {
    fn corpus_entry(&self, height: u32) -> CorpusEntry {
        let mut info = self.info.clone();
        info.number_of_iterations = self.iterations;
        CorpusEntry {
            height,
            input: self.input,
            info,
            proof: self.proof.clone(),
        }
    }
}

fn add_proof<'a>(
    proofs: &mut Vec<VdfEntry<'a>>,
    field: &str,
    info: Option<&'a VDFInfo>,
    proof: Option<&'a VDFProof>,
    input: Option<ClassgroupElement>,
    iterations: Option<u64>,
) -> Result<()> {
    let (info, proof) = match (info, proof) {
        (None, None) => return Ok(()),
        (Some(info), Some(proof)) => (info, proof),
        _ => return Err(format!("{field}: missing VDF info or proof").into()),
    };
    let (input, iterations) = if proof.normalized_to_identity {
        // A compact proof covers the full committed iteration count, starting
        // from the identity, rather than just the segment after an ancestor.
        (ClassgroupElement::default(), info.number_of_iterations)
    } else {
        (
            input.ok_or_else(|| format!("{field}: missing input element"))?,
            iterations.unwrap_or(info.number_of_iterations),
        )
    };
    proofs.push(VdfEntry {
        info,
        proof,
        input,
        iterations,
    });
    Ok(())
}

fn record(records: &HashMap<Bytes32, BlockRecord>, hash: Bytes32) -> Result<&BlockRecord> {
    records
        .get(&hash)
        .ok_or_else(|| format!("missing ancestor record {hash}").into())
}

fn block_proofs<'a>(
    block: &'a FullBlock,
    records: &HashMap<Bytes32, BlockRecord>,
) -> Result<Vec<VdfEntry<'a>>> {
    let identity = Some(ClassgroupElement::default());
    let previous = if block.height() == 0 {
        None
    } else {
        Some(record(records, block.prev_header_hash())?)
    };
    let cc_input = previous.map_or(identity, |prev| Some(prev.challenge_vdf_output));
    let icc_input = match previous {
        Some(prev) if !prev.is_challenge_block(MIN_BLOCKS_PER_CHALLENGE_BLOCK) => {
            prev.infused_challenge_vdf_output
        }
        _ => identity,
    };
    let mut proofs = Vec::new();
    for (index, slot) in block.finished_sub_slots.iter().enumerate() {
        let prefix = format!("finished_sub_slots[{index}]");
        // RC info stores the segment length. CC/ICC info may instead commit
        // to the count from the start of the slot or challenge.
        let iterations = Some(slot.reward_chain.end_of_slot_vdf.number_of_iterations);
        add_proof(
            &mut proofs,
            &format!("{prefix}.rc"),
            Some(&slot.reward_chain.end_of_slot_vdf),
            Some(&slot.proofs.reward_chain_slot_proof),
            identity,
            None,
        )?;
        add_proof(
            &mut proofs,
            &format!("{prefix}.cc"),
            Some(&slot.challenge_chain.challenge_chain_end_of_slot_vdf),
            Some(&slot.proofs.challenge_chain_slot_proof),
            if index == 0 { cc_input } else { identity },
            iterations,
        )?;
        add_proof(
            &mut proofs,
            &format!("{prefix}.icc"),
            slot.infused_challenge_chain
                .as_ref()
                .map(|icc| &icc.infused_challenge_chain_end_of_slot_vdf),
            slot.proofs.infused_challenge_chain_slot_proof.as_ref(),
            if index == 0 { icc_input } else { identity },
            iterations,
        )?;
    }

    let reward = &block.reward_chain_block;
    add_proof(
        &mut proofs,
        "rc_sp",
        reward.reward_chain_sp_vdf.as_ref(),
        block.reward_chain_sp_proof.as_ref(),
        identity,
        None,
    )?;
    let mut sp_input = identity;
    let sp_iterations = reward
        .reward_chain_sp_vdf
        .as_ref()
        .map(|vdf| vdf.number_of_iterations);
    if let (Some(cc), Some(rc), Some(proof)) = (
        &reward.challenge_chain_sp_vdf,
        &reward.reward_chain_sp_vdf,
        &block.challenge_chain_sp_proof,
    ) {
        if !proof.normalized_to_identity && cc.number_of_iterations != rc.number_of_iterations {
            // The RC challenge identifies the ancestor at which this partial
            // CC signage-point proof starts. Follow this block's branch.
            let mut ancestor = previous;
            loop {
                let prev = ancestor.ok_or("cc_sp: cannot find starting ancestor")?;
                if prev.reward_infusion_new_challenge == rc.challenge {
                    sp_input = Some(prev.challenge_vdf_output);
                    break;
                }
                ancestor = if prev.height == 0 {
                    None
                } else {
                    Some(record(records, prev.prev_hash)?)
                };
            }
        }
    }
    add_proof(
        &mut proofs,
        "cc_sp",
        reward.challenge_chain_sp_vdf.as_ref(),
        block.challenge_chain_sp_proof.as_ref(),
        sp_input,
        sp_iterations,
    )?;

    let iterations = Some(reward.reward_chain_ip_vdf.number_of_iterations);
    add_proof(
        &mut proofs,
        "rc_ip",
        Some(&reward.reward_chain_ip_vdf),
        Some(&block.reward_chain_ip_proof),
        identity,
        None,
    )?;
    add_proof(
        &mut proofs,
        "cc_ip",
        Some(&reward.challenge_chain_ip_vdf),
        Some(&block.challenge_chain_ip_proof),
        if block.finished_sub_slots.is_empty() {
            cc_input
        } else {
            identity
        },
        iterations,
    )?;
    add_proof(
        &mut proofs,
        "icc_ip",
        reward.infused_challenge_chain_ip_vdf.as_ref(),
        block.infused_challenge_chain_ip_proof.as_ref(),
        if block.finished_sub_slots.is_empty() {
            icc_input
        } else {
            identity
        },
        iterations,
    )?;
    Ok(proofs)
}

fn export(args: &Args) -> Result<()> {
    let db = Connection::open_with_flags(&args.database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let version: i64 =
        db.query_row("SELECT version FROM database_version", [], |row| row.get(0))?;
    if version != 2 {
        return Err("expected a v2 blockchain database".into());
    }
    let mut output = BufWriter::new(File::create_new(&args.output)?);
    let mut records = HashMap::new();
    let mut heights: VecDeque<(u32, Bytes32)> = VecDeque::new();
    // A signage point is in the current or previous sub-slot. Retain every
    // branch for two slots' worth of heights; no per-block SQL lookups needed.
    let history = 2 * MAX_SUB_SLOT_BLOCKS;
    let mut query = db.prepare(
        "SELECT header_hash, height, block, block_record FROM full_blocks ORDER BY height",
    )?;
    let mut rows = query.query([])?;
    let mut blocks = 0_u64;
    let mut proofs = 0_u64;
    while let Some(row) = rows.next()? {
        let hash = Bytes32::from(row.get::<_, [u8; 32]>(0)?);
        let height: u32 = row.get(1)?;
        while heights
            .front()
            .is_some_and(|(h, _)| *h < height.saturating_sub(history))
        {
            let (_, old_hash) = heights.pop_front().expect("nonempty history");
            records.remove(&old_hash);
        }
        let result = (|| -> Result<()> {
            let compressed: Vec<u8> = row.get(2)?;
            let record_bytes: Vec<u8> = row.get(3)?;
            let block_record = BlockRecord::from_bytes(&record_bytes)?;
            let block = FullBlock::from_bytes(&zstd::stream::decode_all(compressed.as_slice())?)?;
            if block.height() != height
                || block_record.height != height
                || block_record.header_hash != hash
            {
                return Err(
                    "database row disagrees with block/record height or record hash".into(),
                );
            }
            records.insert(hash, block_record);
            heights.push_back((height, hash));
            for proof in block_proofs(&block, &records)? {
                output.write_all(&proof.corpus_entry(height).to_bytes()?)?;
                proofs += 1;
            }
            Ok(())
        })();
        result.map_err(|error| format!("height={height} hash={hash}: {error}"))?;
        blocks += 1;
        if blocks.is_multiple_of(1000) {
            eprintln!("Height {height}: exported {blocks} blocks, {proofs} proofs");
        }
    }
    output.flush()?;
    eprintln!("Exported {blocks} blocks, {proofs} proofs");
    Ok(())
}

fn main() -> Result<()> {
    export(&Args::parse())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn corpus_streamable_layout_and_concatenation() {
        let entry = CorpusEntry {
            height: 0x0102_0304,
            input: ClassgroupElement::default(),
            info: VDFInfo {
                challenge: [9; 32].into(),
                number_of_iterations: 0x0102_0304_0506_0708,
                output: ClassgroupElement {
                    data: [6; 100].into(),
                },
            },
            proof: VDFProof {
                witness_type: 0,
                witness: vec![7; 100].into(),
                normalized_to_identity: false,
            },
        };
        let bytes = entry.to_bytes().unwrap();
        assert_eq!(bytes.len(), 350);
        // These offsets also pin the C++ reader's layout. There is no block hash.
        assert_eq!(&bytes[..4], b"\x01\x02\x03\x04");
        assert_eq!(
            &bytes[136..144],
            &entry.info.number_of_iterations.to_be_bytes()
        );
        assert_eq!(&bytes[244..249], &[0, 0, 0, 0, 100]);
        assert_eq!(bytes[349], 0);
        assert_eq!(CorpusEntry::from_bytes(&bytes).unwrap(), entry);
        let combined = [bytes.as_slice(), bytes.as_slice()].concat();
        let mut cursor = Cursor::new(combined.as_slice());
        assert_eq!(CorpusEntry::parse::<false>(&mut cursor).unwrap(), entry);
        assert_eq!(CorpusEntry::parse::<false>(&mut cursor).unwrap(), entry);
        assert_eq!(cursor.position(), combined.len() as u64);
        for length in 0..bytes.len() {
            assert!(CorpusEntry::from_bytes(&bytes[..length]).is_err());
        }
    }

    #[test]
    fn normalized_proof_uses_identity_and_committed_iterations() {
        let info = VDFInfo {
            challenge: Bytes32::default(),
            number_of_iterations: 100,
            output: ClassgroupElement::default(),
        };
        let mut proof = VDFProof {
            witness_type: 0,
            witness: vec![0; 100].into(),
            normalized_to_identity: true,
        };
        let partial_input = ClassgroupElement {
            data: [1; 100].into(),
        };
        let mut proofs = Vec::new();
        add_proof(
            &mut proofs,
            "cc_ip",
            Some(&info),
            Some(&proof),
            Some(partial_input),
            Some(20),
        )
        .unwrap();
        assert_eq!(proofs[0].input, ClassgroupElement::default());
        assert_eq!(proofs[0].iterations, 100);
        drop(proofs);

        proof.normalized_to_identity = false;
        let mut proofs = Vec::new();
        add_proof(
            &mut proofs,
            "cc_ip",
            Some(&info),
            Some(&proof),
            Some(partial_input),
            Some(20),
        )
        .unwrap();
        assert_eq!(proofs[0].input, partial_input);
        assert_eq!(proofs[0].iterations, 20);
    }

    #[test]
    fn missing_info_or_input_is_an_error() {
        let info = VDFInfo {
            challenge: Bytes32::default(),
            number_of_iterations: 100,
            output: ClassgroupElement::default(),
        };
        let proof = VDFProof {
            witness_type: 0,
            witness: vec![0; 100].into(),
            normalized_to_identity: false,
        };
        let mut proofs = Vec::new();
        assert!(add_proof(&mut proofs, "icc_ip", None, Some(&proof), None, None).is_err());
        assert!(add_proof(&mut proofs, "icc_ip", Some(&info), Some(&proof), None, None).is_err());
        add_proof(&mut proofs, "icc_ip", None, None, None, None).unwrap();
        assert!(proofs.is_empty());
    }
}
