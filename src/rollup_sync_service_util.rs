use ethers::abi::{Abi, Function};
use ethers::utils::rlp;
use rlp::{Decodable, Encodable, Rlp, RlpStream};
use std::error::Error;
use tracing::info;

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkBlockRange {
    start_block_number: u64,
    end_block_number: u64,
}

impl Encodable for ChunkBlockRange {
    fn rlp_append(&self, stream: &mut RlpStream) {
        stream.begin_list(2);
        stream.append(&self.start_block_number);
        stream.append(&self.end_block_number);
    }
}

impl Decodable for ChunkBlockRange {
    fn decode(rlp: &Rlp) -> Result<Self, rlp::DecoderError> {
        Ok(ChunkBlockRange {
            start_block_number: rlp.val_at(0)?,
            end_block_number: rlp.val_at(1)?,
        })
    }
}

#[derive(Debug)]
pub enum CodecVersion {
    CodecV0,
    CodecV1,
}

impl CodecVersion {
    fn from_u8(value: u8) -> Result<Self, String> {
        match value {
            0 => Ok(CodecVersion::CodecV0),
            1 => Ok(CodecVersion::CodecV1),
            _ => Err(format!("unexpected batch version {}", value)),
        }
    }
}

pub fn decode_block_ranges_from_encoded_chunks(
    codec_version: CodecVersion,
    chunks: Vec<Vec<u8>>,
) -> Result<Vec<ChunkBlockRange>, Box<dyn Error>> {
    let mut chunk_block_ranges = Vec::new();
    for chunk in chunks {
        if chunk.len() < 1 {
            return Err("invalid chunk, length is less than 1".into());
        }

        let num_blocks = chunk[0] as usize;

        match codec_version {
            CodecVersion::CodecV0 => {
                if chunk.len() < 1 + num_blocks * 60 {
                    return Err(format!(
                        "invalid chunk byte length, expected: {}, got: {}",
                        1 + num_blocks * 60,
                        chunk.len()
                    )
                    .into());
                }

                let mut da_blocks = Vec::new();
                for i in 0..num_blocks {
                    let start_idx = 1 + i * 60; // add 1 to skip numBlocks byte
                    let end_idx = start_idx + 60;
                    let da_block = chunk[start_idx..end_idx].to_vec();
                    da_blocks.push(da_block);
                }

                let start_block_number = u64::from_be_bytes(da_blocks[0][0..8].try_into()?);
                let end_block_number =
                    u64::from_be_bytes(da_blocks[num_blocks - 1][0..8].try_into()?);

                chunk_block_ranges.push(ChunkBlockRange {
                    start_block_number,
                    end_block_number,
                });
            }
            CodecVersion::CodecV1 => {
                if chunk.len() != 1 + num_blocks * 60 {
                    return Err(format!(
                        "invalid chunk byte length, expected: {}, got: {}",
                        1 + num_blocks * 60,
                        chunk.len()
                    )
                    .into());
                }

                let mut da_blocks = Vec::new();
                info!("******Num of blocks: {:?}\n\n", num_blocks);
                for i in 0..num_blocks {
                    let start_idx = 1 + i * 60; // add 1 to skip numBlocks byte
                    let end_idx = start_idx + 60;
                    let da_block = chunk[start_idx..end_idx].to_vec();
                    da_blocks.push(da_block);
                }

                let start_block_number = u64::from_be_bytes(da_blocks[0][0..8].try_into()?);
                let end_block_number =
                    u64::from_be_bytes(da_blocks[num_blocks - 1][0..8].try_into()?);

                chunk_block_ranges.push(ChunkBlockRange {
                    start_block_number,
                    end_block_number,
                });
            }
        }
    }
    Ok(chunk_block_ranges)
}

pub fn decode_chunk_block_ranges(
    tx_data: Vec<u8>,
    abi: &Abi,
) -> Result<Vec<ChunkBlockRange>, Box<dyn Error>> {
    const METHOD_ID_LENGTH: usize = 4;

    if tx_data.len() < METHOD_ID_LENGTH {
        return Err(format!(
            "transaction data is too short, length of tx data: {}, minimum length required: {}",
            tx_data.len(),
            METHOD_ID_LENGTH
        )
        .into());
    }

    let method_id = &tx_data[..METHOD_ID_LENGTH];

    let mut method: Option<&Function> = None;

    for f in abi.functions() {
        if f.short_signature().to_vec() == method_id {
            method = Some(f);
            break;
        }
    }

    let method =
        method.ok_or_else(|| format!("failed to get method by ID, ID: {:?}", method_id))?;

    let inputs = method.decode_input(&tx_data[METHOD_ID_LENGTH..])?;
    let version: u8 = inputs[0].clone().into_uint().unwrap().as_u64() as u8;
    let chunks: Vec<Vec<u8>> = inputs[2]
        .clone()
        .into_array()
        .unwrap()
        .into_iter()
        .map(|token| token.into_bytes().unwrap())
        .collect();

    let codec_version = CodecVersion::from_u8(version)?;

    decode_block_ranges_from_encoded_chunks(codec_version, chunks)
}
