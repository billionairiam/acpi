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

    let tree = AmlNode::Scope {
        name: "\\_SB".to_string(),
        children: vec![AmlNode::Device {
            name: "PCI0".to_string(),
            children: vec![
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
                    name: "_STA".to_string(),
                    value: AmlValue::Integer(0x0f),
                },
                AmlNode::Name {
                    name: "_CRS".to_string(),
                    value: AmlValue::Buffer(resource_template(config)),
                },
                AmlNode::Name {
                    name: "_PRT".to_string(),
                    value: AmlValue::Package(Vec::new()),
                },
                AmlNode::Method {
                    name: "_OSC".to_string(),
                    args: 4,
                    serialized: false,
                    body: vec![AmlNode::Return(AmlValue::Arg(3))],
                },
            ],
        }],
    };

    let mut body = Vec::new();
    tree.encode(&mut body);
    finalize_table(header, &body)
}

fn resource_template(config: &PlatformConfig) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x88);
    out.extend_from_slice(&0x000d_u16.to_le_bytes());
    out.push(0);
    out.extend_from_slice(&0x0000_u16.to_le_bytes());
    out.extend_from_slice(&config.pci_bus_end.to_le_bytes());
    out.extend_from_slice(&0x0000_u16.to_le_bytes());
    out.extend_from_slice(&u16::from(config.pci_bus_end).to_le_bytes());
    out.push(0x79);
    out.push(0x00);
    out
}
