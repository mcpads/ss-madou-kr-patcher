use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use clap::ValueEnum;

/// Disassembly mode for the `disasm` CLI command.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum DisasmMode {
    Linear,
    Recursive,
    FindValue,
    FindString,
    Xrefs,
    Func,
    ScanFuncs,
    CallGraph,
    Callers,
    Strings,
    MemRefs,
}

use ss_madou::disasm::{AddressSpace, MemoryRegion, RecursiveDisassembler, disassemble_linear};
use ss_madou::disc::{DiscImage, Iso9660, SaturnHeader};
use ss_madou::output::ListingWriter;

pub(crate) fn parse_hex(s: &str) -> Result<u32> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u32::from_str_radix(s, 16).context("Invalid hex address")
}

pub(crate) fn cmd_disasm(
    rom: &Path,
    mode: DisasmMode,
    start_override: Option<&str>,
    end_override: Option<&str>,
    query: Option<&str>,
    output: &Path,
) -> Result<()> {
    let disc =
        DiscImage::from_bin_file(rom).context("Failed to open disc image")?;
    let iso =
        Iso9660::parse(&disc).context("Failed to parse ISO 9660")?;

    // Parse header to get 1st Read info
    let ip_data = disc
        .read_user_data(0)
        .context("Failed to read sector 0")?;
    let header =
        SaturnHeader::parse(ip_data).context("Failed to parse Saturn header")?;

    // Extract 1st Read binary via ISO 9660.
    // Try "0" first (common Saturn convention), then "1ST_READ.BIN".
    let entry = iso
        .find_file(&disc, "0")
        .ok()
        .flatten()
        .or_else(|| iso.find_file(&disc, "1ST_READ.BIN").ok().flatten())
        .context("Could not find 1st Read binary in disc image")?;

    let first_read = iso
        .extract_file(&disc, &entry)
        .context("Failed to extract 1st Read binary")?;

    // Binary is always loaded at the header's first_read_addr.
    // --start/--end only control the disassembly range (for linear mode).
    let load_addr = header.first_read_addr;

    let mut space = AddressSpace::new();
    space.add_region(MemoryRegion::new(
        "1st_read",
        load_addr,
        first_read.clone(),
    ));

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    // Pre-compute literal pool refs once for modes that need it.
    // linear/recursive/func modes don't use this scan.
    let needs_pool_refs = matches!(
        mode,
        DisasmMode::FindValue | DisasmMode::FindString | DisasmMode::Xrefs
            | DisasmMode::ScanFuncs | DisasmMode::CallGraph
            | DisasmMode::Callers | DisasmMode::Strings | DisasmMode::MemRefs
    );
    let pool_refs = if needs_pool_refs {
        linear_scan_literal_pool_refs(&space, load_addr, first_read.len())
    } else {
        Vec::new()
    };

    match mode {
        DisasmMode::Linear => {
            let mut file = fs::File::create(output).context("Failed to create output file")?;
            let scan_start = if let Some(s) = start_override {
                parse_hex(s)?
            } else {
                load_addr
            };
            let end_addr = if let Some(s) = end_override {
                parse_hex(s)?
            } else {
                load_addr + first_read.len() as u32
            };

            let lines = disassemble_linear(&space, scan_start, end_addr);
            ListingWriter::write_linear_listing(&mut file, &lines)?;
            file.flush()?;

            println!(
                "Linear disassembly: {} instructions, 0x{:08X}-0x{:08X} -> {}",
                lines.len(),
                scan_start,
                end_addr,
                output.display()
            );
        }
        DisasmMode::Recursive => {
            let mut file = fs::File::create(output).context("Failed to create output file")?;
            let mut disasm = RecursiveDisassembler::new(&space);
            disasm.add_entry_point(load_addr, Some("_start".into()));
            let db = disasm.run();

            let writer = ListingWriter::new(&db);
            writer.write_listing(&mut file)?;
            file.flush()?;

            println!(
                "Recursive disassembly: {} functions, {} code addresses -> {}",
                db.functions.len(),
                db.code_count(),
                output.display()
            );
        }
        DisasmMode::FindValue => {
            let q = query.context("--query required: hex value to search (e.g. 0x00200000)")?;
            let target = parse_hex(q)?;
            cmd_disasm_find_value(&space, load_addr, target, &pool_refs)?;
        }
        DisasmMode::FindString => {
            let q = query.context("--query required: string to search (e.g. COMMON.SEQ)")?;
            cmd_disasm_find_string(&space, load_addr, q, &pool_refs)?;
        }
        DisasmMode::Xrefs => {
            let q = query.context("--query required: hex address to query xrefs (e.g. 0x06000952)")?;
            let addr = parse_hex(q)?;
            cmd_disasm_xrefs(&space, load_addr, addr, &pool_refs)?;
        }
        DisasmMode::Func => {
            let q = query.context("--query required: hex address of function (e.g. 0x06046814)")?;
            let addr = parse_hex(q)?;
            let mut file = fs::File::create(output).context("Failed to create output file")?;
            cmd_disasm_func(&space, load_addr, addr, &mut file)?;
            file.flush()?;
        }
        DisasmMode::ScanFuncs => {
            let mut file = fs::File::create(&output).context("Failed to create output file")?;
            cmd_disasm_scan_funcs(&space, load_addr, first_read.len(), &pool_refs, &mut file)?;
            file.flush()?;
        }
        DisasmMode::CallGraph => {
            let q = query.context("--query required: hex address of function (e.g. 0x06048A34)")?;
            let addr = parse_hex(q)?;
            let mut file = fs::File::create(&output).context("Failed to create output file")?;
            cmd_disasm_call_graph(&space, load_addr, first_read.len(), addr, &pool_refs, &mut file)?;
            file.flush()?;
        }
        DisasmMode::Callers => {
            let q = query.context("--query required: hex address of function (e.g. 0x06048A00)")?;
            let addr = parse_hex(q)?;
            let mut file = fs::File::create(&output).context("Failed to create output file")?;
            cmd_disasm_callers(&space, load_addr, first_read.len(), addr, &pool_refs, &mut file)?;
            file.flush()?;
        }
        DisasmMode::Strings => {
            let min_len = query.map(|q| q.parse::<usize>().unwrap_or(4)).unwrap_or(4);
            let mut file = fs::File::create(&output).context("Failed to create output file")?;
            cmd_disasm_strings(&space, load_addr, first_read.len(), min_len, &pool_refs, &mut file)?;
            file.flush()?;
        }
        DisasmMode::MemRefs => {
            let q = query.context("--query required: memory range (e.g. 0x00200000-0x00300000)")?;
            let parts: Vec<&str> = q.split('-').collect();
            if parts.len() != 2 {
                anyhow::bail!("Expected format: START-END (e.g. 0x00200000-0x00300000)");
            }
            let range_start = parse_hex(parts[0].trim())?;
            let range_end = parse_hex(parts[1].trim())?;
            let mut file = fs::File::create(&output).context("Failed to create output file")?;
            cmd_disasm_mem_refs(&space, load_addr, first_read.len(), range_start, range_end, &pool_refs, &mut file)?;
            file.flush()?;
        }
    }

    Ok(())
}

