use thiserror::Error;

const CNX_MAGIC: &[u8; 3] = b"CNX";
const CNX_VERSION: u8 = 0x02;
const HEADER_SIZE: usize = 16;

#[derive(Debug, Clone)]
pub struct CnxHeader {
    /// Content subtype (e.g. b"bin", b"bit", b"bmd").
    pub subtype: [u8; 3],
    /// Compressed data size (excludes 16-byte header).
    pub compressed_size: u32,
    /// Expected decompressed output size.
    pub decompressed_size: u32,
}

#[derive(Debug, Error)]
pub enum CnxError {
    #[error("not a CNX file (invalid magic or version)")]
    InvalidMagic,
    #[error("data too short ({0} bytes, need at least {HEADER_SIZE})")]
    TooShort(usize),
    #[error("decompressed size mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: usize, actual: usize },
    #[error("unexpected end of compressed data at offset {0}")]
    UnexpectedEof(usize),
}

/// Check if data starts with a valid CNX header.
pub fn is_cnx(data: &[u8]) -> bool {
    data.len() >= HEADER_SIZE && data[0..3] == *CNX_MAGIC && data[3] == CNX_VERSION
}

/// Parse the 16-byte CNX header.
pub fn parse_header(data: &[u8]) -> Result<CnxHeader, CnxError> {
    if data.len() < HEADER_SIZE {
        return Err(CnxError::TooShort(data.len()));
    }
    if data[0..3] != *CNX_MAGIC || data[3] != CNX_VERSION {
        return Err(CnxError::InvalidMagic);
    }
    let subtype = [data[4], data[5], data[6]];
    let compressed_size = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    let decompressed_size = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
    Ok(CnxHeader {
        subtype,
        compressed_size,
        decompressed_size,
    })
}

const WINDOW_SIZE: usize = 0x800;
const WINDOW_MASK: usize = 0x7FF;

/// Decompress a CNX-compressed buffer. Input includes the 16-byte header.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, CnxError> {
    let header = parse_header(data)?;
    let expected_size = header.decompressed_size as usize;
    let source_end = HEADER_SIZE + header.compressed_size as usize;

    if data.len() < source_end {
        return Err(CnxError::TooShort(data.len()));
    }

    let mut output = Vec::with_capacity(expected_size);
    let mut ring = [0u8; WINDOW_SIZE];
    let mut bp: usize = 0;
    let mut sp: usize = HEADER_SIZE;

    while sp < source_end {
        let flag = data[sp];
        sp += 1;

        if flag == 0 {
            break;
        }

        let mut f = flag;
        for _ in 0..4 {
            if sp >= source_end {
                break;
            }

            match f & 0x03 {
                0 => {
                    if sp >= source_end {
                        break;
                    }
                    let skip = data[sp] as usize;
                    sp += skip + 1;
                    break;
                }
                1 => {
                    let byte = data[sp];
                    sp += 1;
                    output.push(byte);
                    ring[bp] = byte;
                    bp = (bp + 1) & WINDOW_MASK;
                }
                2 => {
                    if sp + 1 >= source_end {
                        break;
                    }
                    let pair = u16::from_be_bytes([data[sp], data[sp + 1]]);
                    sp += 2;
                    let distance = ((pair >> 5) as usize) + 1;
                    let length = ((pair & 0x1F) as usize) + 4;
                    for _ in 0..length {
                        let byte = ring[bp.wrapping_sub(distance) & WINDOW_MASK];
                        output.push(byte);
                        ring[bp] = byte;
                        bp = (bp + 1) & WINDOW_MASK;
                    }
                }
                3 => {
                    if sp >= source_end {
                        break;
                    }
                    let count = data[sp] as usize;
                    sp += 1;
                    for _ in 0..count {
                        if sp >= source_end {
                            break;
                        }
                        let byte = data[sp];
                        sp += 1;
                        output.push(byte);
                        ring[bp] = byte;
                        bp = (bp + 1) & WINDOW_MASK;
                    }
                }
                _ => unreachable!(),
            }

            f >>= 2;
        }
    }

    if output.len() != expected_size {
        return Err(CnxError::SizeMismatch {
            expected: expected_size,
            actual: output.len(),
        });
    }

    Ok(output)
}

