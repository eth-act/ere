// This is ere-risc0/build_script_template.rs
// This script will be temporarily copied as build.rs into the target methods crate.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(Debug)]
struct GuestMethodInfo {
    name: String,
    elf_path: String, // Path to the ELF in OUT_DIR, as determined by risc0_build
    image_id_hex: String,
}

fn main() {
    let guest_entries = risc0_build::embed_methods();

    if guest_entries.is_empty() {
        eprintln!("ere Risc0 Template Build: risc0_build::embed_methods() found no guest methods.");
        return;
    }

    let entry = &guest_entries[0]; // For simplicity, take the first guest
    let info = GuestMethodInfo {
        name: entry.name.to_string(),
        elf_path: entry.path.to_string(), // This path is to the ELF in OUT_DIR
        image_id_hex: entry
            .image_id
            .as_bytes()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect(),
    };

    // Output the info to a known file directly in the methods crate directory.
    let manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set for template build.rs");
    let info_file_path = Path::new(&manifest_dir).join("ere_guest_info.json");

    let json_output = format!(
        r#"{{
    "name": "{}",
    "elf_path": "{}",
    "image_id_hex": "{}"
}}"#,
        info.name.replace('\\', "\\\\").replace('"', "\\\""),
        info.elf_path.replace('\\', "\\\\").replace('"', "\\\""),
        info.image_id_hex
    );

    let mut file = File::create(&info_file_path)
        .expect("Template build.rs: Failed to create ere_guest_info.json in manifest dir");
    file.write_all(json_output.as_bytes())
        .expect("Template build.rs: Failed to write to ere_guest_info.json in manifest dir");

    println!("cargo:rerun-if-changed=build.rs");
    eprintln!(
        "ere Risc0 Template Build: Guest info written to {:?}",
        info_file_path
    );
}
