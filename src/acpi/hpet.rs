use crate::acpi::config::PlatformConfig;
use crate::acpi::header::{AcpiHeader, finalize_table};

const HPET_BASE: u64 = 0xfed0_0000;

pub fn build_hpet(config: &PlatformConfig) -> Vec<u8> {
    let header = AcpiHeader {
        signature: *b"HPET",
        revision: 1,
        oem_id: config.oem_id,
        oem_table_id: config.oem_table_id,
        oem_revision: config.oem_revision,
        creator_id: config.creator_id,
        creator_revision: config.creator_revision,
    };

    let mut body = Vec::new();
    body.extend_from_slice(&0x8086_a201_u32.to_le_bytes());
    body.push(0);
    body.push(0);
    body.push(0);
    body.push(0);
    body.extend_from_slice(&HPET_BASE.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&0u16.to_le_bytes());
    body.push(0);
    finalize_table(header, &body)
}
