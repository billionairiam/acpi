pub mod blob;
pub mod checksum;
pub mod config;
pub mod dsdt;
pub mod fadt;
pub mod header;
pub mod madt;
pub mod mcfg;
pub mod rsdp;
pub mod rsdt;

use std::collections::BTreeMap;

use blob::{AcpiBlob, AcpiBlobBuilder};
use config::{IommuKind, PlatformConfig, TpmKind};
use dsdt::build_dsdt;
use fadt::build_fadt;
use madt::build_madt;
use mcfg::build_mcfg;
use rsdp::build_rsdp;
use rsdt::build_rsdt;

#[derive(Debug, Clone)]
pub struct BuiltBlob {
    pub blob: AcpiBlob,
    pub rsdp: Vec<u8>,
    pub layout: BTreeMap<String, TableLayout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableLayout {
    pub offset: u32,
    pub length: u32,
    pub checksum_offset: u32,
}

fn optional_table_len(signature: [u8; 4], config: &PlatformConfig) -> u32 {
    match &signature {
        b"HPET" => 56,
        b"TCPA" => 50,
        b"TPM2" => 52,
        b"SRAT" => {
            let cpu_len = config
                .cpu_apic_ids
                .iter()
                .map(|apic_id| if *apic_id < 255 { 16 } else { 24 })
                .sum::<u32>();
            48 + cpu_len
        }
        b"SLIT" => 45,
        b"HMAT" => 40,
        b"MCFG" => 60,
        b"DMAR" => 60,
        b"IVRS" => 64,
        b"VIOT" => 48,
        b"NFIT" => 40,
        b"CEDT" => 36,
        b"WAET" => 40,
        _ => 36,
    }
}

pub fn build_minimal_acpi(config: &PlatformConfig) -> BuiltBlob {
    let mut blob = AcpiBlobBuilder::new(config.blob_base_address);
    blob.reserve_front(config.front_padding as usize);

    let dsdt = build_dsdt(config);
    let dsdt_offset = blob.append_table(*b"DSDT", dsdt);

    let fadt = build_fadt(config, blob.address_of(dsdt_offset));
    let fadt_offset = blob.append_table(*b"FACP", fadt);

    let madt = build_madt(config);
    let madt_offset = blob.append_table(*b"APIC", madt);

    let hpet_offset = if config.has_hpet {
        Some(blob.append_placeholder_table(*b"HPET", optional_table_len(*b"HPET", config)))
    } else {
        None
    };

    let tcpa_offset = match config.tpm_kind {
        Some(TpmKind::Tcpa) => {
            Some(blob.append_placeholder_table(*b"TCPA", optional_table_len(*b"TCPA", config)))
        }
        Some(TpmKind::Tpm2) => {
            Some(blob.append_placeholder_table(*b"TPM2", optional_table_len(*b"TPM2", config)))
        }
        None => None,
    };

    let srat_offset = if config.has_numa {
        Some(blob.append_placeholder_table(*b"SRAT", optional_table_len(*b"SRAT", config)))
    } else {
        None
    };

    let slit_offset = if config.has_slit {
        Some(blob.append_placeholder_table(*b"SLIT", optional_table_len(*b"SLIT", config)))
    } else {
        None
    };

    let hmat_offset = if config.has_hmat {
        Some(blob.append_placeholder_table(*b"HMAT", optional_table_len(*b"HMAT", config)))
    } else {
        None
    };

    let mcfg_offset = if config.has_mcfg {
        Some(blob.append_table(*b"MCFG", build_mcfg(config)))
    } else {
        None
    };

    let iommu_offset = match config.iommu_kind {
        Some(IommuKind::Dmar) => {
            Some(blob.append_placeholder_table(*b"DMAR", optional_table_len(*b"DMAR", config)))
        }
        Some(IommuKind::Ivrs) => {
            Some(blob.append_placeholder_table(*b"IVRS", optional_table_len(*b"IVRS", config)))
        }
        Some(IommuKind::Viot) => {
            Some(blob.append_placeholder_table(*b"VIOT", optional_table_len(*b"VIOT", config)))
        }
        None => None,
    };

    let nfit_offset = if config.nvdimm_enabled {
        Some(blob.append_placeholder_table(*b"NFIT", optional_table_len(*b"NFIT", config)))
    } else {
        None
    };

    let cedt_offset = if config.cxl_enabled {
        Some(blob.append_placeholder_table(*b"CEDT", optional_table_len(*b"CEDT", config)))
    } else {
        None
    };

    let waet_offset = blob.append_placeholder_table(*b"WAET", optional_table_len(*b"WAET", config));

    let mut rsdt_entries = vec![blob.address_of(fadt_offset), blob.address_of(madt_offset)];
    if let Some(hpet_offset) = hpet_offset {
        rsdt_entries.push(blob.address_of(hpet_offset));
    }
    if let Some(tcpa_offset) = tcpa_offset {
        rsdt_entries.push(blob.address_of(tcpa_offset));
    }
    if let Some(srat_offset) = srat_offset {
        rsdt_entries.push(blob.address_of(srat_offset));
    }
    if let Some(slit_offset) = slit_offset {
        rsdt_entries.push(blob.address_of(slit_offset));
    }
    if let Some(hmat_offset) = hmat_offset {
        rsdt_entries.push(blob.address_of(hmat_offset));
    }
    if let Some(mcfg_offset) = mcfg_offset {
        rsdt_entries.push(blob.address_of(mcfg_offset));
    }
    if let Some(iommu_offset) = iommu_offset {
        rsdt_entries.push(blob.address_of(iommu_offset));
    }
    if let Some(nfit_offset) = nfit_offset {
        rsdt_entries.push(blob.address_of(nfit_offset));
    }
    if let Some(cedt_offset) = cedt_offset {
        rsdt_entries.push(blob.address_of(cedt_offset));
    }
    rsdt_entries.push(blob.address_of(waet_offset));
    let rsdt = build_rsdt(config, &rsdt_entries);
    let rsdt_offset = blob.append_table(*b"RSDT", rsdt);

    let rsdp = build_rsdp(config, blob.address_of(rsdt_offset));
    let blob = blob.finish();

    let mut layout = BTreeMap::new();
    for table in &blob.tables {
        layout.insert(
            String::from_utf8_lossy(&table.signature).into_owned(),
            TableLayout {
                offset: table.offset,
                length: table.length,
                checksum_offset: table.offset + 9,
            },
        );
    }

    BuiltBlob { blob, rsdp, layout }
}

#[cfg(test)]
mod tests {
    use super::build_minimal_acpi;
    use crate::acpi::config::PlatformConfig;