/// Linear-scan the entire binary for MOV.L @(disp,PC) instructions that
/// load a specific value from literal pools. Much more comprehensive than
/// recursive disasm since it doesn't need to follow control flow.
pub(crate) fn linear_scan_literal_pool_refs(
    space: &AddressSpace,
    load_addr: u32,
    binary_len: usize,
) -> Vec<(u32, u32, u32)> {
    // Returns: (instruction_addr, literal_pool_addr, literal_pool_value)
    let mut results = Vec::new();
    let end_addr = load_addr + binary_len as u32;
    let mut pc = load_addr;
    while pc < end_addr {
        if let Some(opcode) = space.read_u16_be(pc) {
            let inst = ss_madou::sh2::decode(opcode);
            if let Some(pool_addr) = inst.literal_pool_addr(pc) {
                if let Some(value) = space.read_u32_be(pool_addr) {
                    results.push((pc, pool_addr, value));
                }
            }
        }
        pc += 2;
    }
    results
}

/// Find function start by walking backward from `addr` looking for
/// prologue pattern (register saves + STS.L PR,@-R15).
/// Returns (func_start, regs_saved) or None if no prologue found within limit.
fn find_prologue_backward(
    space: &AddressSpace,
    addr: u32,
    load_addr: u32,
) -> Option<(u32, Vec<u8>)> {
    let max_walk = 4096u32;
    let mut check_addr = addr.wrapping_sub(2);
    let limit = addr.saturating_sub(max_walk);
    while check_addr >= limit && check_addr >= load_addr {
        if let Some(opcode) = space.read_u16_be(check_addr) {
            if opcode == 0x4F22 {
                // STS.L PR,@-R15 found. Walk further backward for register saves
                // (MOV.L Rm,@-R15 = 0x2Fm6 for R8-R14).
                let mut func_start = check_addr;
                let mut regs_saved = Vec::new();
                let mut prev_addr = check_addr.wrapping_sub(2);
                while prev_addr >= load_addr {
                    if let Some(prev) = space.read_u16_be(prev_addr) {
                        if prev & 0xFF0F == 0x2F06 {
                            let reg = ((prev >> 4) & 0xF) as u8;
                            if (8..=14).contains(&reg) {
                                regs_saved.push(reg);
                                func_start = prev_addr;
                                prev_addr = prev_addr.wrapping_sub(2);
                                continue;
                            }
                        }
                    }
                    break;
                }
                regs_saved.reverse();
                return Some((func_start, regs_saved));
            }
        }
        check_addr = check_addr.wrapping_sub(2);
    }
    None
}

