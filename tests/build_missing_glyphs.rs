//! Missing glyphs must be rejected before opening or writing the disc.
use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn incomplete_glyph_assignment_stops_before_disc_access() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "madou-glyph-demand-{}-{unique}",
        std::process::id()
    ));
    let scripts = root.join("scripts/needs_review");
    fs::create_dir_all(&scripts).unwrap();
    let text: String = (0xac00..0xac00 + 1200)
        .map(|c| char::from_u32(c).unwrap())
        .collect();
    fs::write(
        scripts.join("MP0001.json"),
        serde_json::to_vec(&serde_json::json!({
            "source":"MP0001.SEQ", "source_md5":"", "entries":[{
                "id":"MP0001_0000", "offset":"0x0000", "raw_hex":"FF 00",
                "text":"{ctrl:FF00}", "ko":text, "status":"needs_human_review", "notes":""
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let output_path = root.join("result.bin");
    let output = Command::new(env!("CARGO_BIN_EXE_ss_madou"))
        .args(["build-rom", "--translations-dir"])
        .arg(root.join("scripts"))
        .arg("--rom")
        .arg(root.join("absent-source.bin"))
        .arg("--output")
        .arg(&output_path)
        .output()
        .unwrap();
    let wrote_artifact = ["bin", "cue", "bps"]
        .iter()
        .any(|ext| output_path.with_extension(ext).exists());
    fs::remove_dir_all(&root).unwrap();
    assert!(!wrote_artifact);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Glyph allocation incomplete"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Loading ROM:"));
    // The primary assertion above proves the allocation gate, even though the
    // source ROM is absent. It cannot pass through a later I/O failure.
}