    #[test]
    fn builds_expected_minimal_layout() {
        let built = build_minimal_acpi(&PlatformConfig::intel_tdx_q35(4));
        assert_eq!(built.layout["DSDT"].offset, 64);
        assert_eq!(built.layout["MCFG"].length, 60);
        assert_eq!(built.layout["WAET"].length, 40);
        assert_eq!(built.layout["RSDT"].length, 52);
        assert_eq!(built.rsdp.len(), 20);
    }

    #[test]
    fn rsdt_points_at_other_tables() {
        let built = build_minimal_acpi(&PlatformConfig::intel_tdx_q35(2));
        let rsdt = &built.blob.data[built.layout["RSDT"].offset as usize..];
        let payload = &rsdt[36..52];
        let mut entries = Vec::new();
        for chunk in payload.chunks_exact(4) {
            entries.push(u32::from_le_bytes(chunk.try_into().unwrap()));
        }
        assert_eq!(entries[0], built.layout["FACP"].offset);
        assert_eq!(entries[1], built.layout["APIC"].offset);
        assert_eq!(entries[2], built.layout["MCFG"].offset);
        assert_eq!(entries[3], built.layout["WAET"].offset);
    }

    #[test]
    fn optional_tables_follow_config_switches() {
        let mut config = PlatformConfig::intel_tdx_q35(2);
        config.has_hpet = true;
        config.tpm_kind = Some(crate::acpi::config::TpmKind::Tpm2);
        config.has_numa = true;
        config.has_slit = true;
        config.has_hmat = true;
        config.iommu_kind = Some(crate::acpi::config::IommuKind::Dmar);
        config.nvdimm_enabled = true;
        config.cxl_enabled = true;

        let built = build_minimal_acpi(&config);
        for name in [
            "HPET", "TPM2", "SRAT", "SLIT", "HMAT", "MCFG", "DMAR", "NFIT", "CEDT",
        ] {
            assert!(built.layout.contains_key(name), "missing {name}");
        }
    }

    #[test]
    fn can_disable_mcfg() {
        let mut config = PlatformConfig::intel_tdx_q35(2);
        config.has_mcfg = false;
        let built = build_minimal_acpi(&config);
        assert!(!built.layout.contains_key("MCFG"));
        assert_eq!(built.layout["RSDT"].length, 48);
    }

