use crate::acpi::config::PlatformConfig;
use crate::acpi::header::{AcpiHeader, finalize_table};

const MADT_FLAG_PCAT_COMPAT: u32 = 1;

pub fn build_madt(config: &PlatformConfig) -> Vec<u8> {
    let header = AcpiHeader {
        signature: *b"APIC",
        revision: 3,
        oem_id: config.oem_id,
        oem_table_id: config.oem_table_id,
        oem_revision: config.oem_revision,
        creator_id: config.creator_id,
        creator_revision: config.creator_revision,
    };

    let mut body = Vec::new();
    body.extend_from_slice(&config.local_apic_address.to_le_bytes());
    body.extend_from_slice(&MADT_FLAG_PCAT_COMPAT.to_le_bytes());

    for (uid, apic_id) in config.cpu_apic_ids.iter().copied().enumerate() {
        body.push(9);
        body.push(16);
        body.extend_from_slice(&[0u8; 2]);
        body.extend_from_slice(&(uid as u32).to_le_bytes());
        body.extend_from_slice(&apic_id.to_le_bytes());
        body.extend_from_slice(&1u32.to_le_bytes());
    }

    body.push(1);
    body.push(12);
    body.push(config.ioapic_id);
    body.push(0);
    body.extend_from_slice(&config.ioapic_address.to_le_bytes());
    body.extend_from_slice(&config.ioapic_gsi_base.to_le_bytes());

    body.push(2);
    body.push(10);
    body.push(0);
    body.push(0);
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&9u16.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes());

    finalize_table(header, &body)
}