/// Find all occurrences of a 32-bit value in the binary, using full
/// linear scan to find every MOV.L @(disp,PC) that loads it.
fn cmd_disasm_find_value(
    space: &AddressSpace,
    load_addr: u32,
    target: u32,
    pool_refs: &[(u32, u32, u32)],
) -> Result<()> {
    println!("=== Searching for value 0x{target:08X} ===\n");

    // 1. Raw binary search (finds ALL occurrences including data)
    let raw_hits = space.find_u32_be(target);
    println!("Raw binary matches: {} occurrences", raw_hits.len());
    for addr in &raw_hits {
        let aligned = addr % 4 == 0;
        let file_offset = addr - load_addr;
        print!("  0x{addr:08X} (file +0x{file_offset:05X})");
        if !aligned {
            print!(" [unaligned]");
        }
        if let Some(s) = space.read_cstring(*addr) {
            if s.len() >= 4 {
                print!("  str: \"{s}\"");
            }
        }
        println!();
    }

    // 2. Linear scan: find all MOV.L @(disp,PC) instructions loading this value
    println!("\nLinear scan for MOV.L @(disp,PC) references...");
    let matching: Vec<_> = pool_refs
        .iter()
        .filter(|&&(_, _, val)| val == target)
        .collect();

    println!("Instructions loading 0x{target:08X}: {}\n", matching.len());
    for &&(inst_addr, pool_addr, _) in &matching {
        let inst_offset = inst_addr - load_addr;
        let pool_offset = pool_addr - load_addr;
        if let Some(opcode) = space.read_u16_be(inst_addr) {
            let inst = ss_madou::sh2::decode(opcode);
            println!(
                "  0x{inst_addr:08X} (+0x{inst_offset:05X}): {inst}  ; pool@0x{pool_addr:08X} (+0x{pool_offset:05X})"
            );
        }
    }

    // 3. Show aligned raw hits that are NOT referenced by any MOV.L instruction
    let pool_set: std::collections::HashSet<u32> =
        matching.iter().map(|&&(_, pa, _)| pa).collect();
    let unreferenced: Vec<_> = raw_hits
        .iter()
        .filter(|a| *a % 4 == 0 && !pool_set.contains(a))
        .collect();
    if !unreferenced.is_empty() {
        println!("\nAligned occurrences not referenced as literal pool ({}):", unreferenced.len());
        for &addr in &unreferenced {
            let file_offset = addr - load_addr;
            println!("  0x{addr:08X} (file +0x{file_offset:05X})");
        }
    }

    println!("\nDone.");
    Ok(())
}

/// Find a string in the binary and show all code that references it via literal pool.
fn cmd_disasm_find_string(
    space: &AddressSpace,
    load_addr: u32,
    needle: &str,
    pool_refs: &[(u32, u32, u32)],
) -> Result<()> {
    println!("=== Searching for string \"{needle}\" ===\n");

    let hits = space.find_bytes(needle.as_bytes());
    if hits.is_empty() {
        println!("String not found in binary.");
        return Ok(());
    }

    println!("Found {} occurrence(s):", hits.len());
    for &addr in &hits {
        let file_offset = addr - load_addr;
        let full_str = space.read_cstring(addr).unwrap_or_default();
        println!("  0x{addr:08X} (file +0x{file_offset:05X}): \"{full_str}\"");
    }

    // Linear scan for ALL literal pool references
    println!("\nLinear scan for literal pool references...");

    for &str_addr in &hits {
        println!("\nReferences to string at 0x{str_addr:08X}:");

        // Find literal pool entries pointing directly to this address
        let direct: Vec<_> = pool_refs
            .iter()
            .filter(|&&(_, _, val)| val == str_addr)
            .collect();

        if !direct.is_empty() {
            println!("  Direct references ({}):", direct.len());
            for &&(inst_addr, pool_addr, _) in &direct {
                let inst_offset = inst_addr - load_addr;
                if let Some(opcode) = space.read_u16_be(inst_addr) {
                    let inst = ss_madou::sh2::decode(opcode);
                    println!("    0x{inst_addr:08X} (+0x{inst_offset:05X}): {inst}  ; pool@0x{pool_addr:08X}");
                }
            }
        }

        // Search ±64 bytes for nearby literal pool values (string might be in a struct)
        let mut nearby: Vec<(u32, u32, u32)> = Vec::new();
        for &(inst_addr, pool_addr, val) in pool_refs {
            let delta = val as i64 - str_addr as i64;
            if delta != 0 && delta.abs() <= 64 {
                nearby.push((inst_addr, pool_addr, val));
            }
        }
        if !nearby.is_empty() {
            println!("  Nearby references (±64 bytes, {}):", nearby.len());
            for &(inst_addr, _pool_addr, val) in &nearby {
                let delta = val as i64 - str_addr as i64;
                let inst_offset = inst_addr - load_addr;
                if let Some(opcode) = space.read_u16_be(inst_addr) {
                    let inst = ss_madou::sh2::decode(opcode);
                    println!(
                        "    0x{inst_addr:08X} (+0x{inst_offset:05X}): {inst}  ; =0x{val:08X} (delta {delta:+})"
                    );
                }
            }
        }

        if direct.is_empty() && nearby.is_empty() {
            println!("  No literal pool references found.");
        }
    }

    println!("\nDone.");
    Ok(())
}

