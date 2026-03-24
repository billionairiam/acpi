mod acpi;
mod vm;

use std::env;
use std::fs;

use acpi::build_minimal_acpi;
use acpi::config::PlatformConfig;
use vm::VmConfig;

fn main() {
    let mut args = env::args().skip(1);
    let first = args.next();
    let second = args.next();

    if let Some(path) = first.as_deref().filter(|value| value.ends_with(".toml")) {
        let vm = VmConfig::load(path).unwrap_or_else(|error| panic!("{error}"));
        let built = build_minimal_acpi(&vm.platform_config());

        println!("qemu_binary: {}", vm.qemu.binary);
        for line in format_qemu_args(&vm.qemu_args()) {
            println!("{line}");
        }
        print_layout(&built);

        if let Some(output) = second {
            write_blob(&built.blob.data, &built.rsdp, &output);
        }
        return;
    }

    let output = first;
    let cpu_count = second
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(7);
    let built = build_minimal_acpi(&PlatformConfig::intel_tdx_q35(cpu_count));

    if let Some(path) = output {
        write_blob(&built.blob.data, &built.rsdp, &path);
    }
    print_layout(&built);
}

fn format_qemu_args(args: &[String]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let current = &args[index];
        if current.starts_with('-') && index + 1 < args.len() && !args[index + 1].starts_with('-') {
            lines.push(format!("\"{}\" \"{}\"", current, args[index + 1]));
            index += 2;
        } else {
            lines.push(format!("\"{}\"", current));
            index += 1;
        }
    }
    lines
}

fn write_blob(blob: &[u8], rsdp: &[u8], path: &str) {
    fs::write(path, blob).expect("failed to write ACPI blob");
    let rsdp_path = format!("{path}.rsdp");
    fs::write(&rsdp_path, rsdp).expect("failed to write RSDP");
    println!("blob={path}");
    println!("rsdp={rsdp_path}");
}

fn print_layout(built: &acpi::BuiltBlob) {
    for name in [
        "DSDT", "FACP", "APIC", "HPET", "TCPA", "TPM2", "SRAT", "SLIT", "HMAT", "MCFG", "DMAR",
        "IVRS", "VIOT", "NFIT", "CEDT", "WAET", "RSDT",
    ] {
        if let Some(layout) = built.layout.get(name) {
            let lower = name.to_ascii_lowercase();
            println!(
                "{lower}_offset: {}, {lower}_csum: {}, {lower}_len: {}",
                layout.offset, layout.checksum_offset, layout.length
            );
        }
    }
}
