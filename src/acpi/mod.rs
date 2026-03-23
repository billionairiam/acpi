pub mod blob;
pub mod checksum;
pub mod config;
pub mod dsdt;
pub mod fadt;
pub mod header;
pub mod madt;
pub mod mcfg;
pub mod rsdp;
pub mod rsdt;
pub mod waet;

use std::collections::BTreeMap;

use blob::{AcpiBlob, AcpiBlobBuilder};
use config::PlatformConfig;
use dsdt::build_dsdt;
use fadt::build_fadt;
use madt::build_madt;
use mcfg::build_mcfg;
use rsdp::build_rsdp;
use rsdt::build_rsdt;
use waet::build_waet;

#[derive(Debug, Clone)]
pub struct BuiltBlob {
    pub blob: AcpiBlob,
    pub rsdp: Vec<u8>,
    pub layout: BTreeMap<String, TableLayout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableLayout {
    pub offset: u32,
    pub length: u32,
    pub checksum_offset: u32,
}

pub fn build_minimal_acpi(config: &PlatformConfig) -> BuiltBlob {
    let mut blob = AcpiBlobBuilder::new(config.blob_base_address);
    blob.reserve_front(config.front_padding as usize);

    let dsdt = build_dsdt(config);
    let dsdt_offset = blob.append_table(*b"DSDT", dsdt);

    let fadt = build_fadt(config, blob.address_of(dsdt_offset));
    let fadt_offset = blob.append_table(*b"FACP", fadt);

    let madt = build_madt(config);
    let madt_offset = blob.append_table(*b"APIC", madt);

    let mcfg = build_mcfg(config);
    let mcfg_offset = blob.append_table(*b"MCFG", mcfg);

    let waet = build_waet(config);
    let waet_offset = blob.append_table(*b"WAET", waet);

    let rsdt_entries = [
        blob.address_of(fadt_offset),
        blob.address_of(madt_offset),
        blob.address_of(mcfg_offset),
        blob.address_of(waet_offset),
    ];
    let rsdt = build_rsdt(config, &rsdt_entries);
    let rsdt_offset = blob.append_table(*b"RSDT", rsdt);

    let rsdp = build_rsdp(config, blob.address_of(rsdt_offset));
    let blob = blob.finish();

    let mut layout = BTreeMap::new();
    for table in &blob.tables {
        layout.insert(
            String::from_utf8_lossy(&table.signature).into_owned(),
            TableLayout {
                offset: table.offset,
                length: table.length,
                checksum_offset: table.offset + 9,
            },
        );
    }

    BuiltBlob { blob, rsdp, layout }
}

#[cfg(test)]
mod tests {
    use super::build_minimal_acpi;
    use crate::acpi::config::PlatformConfig;

    #[test]
    fn builds_expected_minimal_layout() {
        let built = build_minimal_acpi(&PlatformConfig::intel_tdx_q35(4));
        assert_eq!(built.layout["DSDT"].offset, 64);
        assert_eq!(built.layout["MCFG"].length, 60);
        assert_eq!(built.layout["WAET"].length, 40);
        assert_eq!(built.layout["RSDT"].length, 52);
        assert_eq!(built.rsdp.len(), 20);
    }

    #[test]
    fn rsdt_points_at_other_tables() {
        let built = build_minimal_acpi(&PlatformConfig::intel_tdx_q35(2));
        let rsdt = &built.blob.data[built.layout["RSDT"].offset as usize..];
        let payload = &rsdt[36..52];
        let mut entries = Vec::new();
        for chunk in payload.chunks_exact(4) {
            entries.push(u32::from_le_bytes(chunk.try_into().unwrap()));
        }
        assert_eq!(entries[0], built.layout["FACP"].offset);
        assert_eq!(entries[1], built.layout["APIC"].offset);
        assert_eq!(entries[2], built.layout["MCFG"].offset);
        assert_eq!(entries[3], built.layout["WAET"].offset);
    }
}