    #[test]
    fn hpet_adds_table_and_expands_rsdt() {
        let mut config = PlatformConfig::intel_tdx_q35(2);
        config.has_hpet = true;
        let built = build_minimal_acpi(&config);
        assert_eq!(built.layout["HPET"].length, 56);
        assert_eq!(built.layout["RSDT"].length, 56);
    }

    #[test]
    fn fadt_references_dsdt_in_both_fields() {
        let built = build_minimal_acpi(&PlatformConfig::intel_tdx_q35(2));
        let fadt_offset = built.layout["FACP"].offset as usize;
        let fadt =
            &built.blob.data[fadt_offset..fadt_offset + built.layout["FACP"].length as usize];
        let dsdt32 = u32::from_le_bytes(fadt[40..44].try_into().unwrap());
        let x_dsdt = u64::from_le_bytes(fadt[140..148].try_into().unwrap());
        assert_eq!(dsdt32, built.layout["DSDT"].offset);
        assert_eq!(x_dsdt, u64::from(built.layout["DSDT"].offset));
    }

    #[test]
    fn compat_length_model_matches_48_and_64_vcpus() {
        let built48 = build_minimal_acpi(&PlatformConfig::intel_tdx_q35(48));
        assert_eq!(built48.layout["DSDT"].length, 12_159);
        assert_eq!(built48.layout["APIC"].length, 606);

        let built64 = build_minimal_acpi(&PlatformConfig::intel_tdx_q35(64));
        assert_eq!(built64.layout["DSDT"].length, 13_535);
        assert_eq!(built64.layout["APIC"].length, 734);
    }

    #[test]
    fn hpet_on_matches_16_vcpu_ground_truth() {
        let mut config = PlatformConfig::intel_tdx_q35(16);
        config.has_hpet = true;
        let built = build_minimal_acpi(&config);
        assert_eq!(built.layout["DSDT"].length, 9_547);
        assert_eq!(built.layout["FACP"].offset, 9_611);
        assert_eq!(built.layout["APIC"].offset, 9_855);
        assert_eq!(built.layout["HPET"].offset, 10_205);
        assert_eq!(built.layout["MCFG"].offset, 10_261);
        assert_eq!(built.layout["WAET"].offset, 10_321);
        assert_eq!(built.layout["RSDT"].offset, 10_361);
        assert_eq!(built.layout["RSDT"].length, 56);
    }

    #[test]
    fn pci_device_shifts_16_vcpu_ground_truth() {
        let mut config = PlatformConfig::intel_tdx_q35(16);
        config
            .pci_devices
            .push(crate::acpi::config::PciDeviceConfig {
                driver: "virtio-net-pci".to_string(),
                id: Some("net0dev".to_string()),
                bus: Some("pcie.0".to_string()),
                devfn: 0x08,
            });
        let built = build_minimal_acpi(&config);
        assert_eq!(built.layout["DSDT"].length, 9_422);
        assert_eq!(built.layout["FACP"].offset, 9_486);
        assert_eq!(built.layout["APIC"].offset, 9_730);
        assert_eq!(built.layout["MCFG"].offset, 10_080);
        assert_eq!(built.layout["WAET"].offset, 10_140);
        assert_eq!(built.layout["RSDT"].offset, 10_180);
    }

    #[test]
    fn one_vcpu_matches_ground_truth() {
        let built = build_minimal_acpi(&PlatformConfig::intel_tdx_q35(1));
        assert_eq!(built.layout["DSDT"].length, 8_117);
        assert_eq!(built.layout["FACP"].offset, 8_181);
        assert_eq!(built.layout["APIC"].offset, 8_425);
        assert_eq!(built.layout["MCFG"].offset, 8_655);
        assert_eq!(built.layout["WAET"].offset, 8_715);
        assert_eq!(built.layout["RSDT"].offset, 8_755);
    }

    #[test]
    fn two_vcpu_matches_ground_truth() {
        let built = build_minimal_acpi(&PlatformConfig::intel_tdx_q35(2));
        assert_eq!(built.layout["DSDT"].length, 8_200);
        assert_eq!(built.layout["FACP"].offset, 8_264);
        assert_eq!(built.layout["APIC"].offset, 8_508);
        assert_eq!(built.layout["MCFG"].offset, 8_746);
        assert_eq!(built.layout["WAET"].offset, 8_806);
        assert_eq!(built.layout["RSDT"].offset, 8_846);
    }
}
