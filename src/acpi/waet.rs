use crate::acpi::config::PlatformConfig;
use crate::acpi::header::{AcpiHeader, finalize_table};

const WAET_FLAG_ACPI_PM_TIMER_GOOD: u32 = 1 << 1;

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

    finalize_table(header, &WAET_FLAG_ACPI_PM_TIMER_GOOD.to_le_bytes())
}
