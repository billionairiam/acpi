mod acpi;

use std::env;
use std::fs;

use acpi::build_minimal_acpi;
use acpi::config::PlatformConfig;

fn main() {
    let mut args = env::args().skip(1);
    let output = args.next();
    let cpu_count = args
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(4);

    let built = build_minimal_acpi(&PlatformConfig::intel_tdx_q35(cpu_count));

    if let Some(path) = output {
        fs::write(&path, &built.blob.data).expect("failed to write ACPI blob");
        let rsdp_path = format!("{path}.rsdp");
        fs::write(&rsdp_path, &built.rsdp).expect("failed to write RSDP");
        println!("blob={path}");
        println!("rsdp={rsdp_path}");
    }

    for (name, layout) in &built.layout {
        println!(
            "{name}_offset: {}, {name}_csum: {}, {name}_len: {}",
            layout.offset, layout.checksum_offset, layout.length
        );
    }
}
