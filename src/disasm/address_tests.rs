use super::*;

#[test]
fn memory_region_contains() {
    let r = MemoryRegion::new("test", 0x1000, vec![0; 256]);
    assert!(r.contains(0x1000));
    assert!(r.contains(0x10FF));
    assert!(!r.contains(0x0FFF));
    assert!(!r.contains(0x1100));
}

#[test]
fn memory_region_read() {
    let r = MemoryRegion::new("test", 0x1000, vec![0x06, 0x00, 0x40, 0x00, 0xAB]);
    assert_eq!(r.read_u8(0x1000), Some(0x06));
    assert_eq!(r.read_u8(0x1004), Some(0xAB));
    assert_eq!(r.read_u8(0x1005), None);
    assert_eq!(r.read_u16_be(0x1000), Some(0x0600));
    assert_eq!(r.read_u32_be(0x1000), Some(0x06004000));
    assert_eq!(r.read_u32_be(0x1002), None); // not enough bytes
}

#[test]
fn address_space_multi_region() {
    let mut space = AddressSpace::new();
    space.add_region(MemoryRegion::new("ram_h", 0x0600_0000, vec![0xAA; 64]));
    space.add_region(MemoryRegion::new("ram_l", 0x0020_0000, vec![0xBB; 64]));

    assert_eq!(space.read_u8(0x0600_0000), Some(0xAA));
    assert_eq!(space.read_u8(0x0020_0000), Some(0xBB));
    assert_eq!(space.read_u8(0x0000_0000), None);
    assert_eq!(space.find_region(0x0600_0010).unwrap().name, "ram_h");
}

#[test]
fn find_u32_be() {
    let mut space = AddressSpace::new();
    // Place 0x00200000 at offset 0 and offset 8
    let mut data = vec![0u8; 16];
    data[0..4].copy_from_slice(&0x00200000u32.to_be_bytes());
    data[8..12].copy_from_slice(&0x00200000u32.to_be_bytes());
    data[4..8].copy_from_slice(&0xDEADBEEFu32.to_be_bytes());
    space.add_region(MemoryRegion::new("test", 0x1000, data));

    let hits = space.find_u32_be(0x00200000);
    assert_eq!(hits, vec![0x1000, 0x1008]);

    let hits2 = space.find_u32_be(0xDEADBEEF);
    assert_eq!(hits2, vec![0x1004]);

    assert!(space.find_u32_be(0x12345678).is_empty());
}

#[test]
fn find_bytes() {
    let mut space = AddressSpace::new();
    let data = b"hello world hello".to_vec();
    space.add_region(MemoryRegion::new("test", 0x2000, data));

    let hits = space.find_bytes(b"hello");
    assert_eq!(hits, vec![0x2000, 0x200C]);

    let hits2 = space.find_bytes(b"world");
    assert_eq!(hits2, vec![0x2006]);

    assert!(space.find_bytes(b"xyz").is_empty());
    assert!(space.find_bytes(b"").is_empty());
}

#[test]
fn read_cstring() {
    let mut space = AddressSpace::new();
    let mut data = b"COMMON.SEQ\0rest".to_vec();
    data.extend_from_slice(&[0x00; 4]);
    space.add_region(MemoryRegion::new("test", 0x3000, data));

    assert_eq!(space.read_cstring(0x3000), Some("COMMON.SEQ".to_string()));
    assert_eq!(space.read_cstring(0x3007), Some("SEQ".to_string()));
}

#[test]
fn read_bytes() {
    let mut space = AddressSpace::new();
    space.add_region(MemoryRegion::new("test", 0x1000, vec![0xAA, 0xBB, 0xCC, 0xDD]));

    assert_eq!(space.read_bytes(0x1000, 2), Some(vec![0xAA, 0xBB]));
    assert_eq!(space.read_bytes(0x1000, 4), Some(vec![0xAA, 0xBB, 0xCC, 0xDD]));
    assert_eq!(space.read_bytes(0x1000, 5), None); // too long
    assert_eq!(space.read_bytes(0x2000, 1), None); // out of range
}
