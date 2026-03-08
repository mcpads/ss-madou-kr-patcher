/// Saturn memory address (virtual address).
pub type VAddr = u32;

/// A memory region mapping binary data to a virtual address range.
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub name: String,
    /// Start virtual address this region is mapped to.
    pub base_addr: VAddr,
    /// Binary data.
    pub data: Vec<u8>,
}

impl MemoryRegion {
    pub fn new(name: impl Into<String>, base_addr: VAddr, data: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            base_addr,
            data,
        }
    }

    pub fn contains(&self, addr: VAddr) -> bool {
        addr >= self.base_addr && (addr - self.base_addr) < self.data.len() as u32
    }

    pub fn end_addr(&self) -> VAddr {
        self.base_addr + self.data.len() as u32
    }

    pub fn read_u8(&self, addr: VAddr) -> Option<u8> {
        if !self.contains(addr) {
            return None;
        }
        let offset = (addr - self.base_addr) as usize;
        Some(self.data[offset])
    }

    pub fn read_u16_be(&self, addr: VAddr) -> Option<u16> {
        if addr < self.base_addr {
            return None;
        }
        let offset = (addr - self.base_addr) as usize;
        if offset + 2 > self.data.len() {
            return None;
        }
        Some(u16::from_be_bytes([self.data[offset], self.data[offset + 1]]))
    }

    pub fn read_u32_be(&self, addr: VAddr) -> Option<u32> {
        if addr < self.base_addr {
            return None;
        }
        let offset = (addr - self.base_addr) as usize;
        if offset + 4 > self.data.len() {
            return None;
        }
        Some(u32::from_be_bytes([
            self.data[offset],
            self.data[offset + 1],
            self.data[offset + 2],
            self.data[offset + 3],
        ]))
    }
}

/// Address space composed of multiple memory regions.
pub struct AddressSpace {
    regions: Vec<MemoryRegion>,
}

impl AddressSpace {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub fn add_region(&mut self, region: MemoryRegion) {
        self.regions.push(region);
    }

    pub fn regions(&self) -> &[MemoryRegion] {
        &self.regions
    }

    pub fn read_u8(&self, addr: VAddr) -> Option<u8> {
        self.regions.iter().find_map(|r| r.read_u8(addr))
    }

    pub fn read_u16_be(&self, addr: VAddr) -> Option<u16> {
        self.regions.iter().find_map(|r| r.read_u16_be(addr))
    }

    pub fn read_u32_be(&self, addr: VAddr) -> Option<u32> {
        self.regions.iter().find_map(|r| r.read_u32_be(addr))
    }

    pub fn find_region(&self, addr: VAddr) -> Option<&MemoryRegion> {
        self.regions.iter().find(|r| r.contains(addr))
    }

    /// Find all occurrences of a 4-byte big-endian value across all regions.
    /// Returns virtual addresses where the value was found.
    pub fn find_u32_be(&self, value: u32) -> Vec<VAddr> {
        let needle = value.to_be_bytes();
        let mut results = Vec::new();
        for region in &self.regions {
            for i in 0..region.data.len().saturating_sub(3) {
                if region.data[i..i + 4] == needle {
                    results.push(region.base_addr + i as u32);
                }
            }
        }
        results
    }

    /// Find all occurrences of a byte pattern across all regions.
    /// Returns virtual addresses where the pattern starts.
    pub fn find_bytes(&self, pattern: &[u8]) -> Vec<VAddr> {
        if pattern.is_empty() {
            return Vec::new();
        }
        let mut results = Vec::new();
        for region in &self.regions {
            for i in 0..region.data.len().saturating_sub(pattern.len() - 1) {
                if region.data[i..i + pattern.len()] == *pattern {
                    results.push(region.base_addr + i as u32);
                }
            }
        }
        results
    }

    /// Read a slice of bytes from the address space.
    pub fn read_bytes(&self, addr: VAddr, len: usize) -> Option<Vec<u8>> {
        for region in &self.regions {
            if addr >= region.base_addr {
                let offset = (addr - region.base_addr) as usize;
                if offset + len <= region.data.len() {
                    return Some(region.data[offset..offset + len].to_vec());
                }
            }
        }
        None
    }

    /// Read a null-terminated ASCII string from the address space (max 256 bytes).
    pub fn read_cstring(&self, addr: VAddr) -> Option<String> {
        let mut buf = Vec::new();
        for i in 0..256u32 {
            match self.read_u8(addr + i) {
                Some(0) => break,
                Some(b) if b.is_ascii_graphic() || b == b' ' || b == b'.' => buf.push(b),
                _ => break,
            }
        }
        if buf.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&buf).into_owned())
        }
    }
}

impl Default for AddressSpace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod tests;