// ---------------------------------------------------------------------------
// Compressor
// ---------------------------------------------------------------------------

const MAX_MATCH_LEN: usize = 35; // 5 bits + 4
const MIN_MATCH_LEN: usize = 4;
const HASH_BITS: usize = 14;
const HASH_SIZE: usize = 1 << HASH_BITS;
const MAX_CHAIN_LEN: usize = 128;

/// Hash-chain match finder for O(N) amortised LZ matching.
struct MatchFinder {
    head: Vec<u32>,
    chain: Vec<u32>,
}

impl MatchFinder {
    fn new(data_len: usize) -> Self {
        Self {
            head: vec![u32::MAX; HASH_SIZE],
            chain: vec![u32::MAX; data_len],
        }
    }

    #[inline]
    fn hash3(data: &[u8], pos: usize) -> usize {
        let a = data[pos] as usize;
        let b = data[pos + 1] as usize;
        let c = data[pos + 2] as usize;
        ((a << 10) ^ (b << 5) ^ c) & (HASH_SIZE - 1)
    }

    fn insert(&mut self, data: &[u8], pos: usize) {
        if pos + 2 >= data.len() {
            return;
        }
        let h = Self::hash3(data, pos);
        self.chain[pos] = self.head[h];
        self.head[h] = pos as u32;
    }

    fn find_best(&self, data: &[u8], pos: usize) -> Option<(u16, u16)> {
        let remaining = data.len() - pos;
        if remaining < MIN_MATCH_LEN || pos == 0 || pos + 2 >= data.len() {
            return None;
        }
        let h = Self::hash3(data, pos);
        let max_len = remaining.min(MAX_MATCH_LEN);
        let min_pos = pos.saturating_sub(WINDOW_SIZE);

        let mut best_dist: u16 = 0;
        let mut best_len: usize = 0;
        let mut candidate = self.head[h];
        let mut steps = 0;

        while candidate != u32::MAX {
            let cand = candidate as usize;
            if cand < min_pos {
                break;
            }
            if cand >= pos {
                candidate = self.chain[cand];
                continue;
            }
            let distance = pos - cand;
            let mut length = 0;
            for i in 0..max_len {
                if data[cand + (i % distance)] == data[pos + i] {
                    length += 1;
                } else {
                    break;
                }
            }
            if length >= MIN_MATCH_LEN && length > best_len {
                best_dist = distance as u16;
                best_len = length;
                if best_len == max_len {
                    break;
                }
            }
            candidate = self.chain[cand];
            steps += 1;
            if steps >= MAX_CHAIN_LEN {
                break;
            }
        }

        if best_len >= MIN_MATCH_LEN {
            Some((best_dist, best_len as u16))
        } else {
            None
        }
    }
}

/// Internal operation types for the compressor.
enum CompOp {
    /// Single literal byte (mode 1): 1 raw byte.
    Single(u8),
    /// Multi-literal (mode 3): count + raw bytes (count ≥ 2).
    Literal(Vec<u8>),
    /// LZ reference (mode 2): (distance 1..=2048, length 4..=35).
    Match { distance: u16, length: u16 },
}

/// Compress data using the CNX v2 format.
///
/// `subtype` is the 3-byte content type (e.g., `b"bit"` for bitmap data).
/// Returns the full compressed buffer including the 16-byte header.
pub fn compress(data: &[u8], subtype: &[u8; 3]) -> Vec<u8> {
    let ops = merge_single_runs(generate_ops(data));
    let packed = pack_ops(&ops);

    let compressed_size = packed.len() as u32;
    let decompressed_size = data.len() as u32;

    let mut result = Vec::with_capacity(HEADER_SIZE + packed.len());
    result.extend_from_slice(CNX_MAGIC);
    result.push(CNX_VERSION);
    result.extend_from_slice(subtype);
    result.push(0x10); // padding byte (matches original files)
    result.extend_from_slice(&compressed_size.to_be_bytes());
    result.extend_from_slice(&decompressed_size.to_be_bytes());
    result.extend_from_slice(&packed);

    result
}

