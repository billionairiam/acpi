use crate::acpi::checksum::acpi_checksum;
use crate::acpi::config::PlatformConfig;

pub fn build_rsdp(config: &PlatformConfig, rsdt_address: u64) -> Vec<u8> {
    let mut rsdp = Vec::with_capacity(20);
    rsdp.extend_from_slice(b"RSD PTR ");
    rsdp.push(0);
    rsdp.extend_from_slice(&config.oem_id);
    rsdp.push(0);
    rsdp.extend_from_slice(&(rsdt_address as u32).to_le_bytes());
    rsdp[8] = acpi_checksum(&rsdp);
    rsdp
}
