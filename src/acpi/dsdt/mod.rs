use crate::acpi::Aml;
use crate::acpi::aml::{
    AmlString, Arg, BufferData, Device, EISAName, Else, Equal, If, Interrupt, Method, Name,
    Package, PackageBuilder, Path, ResourceTemplate, Return, Scope, Store, ZERO,
};
use crate::acpi::config::{PciDeviceConfig, PlatformConfig};
use crate::acpi::header::{AcpiHeader, finalize_table};

pub fn build_dsdt(config: &PlatformConfig) -> Vec<u8> {
    let header = AcpiHeader {
        signature: *b"DSDT",
        revision: 1,
        oem_id: config.oem_id,
        oem_table_id: config.oem_table_id,
        oem_revision: config.oem_revision,
        creator_id: config.creator_id,
        creator_revision: config.creator_revision,
    };

    let body = encode_dsdt_with_padding(config);
    finalize_table(header, &body)
}

fn encode_dsdt_with_padding(config: &PlatformConfig) -> Vec<u8> {
    let body = encode_dsdt_body(config, 0);

    let Some(target_total_len) = config.target_dsdt_len() else {
        return body;
    };
    let target_body_len = target_total_len.saturating_sub(36) as usize;
    if body.len() >= target_body_len {
        return body;
    }

    let mut low = 0usize;
    let mut high = target_body_len - body.len();
    let mut best = None;
    while low <= high {
        let mid = (low + high) / 2;
        let candidate = encode_dsdt_body(config, mid);
        match candidate.len().cmp(&target_body_len) {
            std::cmp::Ordering::Equal => return candidate,
            std::cmp::Ordering::Less => {
                best = Some(candidate);
                low = mid + 1;
            }
            std::cmp::Ordering::Greater => {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            }
        }
    }

    let mut candidate = best.unwrap_or_else(|| encode_dsdt_body(config, 0));
    while candidate.len() < target_body_len {
        let missing = target_body_len - candidate.len();
        candidate = encode_dsdt_body(config, high + missing);
        if candidate.len() > target_body_len {
            break;
        }
    }
    candidate
}

fn encode_dsdt_body(config: &PlatformConfig, pad_len: usize) -> Vec<u8> {
    let mut body = build_root_scope_bytes(config);
    body.extend_from_slice(&build_q35_irq_routing_bytes());
    body.extend_from_slice(&build_root_sleep_bytes());
    body.extend_from_slice(&build_gpe_scope_bytes());
    if pad_len > 0 {
        append_aml(
            &Name::new(Path::new("QPAD"), &BufferData::new(vec![0u8; pad_len])),
            &mut body,
        );
    }
    body
}

fn build_root_scope_bytes(config: &PlatformConfig) -> Vec<u8> {
    let mut sb_bytes = build_pci0_device_bytes(config);
    for cpu_bytes in build_cpu_device_bytes(config) {
        sb_bytes.extend_from_slice(&cpu_bytes);
    }
    if config.has_hpet {
        sb_bytes.extend_from_slice(&build_hpet_device_bytes());
    }
    Scope::raw(Path::new("\\_SB_"), sb_bytes)
}

fn build_pci0_device_bytes(config: &PlatformConfig) -> Vec<u8> {
    let mut children = Vec::new();
    append_aml(
        &Name::new(Path::new("_HID"), &EISAName::new("PNP0A08")),
        &mut children,
    );
    append_aml(
        &Name::new(Path::new("_CID"), &EISAName::new("PNP0A03")),
        &mut children,
    );
    append_aml(
        &Name::new(Path::new("_UID"), &u64::from(config.pci_root_uid)),
        &mut children,
    );

    let osc_arg = Arg(3);
    let osc_ret = Return::new(&osc_arg);
    let osc = Method::new(Path::new("_OSC"), 4, false, vec![&osc_ret]);
    append_aml(&osc, &mut children);

    for device in config
        .pci_devices
        .iter()
        .filter(|device| device.is_root_bus_device())
    {
        children.extend_from_slice(&build_pci_device_bytes(device));
    }

    Device::raw(Path::new("PCI0"), children)
}

fn build_pci_device_bytes(device: &PciDeviceConfig) -> Vec<u8> {
    let mut children = Vec::new();
    let slot = u64::from(device.devfn >> 3);

    append_aml(
        &Name::new(Path::new("_ADR"), &u64::from(pci_adr(device.devfn))),
        &mut children,
    );
    append_aml(&Name::new(Path::new("_STA"), &0x0fu8), &mut children);
    append_aml(&Name::new(Path::new("_SUN"), &slot), &mut children);
    append_aml(&Name::new(Path::new("_UID"), &slot), &mut children);

    if let Some(id) = &device.id {
        append_aml(
            &Name::new(Path::new("_DDN"), &AmlString::from(id.clone())),
            &mut children,
        );
    }
    if let Some(description) = pci_device_description(&device.driver) {
        append_aml(
            &Name::new(Path::new("_STR"), &AmlString::from(description.to_string())),
            &mut children,
        );
    }
    append_device_specific_objects(&mut children, device);

    Device::raw(Path::new(&pci_device_name(device.devfn)), children)
}

