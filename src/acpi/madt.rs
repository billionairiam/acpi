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
        body.push(0);
        body.push(8);
        body.push(uid as u8);
        body.push(apic_id as u8);
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
    body.push(config.sci_irq as u8);
    body.extend_from_slice(&u32::from(config.sci_irq).to_le_bytes());
    body.extend_from_slice(&0x000d_u16.to_le_bytes());

    append_compat_nmi_entries(&mut body, config);

    finalize_table(header, &body)
}

fn append_compat_nmi_entries(body: &mut Vec<u8>, config: &PlatformConfig) {
    let current_total_len = 36 + body.len() as u32;
    let Some(target_total_len) = config.target_madt_len() else {
        return;
    };
    if target_total_len <= current_total_len {
        return;
    }

    let extra = (target_total_len - current_total_len) as usize;
    let entry_count = extra / 6;
    for _ in 0..entry_count {
        body.push(4);
        body.push(6);
        body.push(0xff);
        body.extend_from_slice(&0x0005_u16.to_le_bytes());
        body.push(1);
    }
}
