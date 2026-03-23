#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub blob_base_address: u64,
    pub front_padding: u32,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: [u8; 4],
    pub creator_revision: u32,
    pub sci_irq: u16,
    pub pm_io_base: u16,
    pub gpe0_base: u16,
    pub gpe0_len: u8,
    pub reset_io_base: u16,
    pub reset_value: u8,
    pub local_apic_address: u32,
    pub ioapic_id: u8,
    pub ioapic_address: u32,
    pub ioapic_gsi_base: u32,
    pub pci_ecam_base: u64,
    pub pci_segment: u16,
    pub pci_bus_start: u8,
    pub pci_bus_end: u8,
    pub cpu_apic_ids: Vec<u32>,
    pub pci_root_uid: u8,
}

impl PlatformConfig {
    pub fn intel_tdx_q35(cpu_count: u32) -> Self {
        let cpu_apic_ids = (0..cpu_count).collect();
        Self {
            blob_base_address: 0,
            front_padding: 64,
            oem_id: *b"TDXOEM",
            oem_table_id: *b"TDXQ35  ",
            oem_revision: 1,
            creator_id: *b"RUST",
            creator_revision: 1,
            sci_irq: 9,
            pm_io_base: 0x1800,
            gpe0_base: 0xafe0,
            gpe0_len: 8,
            reset_io_base: 0x0cf9,
            reset_value: 0x06,
            local_apic_address: 0xfee0_0000,
            ioapic_id: 0,
            ioapic_address: 0xfec0_0000,
            ioapic_gsi_base: 0,
            pci_ecam_base: 0xb000_0000,
            pci_segment: 0,
            pci_bus_start: 0,
            pci_bus_end: 0xff,
            cpu_apic_ids,
            pci_root_uid: 0,
        }
    }
}
