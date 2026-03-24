pub mod aml;

use crate::acpi::config::PlatformConfig;
use crate::acpi::dsdt::aml::{AmlNode, AmlValue};
use crate::acpi::header::{AcpiHeader, finalize_table};

pub fn build_dsdt(config: &PlatformConfig) -> Vec<u8> {
    let header = AcpiHeader {
        signature: *b"DSDT",
        revision: 2,
        oem_id: config.oem_id,
        oem_table_id: config.oem_table_id,
        oem_revision: config.oem_revision,
        creator_id: config.creator_id,
        creator_revision: config.creator_revision,
    };

    let mut sb_children = vec![AmlNode::Device {
        name: "PCI0".to_string(),
        children: pci0_children(config),
    }];
    sb_children.extend(build_cpu_devices(config));
    if config.has_hpet {
        sb_children.push(build_hpet_device());
    }

    let tree = AmlNode::Scope {
        name: "\\_SB".to_string(),
        children: sb_children,
    };

    let body = encode_dsdt_with_padding(config, &tree);
    finalize_table(header, &body)
}

fn pci0_children(config: &PlatformConfig) -> Vec<AmlNode> {
    let mut children = vec![
            AmlNode::Name {
                name: "_HID".to_string(),
                value: AmlValue::EisaId("PNP0A08"),
            },
            AmlNode::Name {
                name: "_CID".to_string(),
                value: AmlValue::EisaId("PNP0A03"),
            },
            AmlNode::Name {
                name: "_UID".to_string(),
                value: AmlValue::Integer(config.pci_root_uid.into()),
            },
            AmlNode::Name {
                name: "_ADR".to_string(),
                value: AmlValue::Integer(0),
            },
            AmlNode::Name {
                name: "_SEG".to_string(),
                value: AmlValue::Integer(config.pci_segment.into()),
            },
            AmlNode::Name {
                name: "_BBN".to_string(),
                value: AmlValue::Integer(config.pci_bus_start.into()),
            },
            AmlNode::Name {
                name: "_STA".to_string(),
                value: AmlValue::Integer(0x0f),
            },
            AmlNode::Name {
                name: "_CRS".to_string(),
                value: AmlValue::Buffer(resource_template(config)),
            },
            AmlNode::Name {
                name: "_PRT".to_string(),
                value: AmlValue::Package(build_pci_routing(config)),
            },
            AmlNode::Method {
                name: "_OSC".to_string(),
                args: 4,
                serialized: false,
                body: vec![AmlNode::Return(AmlValue::Arg(3))],
            },
        ];
    children.extend(build_pci_device_nodes(config));
    children
}

fn build_pci_device_nodes(config: &PlatformConfig) -> Vec<AmlNode> {
    config
        .pci_devices
        .iter()
        .filter(|device| device.is_root_bus_device())
        .map(|device| AmlNode::Device {
            name: pci_device_name(device.devfn),
            children: pci_device_children(device),
        })
        .collect()
}

fn pci_device_name(devfn: u8) -> String {
    format!("S{:02X}", devfn)
}

fn pci_device_children(device: &crate::acpi::config::PciDeviceConfig) -> Vec<AmlNode> {
    let slot = u64::from(device.devfn >> 3);
    let mut children = vec![AmlNode::Name {
        name: "_ADR".to_string(),
        value: AmlValue::Integer(u64::from(pci_adr(device.devfn))),
    }];
    children.push(AmlNode::Name {
        name: "_STA".to_string(),
        value: AmlValue::Integer(0x0f),
    });
    children.push(AmlNode::Name {
        name: "_SUN".to_string(),
        value: AmlValue::Integer(slot),
    });
    children.push(AmlNode::Name {
        name: "_UID".to_string(),
        value: AmlValue::Integer(slot),
    });
    if let Some(id) = &device.id {
        children.push(AmlNode::Name {
            name: "_DDN".to_string(),
            value: AmlValue::String(id.clone()),
        });
    }
    if let Some(description) = pci_device_description(&device.driver) {
        children.push(AmlNode::Name {
            name: "_STR".to_string(),
            value: AmlValue::String(description.to_string()),
        });
    }
    append_device_specific_objects(&mut children, device);
    children
}