/// Show all cross-references to a specific address using linear scan.
fn cmd_disasm_xrefs(
    space: &AddressSpace,
    load_addr: u32,
    addr: u32,
    all_pool_refs: &[(u32, u32, u32)],
) -> Result<()> {
    let binary_len = space.regions().first().map_or(0, |r| r.data.len());
    let end_addr = load_addr + binary_len as u32;

    println!("=== Cross-references to 0x{addr:08X} ===\n");

    // 1. Literal pool references (MOV.L @(disp,PC) loading this value)
    let pool_refs: Vec<_> = all_pool_refs
        .iter()
        .filter(|&&(_, _, val)| val == addr)
        .collect();

    if !pool_refs.is_empty() {
        println!("Literal pool references ({}):", pool_refs.len());
        for &&(inst_addr, pool_addr, _) in &pool_refs {
            let inst_offset = inst_addr - load_addr;
            if let Some(opcode) = space.read_u16_be(inst_addr) {
                let inst = ss_madou::sh2::decode(opcode);
                println!("  0x{inst_addr:08X} (+0x{inst_offset:05X}): {inst}  ; pool@0x{pool_addr:08X}");
            }
        }
    }

    // 2. Direct branch/call references (BSR/BRA with PC-relative offset)
    let mut branch_refs = Vec::new();
    let mut pc = load_addr;
    while pc < end_addr {
        if let Some(opcode) = space.read_u16_be(pc) {
            let inst = ss_madou::sh2::decode(opcode);
            if let Some(target) = inst.branch_target(pc) {
                if target == addr {
                    branch_refs.push((pc, inst));
                }
            }
        }
        pc += 2;
    }

    if !branch_refs.is_empty() {
        println!("\nDirect branch/call references ({}):", branch_refs.len());
        for (from, inst) in &branch_refs {
            let from_offset = from - load_addr;
            println!("  0x{from:08X} (+0x{from_offset:05X}): {inst}");
        }
    }

    // 3. Raw binary matches
    let raw_hits = space.find_u32_be(addr);
    let pool_addrs: std::collections::HashSet<u32> =
        pool_refs.iter().map(|&&(_, pa, _)| pa).collect();
    let other_raw: Vec<_> = raw_hits
        .iter()
        .filter(|a| !pool_addrs.contains(a))
        .collect();
    if !other_raw.is_empty() {
        println!(
            "\nRaw binary matches (not literal pool, {}):",
            other_raw.len()
        );
        for &a in &other_raw {
            let file_offset = a - load_addr;
            let aligned = if a % 4 == 0 { "" } else { " [unaligned]" };
            println!("  0x{a:08X} (file +0x{file_offset:05X}){aligned}");
        }
    }

    if pool_refs.is_empty() && branch_refs.is_empty() && other_raw.is_empty() {
        println!("No references found.");
    }

    println!("\nDone.");
    Ok(())
}

/// Disassemble a single function (linear from given address until RTS).
fn cmd_disasm_func(
    space: &AddressSpace,
    load_addr: u32,
    func_addr: u32,
    out: &mut dyn Write,
) -> Result<()> {
    println!("=== Function at 0x{func_addr:08X} ===\n");

    // Run deep recursive disasm for full context
    let mut disasm = RecursiveDisassembler::new(space);
    disasm.add_entry_point(load_addr, Some("_start".into()));
    // Also add the target as an entry point in case it's not reachable from _start
    disasm.add_entry_point(func_addr, None);
    let db = disasm.run_deep();

    let label = db
        .labels
        .get(&func_addr)
        .cloned()
        .unwrap_or_else(|| format!("sub_{func_addr:08X}"));

    writeln!(out, "; -------- {label} --------")?;

    // Get xrefs to this function
    let callers = db.find_callers(func_addr);
    if !callers.is_empty() {
        write!(out, "; callers:")?;
        for c in &callers {
            let cl = db.containing_function(*c)
                .and_then(|f| db.labels.get(&f))
                .map(|s| s.as_str())
                .unwrap_or("???");
            write!(out, " {cl}@{c:08X}")?;
        }
        writeln!(out)?;
    }

    // Print function instructions
    let instructions = db.function_instructions(func_addr);
    let mut literal_pools = Vec::new();

    for &(&addr, ref line) in &instructions {
        write!(
            out,
            "{:08X}: {:04X}  {}",
            addr, line.opcode, line.instruction
        )?;

        if let Some(target) = line.branch_target {
            write!(out, "  ; -> 0x{target:08X}")?;
            if let Some(lbl) = db.labels.get(&target) {
                write!(out, " ({lbl})")?;
            }
        }

        if let Some(value) = line.literal_pool_value {
            write!(out, "  ; =0x{value:08X}")?;
            if let Some(lbl) = db.labels.get(&value) {
                write!(out, " -> {lbl}")?;
            }
            // Try to interpret the value
            if value >= 0x0020_0000 && value < 0x0030_0000 {
                write!(out, " [Work RAM Low]")?;
            } else if value >= 0x0600_0000 && value < 0x0610_0000 {
                if db.functions.contains(&value) {
                    // already shown via label
                } else if let Some(s) = space.read_cstring(value) {
                    if s.len() >= 3 && s.chars().all(|c| c.is_ascii_graphic() || c == ' ' || c == '.') {
                        write!(out, " \"{s}\"")?;
                    }
                }
            }

            // Collect literal pool entries for summary at end
            if let Some(pool_addr) = line.literal_pool_addr {
                literal_pools.push((pool_addr, value));
            }
        }

        writeln!(out)?;
    }

    // Print literal pool summary
    if !literal_pools.is_empty() {
        writeln!(out)?;
        writeln!(out, "; ---- literal pool ----")?;
        literal_pools.sort_by_key(|(addr, _)| *addr);
        literal_pools.dedup();
        for (pool_addr, value) in &literal_pools {
            write!(out, "; {pool_addr:08X}: {value:08X}")?;
            if let Some(lbl) = db.labels.get(value) {
                write!(out, "  -> {lbl}")?;
            }
            if let Some(s) = space.read_cstring(*value) {
                if s.len() >= 3 && s.chars().all(|c| c.is_ascii_graphic() || c == ' ' || c == '.') {
                    write!(out, "  \"{s}\"")?;
                }
            }
            writeln!(out)?;
        }
    }

    // Print to stdout as well
    println!("Function {label}: {} instructions", instructions.len());
    println!("Output -> see output file");

    Ok(())
}

