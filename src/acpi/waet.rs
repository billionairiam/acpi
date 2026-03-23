use crate::acpi::config::PlatformConfig;
use crate::acpi::header::{AcpiHeader, finalize_table};

pub fn build_waet(config: &PlatformConfig) -> Vec<u8> {
    let header = AcpiHeader {
        signature: *b"WAET",
        revision: 1,
        oem_id: config.oem_id,
        oem_table_id: config.oem_table_id,
        oem_revision: config.oem_revision,
        creator_id: config.creator_id,
        creator_revision: config.creator_revision,
    };

    let mut body = Vec::new();
    body.extend_from_slice(&(1u32 << 1).to_le_bytes());
    finalize_table(header, &body)
}