fn append_device_specific_objects(
    children: &mut Vec<AmlNode>,
    device: &crate::acpi::config::PciDeviceConfig,
) {
    match device.driver.as_str() {
        "virtio-net-pci" => {
            children.push(AmlNode::Name {
                name: "CLSS".to_string(),
                value: AmlValue::String("net".to_string()),
            });
        }
        "virtio-blk-pci" => {
            children.push(AmlNode::Name {
                name: "CLSS".to_string(),
                value: AmlValue::String("block".to_string()),
            });
        }
        "virtio-scsi-pci" => {
            children.push(AmlNode::Name {
                name: "CLSS".to_string(),
                value: AmlValue::String("storage".to_string()),
            });
        }
        "virtio-balloon-pci" => {
            children.push(AmlNode::Name {
                name: "CLSS".to_string(),
                value: AmlValue::String("memory".to_string()),
            });
        }
        _ => {}
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

fn pci_adr(devfn: u8) -> u32 {
    let slot = u32::from(devfn >> 3);
    let func = u32::from(devfn & 0x07);
    (slot << 16) | func
}

fn encode_dsdt_with_padding(config: &PlatformConfig, tree: &AmlNode) -> Vec<u8> {
    let mut body = Vec::new();
    tree.encode(&mut body);

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
        let candidate = encode_dsdt_body(tree, mid);
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

    let mut candidate = best.unwrap_or_else(|| encode_dsdt_body(tree, 0));
    while candidate.len() < target_body_len {
        let missing = target_body_len - candidate.len();
        candidate = encode_dsdt_body(tree, high + missing);
        if candidate.len() > target_body_len {
            break;
        }
    }
    candidate
}

fn encode_dsdt_body(tree: &AmlNode, pad_len: usize) -> Vec<u8> {
    let mut body = Vec::new();
    tree.encode(&mut body);
    if pad_len > 0 {
        AmlNode::Name {
            name: "QPAD".to_string(),
            value: AmlValue::Buffer(vec![0u8; pad_len]),
        }
        .encode(&mut body);
    }
    body
}

fn build_cpu_devices(config: &PlatformConfig) -> Vec<AmlNode> {
    config
        .cpu_apic_ids
        .iter()
        .enumerate()
        .map(|(uid, apic_id)| AmlNode::Device {
            name: cpu_device_name(uid),
            children: vec![
                AmlNode::Name {
                    name: "_HID".to_string(),
                    value: AmlValue::String("ACPI0007".to_string()),
                },
                AmlNode::Name {
                    name: "_UID".to_string(),
                    value: AmlValue::Integer(uid as u64),
                },
                AmlNode::Name {
                    name: "_STA".to_string(),
                    value: AmlValue::Integer(0x0f),
                },
                AmlNode::Name {
                    name: "_MAT".to_string(),
                    value: AmlValue::Buffer(cpu_mat(*apic_id, uid as u32)),
                },
            ],
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

fn resource_template(config: &PlatformConfig) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x88);
    out.extend_from_slice(&0x000d_u16.to_le_bytes());
    out.push(0x02);
    out.push(0x00);
    out.push(0x00);
    out.extend_from_slice(&0x0000_u16.to_le_bytes());
    out.extend_from_slice(&u16::from(config.pci_bus_start).to_le_bytes());
    out.extend_from_slice(&u16::from(config.pci_bus_end).to_le_bytes());
    out.extend_from_slice(&0x0000_u16.to_le_bytes());
    out.extend_from_slice(&(u16::from(config.pci_bus_end) + 1).to_le_bytes());
    out.push(0x79);
    out.push(0x00);
    out
}

fn build_hpet_device() -> AmlNode {
    AmlNode::Device {
        name: "HPET".to_string(),
        children: vec![
            AmlNode::Name {
                name: "_HID".to_string(),
                value: AmlValue::EisaId("PNP0103"),
            },
            AmlNode::Name {
                name: "_UID".to_string(),
                value: AmlValue::Integer(0),
            },
            AmlNode::Name {
                name: "_STA".to_string(),
                value: AmlValue::Integer(0x0f),
            },
            AmlNode::Name {
                name: "_CRS".to_string(),
                value: AmlValue::Buffer(hpet_resource_template()),
            },
        ],
    }
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

fn build_pci_routing(config: &PlatformConfig) -> Vec<AmlValue> {
    let mut routes = Vec::new();
    for slot in 0..32u32 {
        for pin in 0..4u32 {
            routes.push(AmlValue::Package(vec![
                AmlValue::Integer(u64::from((slot << 16) | 0xffff)),
                AmlValue::Integer(u64::from(pin)),
                AmlValue::Integer(0),
                AmlValue::Integer(u64::from(config.pci_irq_base + ((slot + pin) % 4))),
            ]));
        }
    }
    routes
}

#[cfg(test)]
mod tests {
    use super::build_dsdt;
    use crate::acpi::config::{PciDeviceConfig, PlatformConfig};

    #[test]
    fn dsdt_contains_named_pci_device_node() {
        let mut config = PlatformConfig::intel_tdx_q35(16);
        config
            .pci_devices
            .push(PciDeviceConfig {
                driver: "virtio-net-pci".to_string(),
                id: Some("net0dev".to_string()),
                bus: Some("pcie.0".to_string()),
                devfn: 0x10,
            });
        let dsdt = build_dsdt(&config);
        assert!(dsdt.windows(4).any(|window| window == b"S10_"));
        assert!(dsdt.windows("net0dev".len()).any(|window| window == b"net0dev"));
        assert!(dsdt
            .windows("Virtio Network Device".len())
            .any(|window| window == b"Virtio Network Device"));
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
        assert!(dsdt
            .windows("Virtio Block Device".len())
            .any(|window| window == b"Virtio Block Device"));
        assert!(dsdt
            .windows("Virtio SCSI Controller".len())
            .any(|window| window == b"Virtio SCSI Controller"));
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
        assert!(!dsdt.windows("net1dev".len()).any(|window| window == b"net1dev"));
    }
}
