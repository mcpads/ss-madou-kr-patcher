//! Shared test helpers for building synthetic disc images.
//!
//! These helpers create minimal valid BIN disc images with fake PVD,
//! directory records, and sector data for unit/integration testing.

use super::sector::{
    DiscImage, RAW_SECTOR_SIZE, SYNC_PATTERN, SYNC_SIZE, USER_DATA_OFFSET, USER_DATA_SIZE,
};

/// LBA of the Primary Volume Descriptor.
const PVD_SECTOR: usize = 16;

/// Build a minimal PVD sector user-data block for testing.
pub fn make_fake_pvd(volume_id: &str, root_lba: u32, root_size: u32) -> Vec<u8> {
    let mut data = vec![0u8; USER_DATA_SIZE];

    // Type code
    data[0] = 0x01;
    // "CD001"
    data[1..6].copy_from_slice(b"CD001");
    // Version
    data[6] = 0x01;

    // Volume ID at 0x28 (32 bytes, space padded)
    let vid = volume_id.as_bytes();
    let len = vid.len().min(32);
    data[0x28..0x28 + len].copy_from_slice(&vid[..len]);
    for b in &mut data[0x28 + len..0x28 + 32] {
        *b = 0x20;
    }

    // Volume Space Size at 0x50 (both-endian u32)
    let space: u32 = 57318;
    data[0x50..0x54].copy_from_slice(&space.to_le_bytes());
    data[0x54..0x58].copy_from_slice(&space.to_be_bytes());

    // Logical Block Size at 0x80 (both-endian u16)
    let block: u16 = 2048;
    data[0x80..0x82].copy_from_slice(&block.to_le_bytes());
    data[0x82..0x84].copy_from_slice(&block.to_be_bytes());

    // Root directory record at 0x9C (34 bytes).
    let root = &mut data[0x9C..0x9C + 34];
    root[0] = 34; // record length
    root[1] = 0; // extended attribute length
    // Location of Extent (both-endian u32)
    root[2..6].copy_from_slice(&root_lba.to_le_bytes());
    root[6..10].copy_from_slice(&root_lba.to_be_bytes());
    // Data Length (both-endian u32)
    root[10..14].copy_from_slice(&root_size.to_le_bytes());
    root[14..18].copy_from_slice(&root_size.to_be_bytes());
    // Flags: directory
    root[25] = 0x02;
    // File identifier length
    root[32] = 1;
    // File identifier: 0x00 (root ".")
    root[33] = 0x00;

    data
}

/// Build a directory extent buffer containing fake entries.
///
/// Each entry is `(name, lba, size, is_directory)`.
pub fn make_fake_directory(entries: &[(&str, u32, u32, bool)]) -> Vec<u8> {
    let mut buf = Vec::new();

    for &(name, lba, size, is_dir) in entries {
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u8;
        // Record length: 33 + name_len, rounded up to even.
        let mut rec_len = 33u8 + name_len;
        if rec_len % 2 != 0 {
            rec_len += 1;
        }

        let mut record = vec![0u8; rec_len as usize];
        record[0] = rec_len;
        // LBA (both-endian)
        record[2..6].copy_from_slice(&lba.to_le_bytes());
        record[6..10].copy_from_slice(&lba.to_be_bytes());
        // Size (both-endian)
        record[10..14].copy_from_slice(&size.to_le_bytes());
        record[14..18].copy_from_slice(&size.to_be_bytes());
        // Flags
        record[25] = if is_dir { 0x02 } else { 0x00 };
        // File identifier length
        record[32] = name_len;
        // File identifier
        record[33..33 + name_len as usize].copy_from_slice(name_bytes);

        buf.extend_from_slice(&record);
    }

    // Pad to a multiple of USER_DATA_SIZE
    let pad_to = ((buf.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE) * USER_DATA_SIZE;
    buf.resize(pad_to, 0);
    buf
}

/// Build a DiscImage containing a PVD sector (at index 16) and a directory
/// extent at the given LBA from raw sector data.
pub fn build_test_disc(pvd_user_data: &[u8], dir_lba: u32, dir_data: &[u8]) -> DiscImage {
    let dir_sectors = (dir_data.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;
    let min_sectors = (dir_lba as usize + dir_sectors).max(17);
    build_test_disc_sized(pvd_user_data, dir_lba, dir_data, min_sectors)
}

/// Like `build_test_disc` but with explicit minimum sector count.
pub fn build_test_disc_sized(
    pvd_user_data: &[u8],
    dir_lba: u32,
    dir_data: &[u8],
    min_sectors: usize,
) -> DiscImage {
    let dir_sectors = (dir_data.len() + USER_DATA_SIZE - 1) / USER_DATA_SIZE;
    let total_sectors = (dir_lba as usize + dir_sectors).max(17).max(min_sectors);

    let mut bin = vec![0u8; total_sectors * RAW_SECTOR_SIZE];

    // Write sync + mode byte into every sector.
    for i in 0..total_sectors {
        let offset = i * RAW_SECTOR_SIZE;
        bin[offset..offset + SYNC_SIZE].copy_from_slice(&SYNC_PATTERN);
        bin[offset + SYNC_SIZE + 3] = 0x01; // Mode 1
    }

    // Write PVD user data into sector 16.
    let pvd_offset = PVD_SECTOR * RAW_SECTOR_SIZE + USER_DATA_OFFSET;
    let copy_len = pvd_user_data.len().min(USER_DATA_SIZE);
    bin[pvd_offset..pvd_offset + copy_len].copy_from_slice(&pvd_user_data[..copy_len]);

    // Write directory data starting at dir_lba.
    for i in 0..dir_sectors {
        let sector_offset = (dir_lba as usize + i) * RAW_SECTOR_SIZE + USER_DATA_OFFSET;
        let data_offset = i * USER_DATA_SIZE;
        let remaining = dir_data.len() - data_offset;
        let to_copy = remaining.min(USER_DATA_SIZE);
        bin[sector_offset..sector_offset + to_copy]
            .copy_from_slice(&dir_data[data_offset..data_offset + to_copy]);
    }

    DiscImage::from_bytes(bin).unwrap()
}
