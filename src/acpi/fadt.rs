use crate::acpi::config::PlatformConfig;
use crate::acpi::header::{AcpiHeader, finalize_table};

#[derive(Debug, Clone, Copy)]
struct Gas {
    space_id: u8,
    bit_width: u8,
    bit_offset: u8,
    access_size: u8,
    address: u64,
}

impl Gas {
    fn io(bit_width: u8, address: u16) -> Self {
        Self {
            space_id: 1,
            bit_width,
            bit_offset: 0,
            access_size: if bit_width <= 8 {
                1
            } else if bit_width <= 16 {
                2
            } else if bit_width <= 32 {
                3
            } else {
                4
            },
            address: u64::from(address),
        }
    }

    fn encode(self, out: &mut Vec<u8>) {
        out.push(self.space_id);
        out.push(self.bit_width);
        out.push(self.bit_offset);
        out.push(self.access_size);
        out.extend_from_slice(&self.address.to_le_bytes());
    }
}

const FADT_FLAG_WBINVD: u32 = 1 << 0;
const FADT_FLAG_PROC_C1: u32 = 1 << 2;
const FADT_FLAG_SLP_BUTTON: u32 = 1 << 5;
const FADT_FLAG_RTC_S4: u32 = 1 << 7;
const FADT_FLAG_RESET_REG_SUP: u32 = 1 << 10;
const FADT_FLAG_USE_PLATFORM_CLOCK: u32 = 1 << 15;
const FADT_BODY_LEN: usize = 208;
const FADT_OFFSET_DSDT: usize = 4;
const FADT_OFFSET_SCI_INT: usize = 10;
const FADT_OFFSET_PM1A_EVT_BLK: usize = 20;
const FADT_OFFSET_PM1A_CNT_BLK: usize = 28;
const FADT_OFFSET_PM_TMR_BLK: usize = 40;
const FADT_OFFSET_GPE0_BLK: usize = 44;
const FADT_OFFSET_PM1_EVT_LEN: usize = 52;
const FADT_OFFSET_PM1_CNT_LEN: usize = 53;
const FADT_OFFSET_PM_TMR_LEN: usize = 55;
const FADT_OFFSET_GPE0_BLK_LEN: usize = 56;
const FADT_OFFSET_P_LVL2_LAT: usize = 60;
const FADT_OFFSET_P_LVL3_LAT: usize = 62;
const FADT_OFFSET_CENTURY: usize = 72;
const FADT_OFFSET_IAPC_BOOT_ARCH: usize = 74;
const FADT_OFFSET_FLAGS: usize = 76;
const FADT_OFFSET_RESET_REG: usize = 80;
const FADT_OFFSET_RESET_VALUE: usize = 92;
const FADT_OFFSET_X_FIRMWARE_CTRL: usize = 96;
const FADT_OFFSET_X_DSDT: usize = 104;
const FADT_OFFSET_X_PM1A_EVT_BLK: usize = 112;
const FADT_OFFSET_X_PM1A_CNT_BLK: usize = 136;
const FADT_OFFSET_X_PM_TMR_BLK: usize = 172;
const FADT_OFFSET_X_GPE0_BLK: usize = 184;
const IAPC_BOOT_ARCH_8042: u16 = 1 << 1;

pub fn build_fadt(config: &PlatformConfig, dsdt_address: u64) -> Vec<u8> {
    let header = AcpiHeader {
        signature: *b"FACP",
        revision: 6,
        oem_id: config.oem_id,
        oem_table_id: config.oem_table_id,
        oem_revision: config.oem_revision,
        creator_id: config.creator_id,
        creator_revision: config.creator_revision,
    };

    let mut body = vec![0u8; FADT_BODY_LEN];
    patch_u32(&mut body, FADT_OFFSET_DSDT, dsdt_address as u32);
    patch_u16(&mut body, FADT_OFFSET_SCI_INT, config.sci_irq);
    patch_u32(
        &mut body,
        FADT_OFFSET_PM1A_EVT_BLK,
        u32::from(config.pm_io_base),
    );
    patch_u32(
        &mut body,
        FADT_OFFSET_PM1A_CNT_BLK,
        u32::from(config.pm_io_base + 0x04),
    );
    patch_u32(
        &mut body,
        FADT_OFFSET_PM_TMR_BLK,
        u32::from(config.pm_io_base + 0x08),
    );
    patch_u32(&mut body, FADT_OFFSET_GPE0_BLK, u32::from(config.gpe0_base));
    body[FADT_OFFSET_PM1_EVT_LEN] = 4;
    body[FADT_OFFSET_PM1_CNT_LEN] = 2;
    body[FADT_OFFSET_PM_TMR_LEN] = 4;
    body[FADT_OFFSET_GPE0_BLK_LEN] = config.gpe0_len;
    patch_u16(&mut body, FADT_OFFSET_P_LVL2_LAT, 0x0fff);
    patch_u16(&mut body, FADT_OFFSET_P_LVL3_LAT, 0x0fff);
    body[FADT_OFFSET_CENTURY] = 0x32;
    patch_u16(&mut body, FADT_OFFSET_IAPC_BOOT_ARCH, IAPC_BOOT_ARCH_8042);
    patch_u32(
        &mut body,
        FADT_OFFSET_FLAGS,
        FADT_FLAG_WBINVD
            | FADT_FLAG_PROC_C1
            | FADT_FLAG_SLP_BUTTON
            | FADT_FLAG_RTC_S4
            | FADT_FLAG_RESET_REG_SUP
            | FADT_FLAG_USE_PLATFORM_CLOCK,
    );
    patch_gas(
        &mut body,
        FADT_OFFSET_RESET_REG,
        Gas::io(8, config.reset_io_base),
    );
    body[FADT_OFFSET_RESET_VALUE] = config.reset_value;
    patch_u64(&mut body, FADT_OFFSET_X_FIRMWARE_CTRL, 0);
    patch_u64(&mut body, FADT_OFFSET_X_DSDT, dsdt_address);
    patch_gas(
        &mut body,
        FADT_OFFSET_X_PM1A_EVT_BLK,
        Gas::io(32, config.pm_io_base),
    );
    patch_gas(
        &mut body,
        FADT_OFFSET_X_PM1A_CNT_BLK,
        Gas::io(16, config.pm_io_base + 0x04),
    );
    patch_gas(
        &mut body,
        FADT_OFFSET_X_PM_TMR_BLK,
        Gas::io(32, config.pm_io_base + 0x08),
    );
    patch_gas(
        &mut body,
        FADT_OFFSET_X_GPE0_BLK,
        Gas::io((config.gpe0_len.saturating_mul(8)).max(8), config.gpe0_base),
    );

    finalize_table(header, &body)
}

fn patch_u16(body: &mut [u8], offset: usize, value: u16) {
    body[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn patch_u32(body: &mut [u8], offset: usize, value: u32) {
    body[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn patch_u64(body: &mut [u8], offset: usize, value: u64) {
    body[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn patch_gas(body: &mut [u8], offset: usize, gas: Gas) {
    let mut encoded = Vec::with_capacity(12);
    gas.encode(&mut encoded);
    body[offset..offset + 12].copy_from_slice(&encoded);
}
