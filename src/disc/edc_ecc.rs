//! CD-ROM Mode 1 sector EDC/ECC computation.
//!
//! Based on Neill Corlett's ecm.c (GPLv2) algorithm.
//! EDC: CD-ROM CRC-32 variant (polynomial 0xD8018001)
//! ECC: Reed-Solomon Product Code (ECMA-130), P-parity + Q-parity

/// Lookup tables for EDC/ECC computation.
pub struct EdcEccTables {
    ecc_f_lut: [u8; 256],
    ecc_b_lut: [u8; 256],
    edc_lut: [u32; 256],
}

impl EdcEccTables {
    pub fn new() -> Self {
        let mut ecc_f_lut = [0u8; 256];
        let mut ecc_b_lut = [0u8; 256];
        let mut edc_lut = [0u32; 256];

        for i in 0u32..256 {
            // ECC forward/backward LUTs — GF(2^8), primitive polynomial 0x11D
            let j = ((i << 1) ^ (if i & 0x80 != 0 { 0x11D } else { 0 })) & 0xFF;
            ecc_f_lut[i as usize] = j as u8;
            ecc_b_lut[(i ^ j) as usize] = i as u8;

            // EDC LUT — CD-ROM CRC-32, reflected polynomial 0xD8018001
            let mut edc = i;
            for _ in 0..8 {
                edc = if edc & 1 != 0 {
                    (edc >> 1) ^ 0xD8018001
                } else {
                    edc >> 1
                };
            }
            edc_lut[i as usize] = edc;
        }

        Self { ecc_f_lut, ecc_b_lut, edc_lut }
    }

    /// Compute EDC over a byte slice.
    fn compute_edc(&self, data: &[u8], init: u32) -> u32 {
        let mut edc = init;
        for &byte in data {
            edc = (edc >> 8) ^ self.edc_lut[((edc ^ byte as u32) & 0xFF) as usize];
        }
        edc
    }

    /// Compute ECC P or Q parity block (ecm.c ecc_computeblock algorithm).
    ///
    /// Parameters match ECMA-130:
    ///   P-parity: major_count=86, minor_count=24, major_mult=2,  minor_inc=86
    ///   Q-parity: major_count=52, minor_count=43, major_mult=86, minor_inc=88
    fn ecc_compute_block(
        &self,
        src: &[u8],
        major_count: usize,
        minor_count: usize,
        major_mult: usize,
        minor_inc: usize,
        dest: &mut [u8],
    ) {
        let size = major_count * minor_count;
        for major in 0..major_count {
            let mut index = (major >> 1) * major_mult + (major & 1);
            let mut ecc_a: u8 = 0;
            let mut ecc_b: u8 = 0;
            for _ in 0..minor_count {
                let temp = if index < src.len() { src[index] } else { 0 };
                index += minor_inc;
                if index >= size {
                    index -= size;
                }
                ecc_a ^= temp;
                ecc_b ^= temp;
                ecc_a = self.ecc_f_lut[ecc_a as usize];
            }
            ecc_a = self.ecc_b_lut[(self.ecc_f_lut[ecc_a as usize] ^ ecc_b) as usize];
            dest[major] = ecc_a;
            dest[major + major_count] = ecc_a ^ ecc_b;
        }
    }

    /// Regenerate EDC and ECC for a Mode 1 sector (2352 bytes).
    pub fn regenerate_sector(&self, sector: &mut [u8]) {
        assert!(sector.len() >= 2352);
        assert_eq!(sector[0x00F], 0x01, "not a Mode 1 sector");

        // 1. Zero reserved bytes
        sector[0x814..0x81C].fill(0);

        // 2. EDC: CRC-32 over bytes 0x000..0x810 (sync+header+user_data = 2064 bytes)
        let edc = self.compute_edc(&sector[0x000..0x810], 0);
        sector[0x810..0x814].copy_from_slice(&edc.to_le_bytes());

        // 3. ECC: zero address bytes for computation, then restore
        let saved_addr = [sector[0x0C], sector[0x0D], sector[0x0E], sector[0x0F]];
        sector[0x0C..0x10].fill(0);

        // Zero P and Q parity areas before computation
        sector[0x81C..0x81C + 172].fill(0);
        sector[0x8C8..0x8C8 + 104].fill(0);

        // P-parity: src=sector[0x0C..], 86 vectors × 24 symbols
        let mut p_parity = [0u8; 172];
        self.ecc_compute_block(&sector[0x0C..0x0C + 86 * 24], 86, 24, 2, 86, &mut p_parity);
        sector[0x81C..0x81C + 172].copy_from_slice(&p_parity);

        // Q-parity: src=sector[0x0C..], 52 vectors × 43 symbols
        // Q needs P-parity in place, so rebuild src including P
        let mut q_parity = [0u8; 104];
        self.ecc_compute_block(&sector[0x0C..0x0C + 52 * 43], 52, 43, 86, 88, &mut q_parity);
        sector[0x8C8..0x8C8 + 104].copy_from_slice(&q_parity);

        // Restore address bytes
        sector[0x0C..0x10].copy_from_slice(&saved_addr);
    }

    /// Verify EDC of a Mode 1 sector. Returns true if valid.
    pub fn verify_sector_edc(&self, sector: &[u8]) -> bool {
        if sector.len() < 2352 || sector[0x00F] != 0x01 {
            return false;
        }
        let computed = self.compute_edc(&sector[0x000..0x810], 0);
        let stored = u32::from_le_bytes([
            sector[0x810], sector[0x811], sector[0x812], sector[0x813],
        ]);
        computed == stored
    }

    /// Verify ECC of a Mode 1 sector. Returns true if P and Q parity match.
    pub fn verify_sector_ecc(&self, sector: &[u8]) -> bool {
        if sector.len() < 2352 || sector[0x00F] != 0x01 {
            return false;
        }

        // Zero address for computation
        let mut buf = sector[0x0C..0x930].to_vec();
        buf[0..4].fill(0); // zero address bytes (relative to 0x0C)

        // Verify P-parity
        let mut p_check = [0u8; 172];
        self.ecc_compute_block(&buf[..86 * 24], 86, 24, 2, 86, &mut p_check);
        if &p_check[..] != &sector[0x81C..0x81C + 172] {
            return false;
        }

        // Verify Q-parity
        let mut q_check = [0u8; 104];
        self.ecc_compute_block(&buf[..52 * 43], 52, 43, 86, 88, &mut q_check);
        &q_check[..] == &sector[0x8C8..0x8C8 + 104]
    }

    /// Regenerate EDC/ECC for all data track sectors in a BIN image.
    /// Returns the number of sectors processed.
    pub fn regenerate_all_sectors(&self, bin: &mut [u8], data_sector_count: usize) -> usize {
        let mut count = 0;
        for i in 0..data_sector_count {
            let start = i * 2352;
            if start + 2352 > bin.len() {
                break;
            }
            if bin[start + 0x00F] == 0x01 {
                self.regenerate_sector(&mut bin[start..start + 2352]);
                count += 1;
            }
        }
        count
    }
}

#[cfg(test)]
#[path = "edc_ecc_tests.rs"]
mod tests;