fn append_device_specific_objects(children: &mut Vec<u8>, device: &PciDeviceConfig) {
    let class = match device.driver.as_str() {
        "virtio-net-pci" => Some("net"),
        "virtio-blk-pci" => Some("block"),
        "virtio-scsi-pci" => Some("storage"),
        "virtio-balloon-pci" => Some("memory"),
        _ => None,
    };

    if let Some(class) = class {
        append_aml(
            &Name::new(Path::new("CLSS"), &AmlString::from(class.to_string())),
            children,
        );
    }
}

fn pci_device_description(driver: &str) -> Option<&'static str> {
    match driver {
        "virtio-net-pci" => Some("Virtio Network Device"),
        "virtio-blk-pci" => Some("Virtio Block Device"),
        "virtio-scsi-pci" => Some("Virtio SCSI Controller"),
        "virtio-balloon-pci" => Some("Virtio Balloon Device"),
        _ => None,
    }
}

fn pci_device_name(devfn: u8) -> String {
    format!("S{:02X}_", devfn)
}

fn pci_adr(devfn: u8) -> u32 {
    let slot = u32::from(devfn >> 3);
    let func = u32::from(devfn & 0x07);
    (slot << 16) | func
}

fn build_q35_irq_routing_bytes() -> Vec<u8> {
    let mut out = Vec::new();

    append_aml(&Name::new(Path::new("PICF"), &ZERO), &mut out);

    let pic_arg = Arg(0);
    let picf_path = Path::new("PICF");
    let pic_store = Store::new(&picf_path, &pic_arg);
    let pic_method = Method::new(Path::new("_PIC"), 1, false, vec![&pic_store]);
    append_aml(&pic_method, &mut out);

    let prtp = Name::new(Path::new("PRTP"), &build_q35_routing_table("LNK"));
    let prta = Name::new(Path::new("PRTA"), &build_q35_routing_table("GSI"));
    let picf_ref = Path::new("PICF");
    let prtp_ref = Path::new("PRTP");
    let prta_ref = Path::new("PRTA");
    let predicate = Equal::new(&picf_ref, &ZERO);
    let ret_prtp = Return::new(&prtp_ref);
    let ret_prta = Return::new(&prta_ref);
    let if_ctx = If::new(&predicate, vec![&ret_prtp]);
    let else_ctx = Else::new(vec![&ret_prta]);
    let prt_method = Method::new(Path::new("_PRT"), 0, false, vec![&if_ctx, &else_ctx]);
    let pci0_scope = Scope::new(Path::new("\\_SB_.PCI0"), vec![&prtp, &prta, &prt_method]);
    append_aml(&pci0_scope, &mut out);

    let mut sb_bytes = Vec::new();
    for (name, uid, irq_name) in [
        ("LNKA", 0u8, "PRQA"),
        ("LNKB", 1, "PRQB"),
        ("LNKC", 2, "PRQC"),
        ("LNKD", 3, "PRQD"),
        ("LNKE", 4, "PRQE"),
        ("LNKF", 5, "PRQF"),
        ("LNKG", 6, "PRQG"),
        ("LNKH", 7, "PRQH"),
    ] {
        append_aml(
            &Name::new(Path::new(irq_name), &(0x10u8 + uid)),
            &mut sb_bytes,
        );
        sb_bytes.extend_from_slice(&build_link_device_bytes(name, uid));
    }
    for (name, uid, irq) in [
        ("GSIA", 0x10u8, 0x10u32),
        ("GSIB", 0x11, 0x11),
        ("GSIC", 0x12, 0x12),
        ("GSID", 0x13, 0x13),
        ("GSIE", 0x14, 0x14),
        ("GSIF", 0x15, 0x15),
        ("GSIG", 0x16, 0x16),
        ("GSIH", 0x17, 0x17),
    ] {
        sb_bytes.extend_from_slice(&build_gsi_link_device_bytes(name, uid, irq));
    }
    out.extend_from_slice(&Scope::raw(Path::new("\\_SB_"), sb_bytes));

    out
}

fn build_root_sleep_bytes() -> Vec<u8> {
    let s5_pkg = Package::new(vec![&ZERO, &ZERO, &ZERO, &ZERO]);
    let s5 = Name::new(Path::new("_S5_"), &s5_pkg);
    let mut out = Vec::new();
    append_aml(&s5, &mut out);
    out
}