/// Discovered function entry from prologue scanning.
struct ScannedFunc {
    /// Address of the first instruction (start of prologue).
    addr: u32,
    /// Number of registers saved in the prologue.
    regs_saved: Vec<u8>,
    /// Whether PR (return address) is saved (non-leaf).
    saves_pr: bool,
    /// Address of RTS if found within reasonable range.
    rts_addr: Option<u32>,
    /// Estimated size in bytes (addr..rts+delay_slot).
    size: Option<u32>,
}

/// Scan entire binary for SH-2 function prologues.
///
/// Detection patterns:
/// 1. `STS.L PR,@-R15` (0x4F22) preceded by optional `MOV.L Rn,@-R15` (0x2Fn6)
/// 2. Walks forward to find matching RTS (0x000B) for size estimation
fn cmd_disasm_scan_funcs(
    space: &AddressSpace,
    load_addr: u32,
    binary_len: usize,
    pool_refs: &[(u32, u32, u32)],
    out: &mut dyn Write,
) -> Result<()> {
    let end_addr = load_addr + binary_len as u32;
    let mut funcs: Vec<ScannedFunc> = Vec::new();

    // Also collect literal pool call targets (JSR/BSR targets via pool)
    let mut call_targets: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
    // Find JSR @Rn instructions and try to resolve their targets from nearby MOV.L @(disp,PC)
    for &(inst_addr, _pool_addr, value) in pool_refs {
        // Check if the next instruction after this MOV.L is a JSR
        if value >= load_addr && value < end_addr {
            // Check following instructions for JSR pattern
            for offset in (2..=6).step_by(2) {
                if let Some(next_opcode) = space.read_u16_be(inst_addr + offset) {
                    let next_inst = ss_madou::sh2::decode(next_opcode);
                    if matches!(next_inst.kind, ss_madou::sh2::InstructionKind::Jsr { .. }) {
                        call_targets.insert(value);
                        break;
                    }
                }
            }
        }
    }

    // Scan for STS.L PR,@-R15 (0x4F22) - the definitive non-leaf function marker
    let mut pc = load_addr;
    while pc < end_addr {
        if let Some(opcode) = space.read_u16_be(pc) {
            if opcode == 0x4F22 {
                // STS.L PR,@-R15 found. Use shared prologue detector to walk backward.
                // Pass pc+2 so find_prologue_backward finds 0x4F22 at pc and walks back for reg saves.
                let (func_start, regs_saved) =
                    if let Some((start, regs)) = find_prologue_backward(space, pc + 2, load_addr) {
                        (start, regs)
                    } else {
                        // No register saves before this STS.L PR,@-R15 — function starts here
                        (pc, Vec::new())
                    };

                // Walk forward to find RTS (with limit to avoid scanning past function)
                let max_scan = 4096u32; // reasonable max function size
                let mut rts_addr = None;
                let mut scan = pc + 2;
                while scan < end_addr && scan < pc + max_scan {
                    if let Some(opc) = space.read_u16_be(scan) {
                        if opc == 0x000B {
                            // RTS found
                            rts_addr = Some(scan);
                            break;
                        }
                        // If we hit another STS.L PR,@-R15, stop
                        if opc == 0x4F22 && scan != pc {
                            break;
                        }
                    }
                    scan += 2;
                }

                let size = rts_addr.map(|rts| rts + 4 - func_start); // +4 for RTS + delay slot

                funcs.push(ScannedFunc {
                    addr: func_start,
                    regs_saved,
                    saves_pr: true,
                    rts_addr,
                    size,
                });
            }
        }
        pc += 2;
    }

    // Also detect some leaf functions: BSR targets that don't save PR
    // These are addresses targeted by BSR/JSR that weren't already found
    let existing_addrs: std::collections::HashSet<u32> =
        funcs.iter().map(|f| f.addr).collect();

    for &target in &call_targets {
        if !existing_addrs.contains(&target) && target >= load_addr && target < end_addr {
            // Verify it looks like code (not data)
            if let Some(opcode) = space.read_u16_be(target) {
                let inst = ss_madou::sh2::decode(opcode);
                if !matches!(inst.kind, ss_madou::sh2::InstructionKind::Unknown { .. }) {
                    // Find RTS
                    let mut rts_addr = None;
                    let mut scan = target;
                    let max_scan = 2048u32;
                    while scan < end_addr && scan < target + max_scan {
                        if let Some(opc) = space.read_u16_be(scan) {
                            if opc == 0x000B {
                                rts_addr = Some(scan);
                                break;
                            }
                            if opc == 0x4F22 {
                                break; // hit another function's prologue
                            }
                        }
                        scan += 2;
                    }

                    let size = rts_addr.map(|rts| rts + 4 - target);
                    funcs.push(ScannedFunc {
                        addr: target,
                        regs_saved: Vec::new(),
                        saves_pr: false,
                        rts_addr,
                        size,
                    });
                }
            }
        }
    }

    // Sort by address
    funcs.sort_by_key(|f| f.addr);
    // Deduplicate (same address)
    funcs.dedup_by_key(|f| f.addr);

    // Output
    writeln!(out, "; SH-2 Function Scan Results")?;
    writeln!(out, "; Binary: 0x{load_addr:08X} - 0x{end_addr:08X} ({binary_len} bytes)")?;
    writeln!(out, "; Total functions found: {}", funcs.len())?;
    writeln!(out, ";")?;
    writeln!(
        out,
        "; {:>10}  {:>6}  {:>5}  {:>12}  {}",
        "Address", "Size", "Saves", "RTS@", "Registers"
    )?;
    writeln!(
        out,
        "; {:>10}  {:>6}  {:>5}  {:>12}  {}",
        "----------", "------", "-----", "------------", "---------"
    )?;

    let mut non_leaf = 0usize;
    let mut leaf = 0usize;
    let mut no_rts = 0usize;

    for f in &funcs {
        let regs_str = if f.regs_saved.is_empty() {
            String::from("-")
        } else {
            f.regs_saved
                .iter()
                .map(|r| format!("R{r}"))
                .collect::<Vec<_>>()
                .join(",")
        };

        let size_str = match f.size {
            Some(s) => format!("{s}"),
            None => String::from("?"),
        };

        let rts_str = match f.rts_addr {
            Some(rts) => format!("0x{rts:08X}"),
            None => String::from("-"),
        };

        let kind = if f.saves_pr { "PR" } else { "leaf" };

        writeln!(
            out,
            "  0x{:08X}  {:>6}  {:>5}  {:>12}  {}",
            f.addr, size_str, kind, rts_str, regs_str
        )?;

        if f.saves_pr {
            non_leaf += 1;
        } else {
            leaf += 1;
        }
        if f.rts_addr.is_none() {
            no_rts += 1;
        }
    }

    println!(
        "Function scan: {} total ({} non-leaf, {} leaf, {} without RTS) -> {}",
        funcs.len(),
        non_leaf,
        leaf,
        no_rts,
        "see output file"
    );

    Ok(())
}

