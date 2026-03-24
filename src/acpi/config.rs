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
    pub pci_irq_base: u32,
    pub has_hpet: bool,
    pub pci_ecam_base: u64,
    pub pci_segment: u16,
    pub pci_bus_start: u8,
    pub pci_bus_end: u8,
    pub cpu_apic_ids: Vec<u32>,
    pub pci_root_uid: u8,
    pub pci_devices: Vec<PciDeviceConfig>,
    pub compat_lengths: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PciDeviceConfig {
    pub driver: String,
    pub id: Option<String>,
    pub bus: Option<String>,
    pub devfn: u8,
}

impl PciDeviceConfig {
    pub fn is_root_bus_device(&self) -> bool {
        self.bus
            .as_deref()
            .map(|bus| bus == "pcie.0")
            .unwrap_or(true)
    }
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
            pci_irq_base: 16,
            has_hpet: false,
            pci_ecam_base: 0xb000_0000,
            pci_segment: 0,
            pci_bus_start: 0,
            pci_bus_end: 0xff,
            cpu_apic_ids,
            pci_root_uid: 0,
            pci_devices: Vec::new(),
            compat_lengths: true,
        }
    }

    pub fn target_dsdt_len(&self) -> Option<u32> {
        if !self.compat_lengths {
            return None;
        }

        let cpus = self.cpu_apic_ids.len() as u32;
        let base = match cpus {
            1 => 8_031,
            2 => 8_028,
            3..=16 => 8_029,
            _ => 8_031,
        };
        let hpet_delta = if self.has_hpet { 142 } else { 0 };
        let pci_delta = 17 * self.pci_devices.len() as u32;
        Some(base + 86 * cpus + hpet_delta + pci_delta)
    }

    pub fn target_madt_len(&self) -> Option<u32> {
        self.compat_lengths
            .then_some(222 + 8 * self.cpu_apic_ids.len() as u32)
    }
}