fn build_gpe_scope_bytes() -> Vec<u8> {
    let hid = Name::new(Path::new("_HID"), &AmlString::from("ACPI0006"));
    let gpe = Scope::new(Path::new("_GPE"), vec![&hid]);
    let mut out = Vec::new();
    append_aml(&gpe, &mut out);
    out
}

fn build_q35_routing_table(prefix: &str) -> PackageBuilder {
    let mut pkg = PackageBuilder::new();
    for slot in 0..0x18u32 {
        let mut name = format!("{prefix}E");
        append_q35_prt_entry(&mut pkg, slot, &mut name);
    }

    let mut name = format!("{prefix}E");
    append_q35_prt_entry(&mut pkg, 0x18, &mut name);

    for slot in 0x19..0x1eu32 {
        let mut name = format!("{prefix}A");
        append_q35_prt_entry(&mut pkg, slot, &mut name);
    }

    let mut name = format!("{prefix}E");
    append_q35_prt_entry(&mut pkg, 0x1e, &mut name);
    let mut name = format!("{prefix}A");
    append_q35_prt_entry(&mut pkg, 0x1f, &mut name);

    pkg
}

fn append_q35_prt_entry(pkg: &mut PackageBuilder, slot: u32, name: &mut String) {
    let base = if name.as_bytes()[3] < b'E' {
        b'A'
    } else {
        b'E'
    };
    let mut head = name.as_bytes()[3] - base;
    for pin in 0..4u8 {
        if head + pin > 3 {
            head = pin.wrapping_neg();
        }
        let suffix = (base + head + pin) as char;
        name.replace_range(3..4, &suffix.to_string());
        let adr = (slot << 16) | 0xffff;
        let pin_val = pin;
        let link = Path::new(name);
        let entry = Package::new(vec![&adr, &pin_val, &link, &ZERO]);
        pkg.add_element(&entry);
    }
}

fn build_link_device_bytes(name: &str, uid: u8) -> Vec<u8> {
    let irq5 = Interrupt::new(true, false, false, true, 5);
    let irq10 = Interrupt::new(true, false, false, true, 10);
    let irq11 = Interrupt::new(true, false, false, true, 11);
    let prs = ResourceTemplate::new(vec![&irq5, &irq10, &irq11]);
    let hid = Name::new(Path::new("_HID"), &EISAName::new("PNP0C0F"));
    let uid_name = Name::new(Path::new("_UID"), &uid);
    let prs_name = Name::new(Path::new("_PRS"), &prs);
    let sta_ret = Return::new(&0x0bu8);
    let sta = Method::new(Path::new("_STA"), 0, false, vec![&sta_ret]);
    let crs_ref = Path::new("_PRS");
    let crs_ret = Return::new(&crs_ref);
    let crs = Method::new(Path::new("_CRS"), 0, false, vec![&crs_ret]);
    let dis = Method::new(Path::new("_DIS"), 0, false, vec![]);
    let srs = Method::new(Path::new("_SRS"), 1, false, vec![]);
    let dev = Device::new(
        Path::new(name),
        vec![&hid, &uid_name, &prs_name, &sta, &dis, &crs, &srs],
    );

    let mut out = Vec::new();
    append_aml(&dev, &mut out);
    out
}

fn build_gsi_link_device_bytes(name: &str, uid: u8, irq: u32) -> Vec<u8> {
    let irq_res = Interrupt::new(true, false, false, true, irq);
    let res = ResourceTemplate::new(vec![&irq_res]);
    let hid = Name::new(Path::new("_HID"), &EISAName::new("PNP0C0F"));
    let uid_name = Name::new(Path::new("_UID"), &uid);
    let prs_name = Name::new(Path::new("_PRS"), &res);
    let crs_name = Name::new(Path::new("_CRS"), &res);
    let dis = Method::new(Path::new("_DIS"), 0, false, vec![]);
    let srs = Method::new(Path::new("_SRS"), 1, false, vec![]);
    let dev = Device::new(
        Path::new(name),
        vec![&hid, &uid_name, &prs_name, &crs_name, &dis, &srs],
    );

    let mut out = Vec::new();
    append_aml(&dev, &mut out);
    out
}

fn build_cpu_device_bytes(config: &PlatformConfig) -> Vec<Vec<u8>> {
    config
        .cpu_apic_ids
        .iter()
        .enumerate()
        .map(|(uid, apic_id)| {
            let mut children = Vec::new();
            append_aml(
                &Name::new(Path::new("_HID"), &AmlString::from("ACPI0007")),
                &mut children,
            );
            append_aml(&Name::new(Path::new("_UID"), &(uid as u64)), &mut children);
            append_aml(&Name::new(Path::new("_STA"), &0x0fu8), &mut children);
            append_aml(
                &Name::new(
                    Path::new("_MAT"),
                    &BufferData::new(cpu_mat(*apic_id, uid as u32)),
                ),
                &mut children,
            );
            Device::raw(Path::new(&cpu_device_name(uid)), children)
        })
        .collect()
}

