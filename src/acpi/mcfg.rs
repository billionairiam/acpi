use crate::acpi::config::PlatformConfig;
use crate::acpi::header::{AcpiHeader, finalize_table};

pub fn build_mcfg(config: &PlatformConfig) -> Vec<u8> {
    let header = AcpiHeader {
        signature: *b"MCFG",
        revision: 1,
        oem_id: config.oem_id,
        oem_table_id: config.oem_table_id,
        oem_revision: config.oem_revision,
        creator_id: config.creator_id,
        creator_revision: config.creator_revision,
    };

    let mut body = Vec::new();
    body.extend_from_slice(&[0u8; 8]);
    body.extend_from_slice(&config.pci_ecam_base.to_le_bytes());
    body.extend_from_slice(&config.pci_segment.to_le_bytes());
    body.push(config.pci_bus_start);
    body.push(config.pci_bus_end);
    body.extend_from_slice(&[0u8; 4]);
    finalize_table(header, &body)
}