/// Build and display a call graph from a function address.
/// Shows the function's callees (JSR/BSR targets) and optionally
/// recurses to show a tree of calls.
fn cmd_disasm_call_graph(
    space: &AddressSpace,
    load_addr: u32,
    binary_len: usize,
    root_addr: u32,
    pool_refs: &[(u32, u32, u32)],
    out: &mut dyn Write,
) -> Result<()> {
    let end_addr = load_addr + binary_len as u32;

    // Helper: disassemble a single function and find its callees
    fn find_callees(
        space: &AddressSpace,
        func_addr: u32,
        end_addr: u32,
        pool_refs: &[(u32, u32, u32)],
    ) -> Vec<(u32, u32)> {
        // Returns: Vec<(call_site_addr, target_addr)>
        let mut callees = Vec::new();
        let mut pc = func_addr;
        let max_scan = 4096u32;

        // Build a map: instruction_addr -> literal_pool_value for this function's range
        let mut pool_map: std::collections::HashMap<u32, (u32, u8)> = std::collections::HashMap::new();
        for &(inst_addr, _pool_addr, value) in pool_refs {
            if inst_addr >= func_addr && inst_addr < func_addr + max_scan {
                // Extract the register from the opcode
                if let Some(opcode) = space.read_u16_be(inst_addr) {
                    let reg = ((opcode >> 8) & 0xF) as u8;
                    pool_map.insert(inst_addr, (value, reg));
                }
            }
        }

        let mut found_prologue = false;
        while pc < end_addr && pc < func_addr + max_scan {
            if let Some(opcode) = space.read_u16_be(pc) {
                let inst = ss_madou::sh2::decode(opcode);

                // BSR: direct call with PC-relative offset
                if let Some(target) = inst.branch_target(pc) {
                    if matches!(inst.kind, ss_madou::sh2::InstructionKind::Bsr { .. }) {
                        callees.push((pc, target));
                    }
                }

                // JSR @Rn: resolve via nearby MOV.L @(disp,PC)
                if let ss_madou::sh2::InstructionKind::Jsr { rm } = &inst.kind {
                    // Look backward for MOV.L @(disp,PC),Rn that loaded this register
                    let target_reg = rm.0;
                    let mut search = pc.wrapping_sub(2);
                    let search_limit = pc.saturating_sub(40);
                    while search >= search_limit && search >= func_addr {
                        if let Some(&(value, reg)) = pool_map.get(&search) {
                            if reg == target_reg {
                                callees.push((pc, value));
                                break;
                            }
                        }
                        search = search.wrapping_sub(2);
                    }
                }

                // Stop at RTS (end of function)
                if opcode == 0x000B {
                    break;
                }
                // Stop if we hit another prologue (STS.L PR,@-R15) after the first
                if opcode == 0x4F22 {
                    if found_prologue {
                        break;
                    }
                    found_prologue = true;
                }
            }
            pc += 2;
        }

        callees
    }

    // BFS to build call tree (limited depth)
    let max_depth = 4u32;
    let mut visited: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut queue: std::collections::VecDeque<(u32, u32)> = std::collections::VecDeque::new();
    queue.push_back((root_addr, 0));

    writeln!(out, "; Call graph from 0x{root_addr:08X}")?;
    writeln!(out, "; Max depth: {max_depth}")?;
    writeln!(out, ";")?;

    let mut total_edges = 0usize;

    while let Some((func, depth)) = queue.pop_front() {
        if visited.contains(&func) || depth > max_depth {
            continue;
        }
        if func < load_addr || func >= end_addr {
            continue;
        }
        visited.insert(func);

        let indent = "  ".repeat(depth as usize);
        let callees = find_callees(space, func, end_addr, pool_refs);

        if callees.is_empty() {
            writeln!(out, "{indent}sub_{func:08X} (leaf)")?;
        } else {
            writeln!(out, "{indent}sub_{func:08X} ({} calls):", callees.len())?;
            for &(call_site, target) in &callees {
                let site_offset = call_site - load_addr;
                let already = visited.contains(&target);
                let marker = if already { " [seen]" } else { "" };
                writeln!(
                    out,
                    "{indent}  -> sub_{target:08X}  (from +0x{site_offset:05X}){marker}"
                )?;
                total_edges += 1;

                if !already && target >= load_addr && target < end_addr {
                    queue.push_back((target, depth + 1));
                }
            }
        }
    }

    println!(
        "Call graph from 0x{root_addr:08X}: {} functions, {} edges -> see output file",
        visited.len(),
        total_edges,
    );

    Ok(())
}

