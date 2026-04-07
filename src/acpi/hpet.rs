use crate::acpi::config::PlatformConfig;
use crate::acpi::header::{AcpiHeader, finalize_table};

const HPET_BASE_ADDRESS: u64 = 0xfed0_0000;
const HPET_EVENT_TIMER_BLOCK_ID: u32 = 0x8086_a201;

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

    let mut body = Vec::with_capacity(20);
    body.extend_from_slice(&HPET_EVENT_TIMER_BLOCK_ID.to_le_bytes());
    body.push(0);
    body.push(0);
    body.push(0);
    body.push(0);
    body.extend_from_slice(&HPET_BASE_ADDRESS.to_le_bytes());
    body.push(0);
    body.extend_from_slice(&0u16.to_le_bytes());
    body.push(0);

    finalize_table(header, &body)
}
