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
    patch_u16(&mut body, 10, config.sci_irq);
    patch_u16(&mut body, 20, config.pm_io_base);
    patch_u16(&mut body, 22, config.pm_io_base + 0x04);
    patch_u16(&mut body, 26, config.pm_io_base + 0x08);
    patch_u16(&mut body, 32, config.gpe0_base);
    body[36] = 4;
    body[37] = 2;
    body[39] = 4;
    body[40] = config.gpe0_len;
    patch_u16(&mut body, 45, 0x0fff);
    patch_u16(&mut body, 47, 0x0fff);
    body[52] = 0x32;
    body[53] = config.reset_value;
    patch_u32(
        &mut body,
        56,
        FADT_FLAG_WBINVD
            | FADT_FLAG_PROC_C1
            | FADT_FLAG_SLP_BUTTON
            | FADT_FLAG_RTC_S4
            | FADT_FLAG_RESET_REG_SUP
            | FADT_FLAG_USE_PLATFORM_CLOCK,
    );
    patch_gas(&mut body, 76, Gas::io(32, config.pm_io_base));
    patch_gas(&mut body, 88, Gas::io(16, config.pm_io_base + 0x04));
    patch_gas(&mut body, 100, Gas::io(32, config.pm_io_base + 0x08));
    patch_gas(
        &mut body,
        124,
        Gas::io((config.gpe0_len.saturating_mul(8)).max(8), config.gpe0_base),
    );
    patch_gas(&mut body, 148, Gas::io(8, config.reset_io_base));
    patch_u64(&mut body, 140, 0);
    patch_u64(&mut body, 148 + 16, 0);
    patch_u64(&mut body, 104 + 56, dsdt_address);
    patch_u16(&mut body, 109, 0);
    patch_u32(&mut body, 4, dsdt_address as u32);

    body[8] = 0;
    body[9] = 0;

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