/// Extract all null-terminated ASCII strings from the binary.
fn cmd_disasm_strings(
    space: &AddressSpace,
    _load_addr: u32,
    _binary_len: usize,
    min_len: usize,
    pool_refs: &[(u32, u32, u32)],
    out: &mut dyn Write,
) -> Result<()> {
    let region = space.regions().first().context("No memory region loaded")?;
    let data = &region.data;
    let base = region.base_addr;

    // Collect all null-terminated printable ASCII strings
    let mut strings: Vec<(u32, String)> = Vec::new();
    let mut i = 0;
    while i < data.len() {
        // Check if this byte is printable ASCII (0x20-0x7E)
        if data[i] >= 0x20 && data[i] <= 0x7E {
            let start = i;
            // Scan forward while printable
            while i < data.len() && data[i] >= 0x20 && data[i] <= 0x7E {
                i += 1;
            }
            let len = i - start;
            // Check for null terminator and minimum length
            if len >= min_len && i < data.len() && data[i] == 0x00 {
                let s = String::from_utf8_lossy(&data[start..i]).into_owned();
                let addr = base + start as u32;
                strings.push((addr, s));
            }
        }
        i += 1;
    }

    // Build a set of literal pool values for fast reference lookup
    let mut ref_map: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for &(inst_addr, _pool_addr, value) in pool_refs {
        ref_map.entry(value).or_default().push(inst_addr);
    }

    // Write output
    writeln!(out, "; Strings in binary (min length: {min_len})")?;
    writeln!(out, "; {} strings found", strings.len())?;
    writeln!(out, ";")?;
    writeln!(
        out,
        "; {:>12}  {:<62}  {}",
        "Address", "String", "Reference"
    )?;
    writeln!(
        out,
        "; {:>12}  {:<62}  {}",
        "----------", "-----", "---------"
    )?;

    for (addr, s) in &strings {
        // Truncate long strings for display
        let display = if s.len() > 60 {
            format!("\"{}...\"", &s[..60])
        } else {
            format!("\"{}\"", s)
        };

        // Look up references
        let ref_str = if let Some(refs) = ref_map.get(addr) {
            refs.iter()
                .map(|r| format!("[ref: 0x{r:08X}]"))
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            "[no ref]".to_string()
        };

        writeln!(out, "0x{addr:08X}  {display:<64}  {ref_str}")?;
    }

    // Summary to stdout
    let ref_count = strings.iter().filter(|(addr, _)| ref_map.contains_key(addr)).count();
    println!(
        "Strings: {} found (min len {}), {} with literal pool refs -> see output file",
        strings.len(),
        min_len,
        ref_count,
    );

    Ok(())
}

/// Find all literal pool references to addresses within a memory range.
fn cmd_disasm_mem_refs(
    space: &AddressSpace,
    _load_addr: u32,
    _binary_len: usize,
    range_start: u32,
    range_end: u32,
    pool_refs: &[(u32, u32, u32)],
    out: &mut dyn Write,
) -> Result<()> {
    // 1. Filter to keep only those where range_start <= value < range_end
    let matching: Vec<_> = pool_refs
        .iter()
        .filter(|&&(_, _, val)| val >= range_start && val < range_end)
        .collect();

    // 3. Group by value, sorted by value (BTreeMap gives sorted keys)
    let mut grouped: std::collections::BTreeMap<u32, Vec<(u32, u32)>> =
        std::collections::BTreeMap::new();
    for &&(inst_addr, pool_addr, value) in &matching {
        grouped.entry(value).or_default().push((inst_addr, pool_addr));
    }

    // 4. Write output
    let total_refs = matching.len();
    let unique_values = grouped.len();

    writeln!(
        out,
        "; Memory range references: 0x{:08X} - 0x{:08X}",
        range_start, range_end
    )?;
    writeln!(
        out,
        "; {} references ({} unique values)",
        total_refs, unique_values
    )?;
    writeln!(out, ";")?;

    for (value, refs) in &grouped {
        writeln!(out, "; 0x{:08X} ({} refs):", value, refs.len())?;
        for &(inst_addr, pool_addr) in refs {
            if let Some(opcode) = space.read_u16_be(inst_addr) {
                let inst = ss_madou::sh2::decode(opcode);
                writeln!(
                    out,
                    ";   0x{:08X}: {}   pool@0x{:08X}",
                    inst_addr, inst, pool_addr
                )?;
            }
        }
        writeln!(out)?;
    }

    // 5. Print summary to stdout
    println!(
        "Memory range 0x{:08X}-0x{:08X}: {} references ({} unique values) -> see output file",
        range_start, range_end, total_refs, unique_values
    );

    Ok(())
}

