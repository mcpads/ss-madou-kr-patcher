use super::*;

#[test]
fn lut_construction() {
    let t = EdcEccTables::new();
    // ecc_f_lut[0] = 0, ecc_f_lut[1] = 2 (GF(2^8) doubling)
    assert_eq!(t.ecc_f_lut[0], 0);
    assert_eq!(t.ecc_f_lut[1], 2);
    // ecc_f_lut[0x80] = 0x80 << 1 ^ 0x11D = 0x100 ^ 0x11D = 0x1D (mod 256)
    assert_eq!(t.ecc_f_lut[0x80], 0x1D);
    // edc_lut[0] = 0
    assert_eq!(t.edc_lut[0], 0);
}

#[test]
fn edc_empty_is_zero() {
    let t = EdcEccTables::new();
    assert_eq!(t.compute_edc(&[], 0), 0);
}

#[test]
fn edc_deterministic() {
    let t = EdcEccTables::new();
    let data = b"Hello, CD-ROM world!";
    let a = t.compute_edc(data, 0);
    let b = t.compute_edc(data, 0);
    assert_eq!(a, b);
    assert_ne!(a, 0); // non-trivial
}

/// Build a minimal synthetic Mode 1 sector, regenerate, and verify roundtrip.
#[test]
fn regenerate_roundtrip_synthetic() {
    let t = EdcEccTables::new();

    // Build a blank Mode 1 sector (2352 bytes).
    let mut sector = vec![0u8; 2352];
    // Sync pattern
    sector[0] = 0x00;
    sector[1..12].fill(0xFF);
    sector[11] = 0x00; // actually sector[11] should be 0x00 for proper sync
    // Proper sync: 00 FF FF FF FF FF FF FF FF FF FF 00
    // Header: minute, second, frame, mode
    sector[0x0C] = 0x00; // minute
    sector[0x0D] = 0x02; // second
    sector[0x0E] = 0x00; // frame
    sector[0x0F] = 0x01; // mode 1

    // Put some data in user area
    for i in 0x10..0x810 {
        sector[i] = (i & 0xFF) as u8;
    }

    // Regenerate EDC/ECC
    t.regenerate_sector(&mut sector);

    // Verify
    assert!(t.verify_sector_edc(&sector), "EDC mismatch after regeneration");
    assert!(t.verify_sector_ecc(&sector), "ECC mismatch after regeneration");
}

/// Modify user data, regenerate, and verify again.
#[test]
fn regenerate_after_modification() {
    let t = EdcEccTables::new();

    let mut sector = vec![0u8; 2352];
    sector[0] = 0x00;
    sector[1..12].fill(0xFF);
    sector[11] = 0x00;
    sector[0x0C] = 0x00;
    sector[0x0D] = 0x02;
    sector[0x0E] = 0x01;
    sector[0x0F] = 0x01;
    for i in 0x10..0x810 {
        sector[i] = ((i * 7) & 0xFF) as u8;
    }
    t.regenerate_sector(&mut sector);
    assert!(t.verify_sector_edc(&sector));
    assert!(t.verify_sector_ecc(&sector));

    // Modify a byte
    sector[0x100] ^= 0xAA;
    // Should now fail verification
    assert!(!t.verify_sector_edc(&sector), "EDC should fail after modification");

    // Regenerate and verify again
    t.regenerate_sector(&mut sector);
    assert!(t.verify_sector_edc(&sector), "EDC should pass after re-regeneration");
    assert!(t.verify_sector_ecc(&sector), "ECC should pass after re-regeneration");
}

/// Test regenerate_all_sectors with multiple sectors.
#[test]
fn regenerate_all_sectors_multi() {
    let t = EdcEccTables::new();

    let num_sectors = 3;
    let mut bin = vec![0u8; num_sectors * 2352];

    for s in 0..num_sectors {
        let off = s * 2352;
        // Sync
        bin[off] = 0x00;
        bin[off + 1..off + 12].fill(0xFF);
        bin[off + 11] = 0x00;
        // Header
        bin[off + 0x0C] = 0x00;
        bin[off + 0x0D] = 0x02;
        bin[off + 0x0E] = s as u8;
        bin[off + 0x0F] = 0x01; // Mode 1
        // User data
        for i in 0x10..0x810 {
            bin[off + i] = ((s + i) & 0xFF) as u8;
        }
    }

    let count = t.regenerate_all_sectors(&mut bin, num_sectors);
    assert_eq!(count, num_sectors);

    for s in 0..num_sectors {
        let sector = &bin[s * 2352..(s + 1) * 2352];
        assert!(t.verify_sector_edc(sector), "sector {} EDC failed", s);
        assert!(t.verify_sector_ecc(sector), "sector {} ECC failed", s);
    }
}

/// Non-Mode 1 sectors should be skipped.
#[test]
fn skip_non_mode1() {
    let t = EdcEccTables::new();
    let mut bin = vec![0u8; 2352];
    // Sync
    bin[0] = 0x00;
    bin[1..12].fill(0xFF);
    bin[11] = 0x00;
    // Mode 2
    bin[0x0F] = 0x02;

    let count = t.regenerate_all_sectors(&mut bin, 1);
    assert_eq!(count, 0, "Mode 2 sectors should be skipped");
}