fn generate_ops(data: &[u8]) -> Vec<CompOp> {
    let mut pos: usize = 0;
    let mut ops = Vec::new();
    let mut mf = MatchFinder::new(data.len());

    while pos < data.len() {
        let best = mf.find_best(data, pos);

        if let Some((distance, length)) = best {
            // Lazy matching: check if skipping one byte yields a longer match.
            let mut use_literal = false;
            if (length as usize) < MAX_MATCH_LEN && pos + 1 < data.len() {
                mf.insert(data, pos);
                if let Some((_d2, l2)) = mf.find_best(data, pos + 1) {
                    if l2 > length + 1 {
                        use_literal = true;
                    }
                }
            }

            if use_literal {
                ops.push(CompOp::Single(data[pos]));
                pos += 1;
            } else {
                let lazy_ran = (length as usize) < MAX_MATCH_LEN && pos + 1 < data.len();
                let start = if lazy_ran { pos + 1 } else { pos };
                for p in start..pos + length as usize {
                    mf.insert(data, p);
                }
                ops.push(CompOp::Match { distance, length });
                pos += length as usize;
            }
        } else {
            mf.insert(data, pos);
            ops.push(CompOp::Single(data[pos]));
            pos += 1;
        }
    }

    ops
}

/// Merge runs of consecutive Single ops into Literal ops when doing so saves
/// space.  Runs of ≤ 4 Singles are kept as-is (they pack well into 4-op flag
/// groups).  Runs of ≥ 5 Singles are batched into Literal ops (the 1-byte
/// count overhead is offset by needing fewer flag-byte groups).
fn merge_single_runs(ops: Vec<CompOp>) -> Vec<CompOp> {
    let mut result = Vec::with_capacity(ops.len());
    let mut i = 0;
    while i < ops.len() {
        if matches!(ops[i], CompOp::Single(_)) {
            // Count consecutive Singles.
            let start = i;
            while i < ops.len() && matches!(ops[i], CompOp::Single(_)) {
                i += 1;
            }
            let run_len = i - start;
            if run_len >= 5 {
                // Merge into Literal(s) of up to 255 bytes each.
                let mut buf = Vec::with_capacity(run_len);
                for op in &ops[start..i] {
                    if let CompOp::Single(b) = op {
                        buf.push(*b);
                    }
                }
                for chunk in buf.chunks(255) {
                    result.push(CompOp::Literal(chunk.to_vec()));
                }
            } else {
                // Keep as Singles.
                for op in &ops[start..i] {
                    if let CompOp::Single(b) = op {
                        result.push(CompOp::Single(*b));
                    }
                }
            }
        } else {
            // Move non-Single op.
            let op = match &ops[i] {
                CompOp::Match { distance, length } => {
                    CompOp::Match { distance: *distance, length: *length }
                }
                CompOp::Literal(v) => CompOp::Literal(v.clone()),
                CompOp::Single(b) => CompOp::Single(*b),
            };
            result.push(op);
            i += 1;
        }
    }
    result
}


fn pack_ops(ops: &[CompOp]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < ops.len() {
        let chunk_end = (i + 4).min(ops.len());
        let chunk_size = chunk_end - i;

        let mut flag: u8 = 0;
        let mut data_buf = Vec::new();

        for j in 0..chunk_size {
            let op = &ops[i + j];
            match op {
                CompOp::Single(byte) => {
                    flag |= 1 << (j * 2); // mode 1: single literal
                    data_buf.push(*byte);
                }
                CompOp::Literal(bytes) => {
                    flag |= 3 << (j * 2); // mode 3: multi-literal
                    data_buf.push(bytes.len() as u8);
                    data_buf.extend_from_slice(bytes);
                }
                CompOp::Match { distance, length } => {
                    flag |= 2 << (j * 2); // mode 2: LZ reference
                    let dist_enc = *distance - 1;
                    let len_enc = *length - 4;
                    let pair = (dist_enc << 5) | len_enc;
                    data_buf.extend_from_slice(&pair.to_be_bytes());
                }
            }
        }

        // Partial chunk: remaining 2-bit codes are 0 (mode 0 = skip).
        // Mode 0 reads a skip-count byte then breaks. Provide count = 0.
        if chunk_size < 4 {
            data_buf.push(0x00);
        }

        result.push(flag);
        result.extend_from_slice(&data_buf);

        i = chunk_end;
    }

    // Terminator: a zero flag byte.
    result.push(0x00);

    result
}

#[cfg(test)]
#[path = "cnx_tests.rs"]
mod tests;