/// Build reverse call graph: find all callers of a function, BFS upward.
fn cmd_disasm_callers(
    space: &AddressSpace,
    load_addr: u32,
    binary_len: usize,
    target_addr: u32,
    pool_refs: &[(u32, u32, u32)],
    out: &mut dyn Write,
) -> Result<()> {
    use std::collections::{HashMap, HashSet, VecDeque};

    let end_addr = load_addr + binary_len as u32;

    // Step 1: Build a complete call map by scanning the entire binary.
    // call_edges: Vec<(call_site_addr, callee_addr)>
    let mut call_edges: Vec<(u32, u32)> = Vec::new();

    // 1a. Collect MOV.L @(disp,PC) that load values in the code range,
    //     then use them to resolve JSR @Rn targets.

    // Build a map of instruction_addr -> (value, register) for pool loads
    // that point into the code range.
    let mut pool_map: HashMap<u32, (u32, u8)> = HashMap::new();
    for &(inst_addr, _pool_addr, value) in pool_refs {
        if value >= load_addr && value < end_addr {
            if let Some(opcode) = space.read_u16_be(inst_addr) {
                let reg = ((opcode >> 8) & 0xF) as u8;
                pool_map.insert(inst_addr, (value, reg));
            }
        }
    }

    // 1b. Scan entire binary for JSR and BSR instructions.
    let mut pc = load_addr;
    while pc < end_addr {
        if let Some(opcode) = space.read_u16_be(pc) {
            let inst = ss_madou::sh2::decode(opcode);

            match &inst.kind {
                ss_madou::sh2::InstructionKind::Bsr { .. } => {
                    if let Some(target) = inst.branch_target(pc) {
                        call_edges.push((pc, target));
                    }
                }
                ss_madou::sh2::InstructionKind::Jsr { rm } => {
                    // Resolve target by walking backward for MOV.L @(disp,PC),Rn
                    let target_reg = rm.0;
                    let mut search = pc.wrapping_sub(2);
                    let search_limit = pc.saturating_sub(40);
                    while search >= search_limit && search >= load_addr {
                        if let Some(&(value, reg)) = pool_map.get(&search) {
                            if reg == target_reg {
                                call_edges.push((pc, value));
                                break;
                            }
                        }
                        search = search.wrapping_sub(2);
                    }
                }
                _ => {}
            }
        }
        pc += 2;
    }

    // Step 2: Find containing function for each call site.
    // Walk backward from call_site looking for STS.L PR,@-R15 (0x4F22)
    // within 4096 bytes. If not found, use call_site itself.
    fn find_containing_func(space: &AddressSpace, call_site: u32, load_addr: u32) -> u32 {
        if let Some((func_start, _regs)) = find_prologue_backward(space, call_site, load_addr) {
            func_start
        } else {
            // Not found - use call_site as the "function" address
            call_site
        }
    }

    // Build reverse map: callee -> Vec<(call_site, caller_func)>
    let mut reverse_map: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
    for &(call_site, callee) in &call_edges {
        let caller_func = find_containing_func(space, call_site, load_addr);
        reverse_map
            .entry(callee)
            .or_default()
            .push((call_site, caller_func));
    }

    // Deduplicate entries per callee
    for entries in reverse_map.values_mut() {
        entries.sort();
        entries.dedup();
    }

    // Step 3: BFS upward from target_addr through callers, max depth 4.
    let max_depth = 4u32;

    writeln!(out, "; Callers of sub_{target_addr:08X} (depth {max_depth})")?;
    writeln!(out, ";")?;

    let mut visited: HashSet<u32> = HashSet::new();
    // Queue entries: (func_addr, call_site_addr, depth)
    let mut queue: VecDeque<(u32, u32, u32)> = VecDeque::new();
    let mut total_callers = 0usize;

    // Root node
    writeln!(out, "sub_{target_addr:08X}")?;
    visited.insert(target_addr);

    // Seed BFS with direct callers of target
    if let Some(callers) = reverse_map.get(&target_addr) {
        for &(call_site, caller_func) in callers {
            queue.push_back((caller_func, call_site, 1));
        }
    }

    while let Some((func_addr, call_site, depth)) = queue.pop_front() {
        if depth > max_depth {
            continue;
        }

        let already_seen = visited.contains(&func_addr);
        let indent = "  ".repeat(depth as usize);
        let marker = if already_seen { " [seen]" } else { "" };

        writeln!(
            out,
            "{indent}<- sub_{func_addr:08X} @ 0x{call_site:08X}{marker}",
        )?;
        total_callers += 1;

        if already_seen {
            continue;
        }
        visited.insert(func_addr);

        // Enqueue callers of this function
        if depth < max_depth {
            if let Some(callers) = reverse_map.get(&func_addr) {
                for &(cs, cf) in callers {
                    queue.push_back((cf, cs, depth + 1));
                }
            }
        }
    }

    println!(
        "Callers of 0x{target_addr:08X}: {} unique functions in call chain, {} total edges -> {}",
        visited.len() - 1, // exclude target itself
        total_callers,
        "see output file",
    );

    Ok(())
}