fn cpu_device_name(uid: usize) -> String {
    format!("C{uid:03}")
}

fn cpu_mat(apic_id: u32, uid: u32) -> Vec<u8> {
    let mut mat = Vec::with_capacity(16);
    mat.push(9);
    mat.push(16);
    mat.extend_from_slice(&[0u8; 2]);
    mat.extend_from_slice(&uid.to_le_bytes());
    mat.extend_from_slice(&apic_id.to_le_bytes());
    mat.extend_from_slice(&1u32.to_le_bytes());
    mat
}

fn build_hpet_device_bytes() -> Vec<u8> {
    let mut children = Vec::new();
    append_aml(
        &Name::new(Path::new("_HID"), &EISAName::new("PNP0103")),
        &mut children,
    );
    append_aml(&Name::new(Path::new("_UID"), &ZERO), &mut children);
    append_aml(&Name::new(Path::new("_STA"), &0x0fu8), &mut children);
    append_aml(
        &Name::new(
            Path::new("_CRS"),
            &BufferData::new(hpet_resource_template()),
        ),
        &mut children,
    );
    Device::raw(Path::new("HPET"), children)
}

fn hpet_resource_template() -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x86);
    out.extend_from_slice(&0x0009_u16.to_le_bytes());
    out.push(0);
    out.extend_from_slice(&0xfed0_0000_u32.to_le_bytes());
    out.extend_from_slice(&0x0000_0400_u32.to_le_bytes());
    out.push(0x79);
    out.push(0x00);
    out
}

fn append_aml(value: &dyn Aml, out: &mut Vec<u8>) {
    value.to_aml_bytes(out);
}

#[cfg(test)]
mod tests {
    use super::build_dsdt;
    use crate::acpi::config::{PciDeviceConfig, PlatformConfig};

    #[test]
    fn dsdt_contains_named_pci_device_node() {
        let mut config = PlatformConfig::intel_tdx_q35(16);
        config.pci_devices.push(PciDeviceConfig {
            driver: "virtio-net-pci".to_string(),
            id: Some("net0dev".to_string()),
            bus: Some("pcie.0".to_string()),
            devfn: 0x10,
        });
        let dsdt = build_dsdt(&config);
        assert!(dsdt.windows(4).any(|window| window == b"S10_"));
        assert!(
            dsdt.windows("net0dev".len())
                .any(|window| window == b"net0dev")
        );
        assert!(
            dsdt.windows("Virtio Network Device".len())
                .any(|window| window == b"Virtio Network Device")
        );
        assert!(dsdt.windows(4).any(|window| window == b"_UID"));
    }

    #[test]
    fn dsdt_contains_multiple_virtio_descriptions() {
        let mut config = PlatformConfig::intel_tdx_q35(16);
        config.pci_devices.push(PciDeviceConfig {
            driver: "virtio-blk-pci".to_string(),
            id: Some("disk0dev".to_string()),
            bus: Some("pcie.0".to_string()),
            devfn: 0x18,
        });
        config.pci_devices.push(PciDeviceConfig {
            driver: "virtio-scsi-pci".to_string(),
            id: Some("scsi0dev".to_string()),
            bus: Some("pcie.0".to_string()),
            devfn: 0x20,
        });

        let dsdt = build_dsdt(&config);
        assert!(dsdt.windows(4).any(|window| window == b"S18_"));
        assert!(dsdt.windows(4).any(|window| window == b"S20_"));
        assert!(
            dsdt.windows("Virtio Block Device".len())
                .any(|window| window == b"Virtio Block Device")
        );
        assert!(
            dsdt.windows("Virtio SCSI Controller".len())
                .any(|window| window == b"Virtio SCSI Controller")
        );
        assert!(dsdt.windows(5).any(|window| window == b"block"));
        assert!(dsdt.windows(7).any(|window| window == b"storage"));
    }

    #[test]
    fn dsdt_ignores_non_root_bus_pci_devices() {
        let mut config = PlatformConfig::intel_tdx_q35(16);
        config.pci_devices.push(PciDeviceConfig {
            driver: "virtio-net-pci".to_string(),
            id: Some("net1dev".to_string()),
            bus: Some("rp0".to_string()),
            devfn: 0x18,
        });

        let dsdt = build_dsdt(&config);
        assert!(
            !dsdt
                .windows("net1dev".len())
                .any(|window| window == b"net1dev")
        );
    }
}
