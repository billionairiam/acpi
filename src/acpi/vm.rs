use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::acpi::config::{IommuKind, PciDeviceConfig, PlatformConfig, TpmKind};

#[derive(Debug, Deserialize)]
pub struct VmConfig {
    pub qemu: QemuConfig,
    pub platform: PlatformSection,
    pub cpu: CpuSection,
    pub memory: MemorySection,
    #[serde(default)]
    pub machine: MachineSection,
    #[serde(default)]
    pub defaults: DefaultDevices,
    #[serde(default)]
    pub netdevs: Vec<Netdev>,
    #[serde(default)]
    pub devices: Vec<Device>,
}

impl VmConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        toml::from_str(&text)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))
    }

    pub fn platform_config(&self) -> PlatformConfig {
        let mut config = PlatformConfig::intel_tdx_q35(self.cpu.topology.cpus);
        config.has_hpet = self.machine.properties.bool("hpet").unwrap_or(false);
        config.has_vmgenid = self.devices.iter().any(|device| device.is_vmgenid());
        config.has_mcfg = self.machine.properties.bool("mcfg").unwrap_or(true);
        config.has_numa = self.machine.properties.bool("numa").unwrap_or(false);
        config.has_slit = self
            .machine
            .properties
            .bool("slit")
            .unwrap_or(config.has_numa);
        config.has_hmat = self.machine.properties.bool("hmat").unwrap_or(false);
        config.tpm_kind = self
            .machine
            .properties
            .tpm_kind()
            .or_else(|| self.devices.iter().find_map(|device| device.tpm_kind()));
        config.iommu_kind = self
            .machine
            .properties
            .iommu_kind()
            .or_else(|| self.devices.iter().find_map(|device| device.iommu_kind()));
        config.nvdimm_enabled = self
            .machine
            .properties
            .bool("nvdimm")
            .unwrap_or_else(|| self.devices.iter().any(|device| device.is_nvdimm()));
        config.cxl_enabled = self
            .machine
            .properties
            .bool("cxl")
            .unwrap_or_else(|| self.devices.iter().any(|device| device.is_cxl()));
        config.pci_devices = self
            .devices
            .iter()
            .filter(|device| device.driver.ends_with("-pci"))
            .enumerate()
            .map(|(index, device)| PciDeviceConfig {
                driver: device.driver.clone(),
                id: device.string_prop("id"),
                bus: device.string_prop("bus"),
                devfn: device.devfn().unwrap_or(default_devfn(index)),
            })
            .collect();
        config.pci_devices.sort_by_key(|device| device.devfn);
        config
    }

    pub fn qemu_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        args.push("-accel".to_string());
        args.push(self.platform.accel.clone());

        args.push("-m".to_string());
        args.push(self.memory.size.clone());

        args.push("-smp".to_string());
        args.push(self.cpu.topology.cpus.to_string());

        args.push("-cpu".to_string());
        args.push(self.cpu.model.clone());

        args.push("-machine".to_string());
        args.push(self.machine_arg());

        args.push("-bios".to_string());
        args.push(self.platform.bios.clone());

        if self.headless() {
            args.push("-nographic".to_string());
        }
        if self.platform.nodefaults {
            args.push("-nodefaults".to_string());
        }
        if self.defaults.serial_backend() {
            args.push("-serial".to_string());
            args.push("stdio".to_string());
        }

        for netdev in &self.netdevs {
            args.push("-netdev".to_string());
            args.push(netdev.to_qemu_value());
        }
        for device in &self.devices {
            args.push("-device".to_string());
            args.push(device.to_qemu_value());
        }

        args
    }

    fn machine_arg(&self) -> String {
        let mut parts = vec![self.platform.machine.clone()];
        parts.extend(self.machine.properties.to_qemu_pairs());
        parts.join(",")
    }

    fn headless(&self) -> bool {
        !self.defaults.vga
            && !self.defaults.monitor
            && !self.defaults.parallel
            && !self.defaults.floppy
            && !self.defaults.cdrom
    }
}

#[derive(Debug, Deserialize)]
pub struct QemuConfig {
    pub binary: String,
}

#[derive(Debug, Deserialize)]
pub struct PlatformSection {
    pub machine: String,
    pub bios: String,
    pub accel: String,
    #[serde(default)]
    pub nodefaults: bool,
}

#[derive(Debug, Deserialize)]
pub struct CpuSection {
    pub model: String,
    pub topology: CpuTopology,
}

#[derive(Debug, Deserialize)]
pub struct CpuTopology {
    pub cpus: u32,
}

#[derive(Debug, Deserialize)]
pub struct MemorySection {
    pub size: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct MachineSection {
    #[serde(default)]
    pub properties: MachineProperties,
}

#[derive(Debug, Deserialize, Default)]
pub struct MachineProperties {
    #[serde(flatten)]
    pub values: BTreeMap<String, toml::Value>,
}

impl MachineProperties {
    fn bool(&self, key: &str) -> Option<bool> {
        match self.values.get(key) {
            Some(toml::Value::Boolean(value)) => Some(*value),
            _ => None,
        }
    }

    fn string(&self, key: &str) -> Option<&str> {
        match self.values.get(key) {
            Some(toml::Value::String(value)) => Some(value.as_str()),
            _ => None,
        }
    }

    fn tpm_kind(&self) -> Option<TpmKind> {
        match self.string("tpm")? {
            "1.2" | "tcpa" | "tpm12" => Some(TpmKind::Tcpa),
            "2.0" | "tpm2" => Some(TpmKind::Tpm2),
            _ => None,
        }
    }

    fn iommu_kind(&self) -> Option<IommuKind> {
        match self.string("iommu")? {
            "intel" | "dmar" => Some(IommuKind::Dmar),
            "amd" | "ivrs" => Some(IommuKind::Ivrs),
            "virtio" | "viot" => Some(IommuKind::Viot),
            _ => None,
        }
    }

    fn to_qemu_pairs(&self) -> Vec<String> {
        let mut pairs = Vec::new();
        for (key, value) in &self.values {
            let rendered = match value {
                toml::Value::Boolean(value) => {
                    if *value {
                        "on".to_string()
                    } else {
                        "off".to_string()
                    }
                }
                toml::Value::String(value) => value.clone(),
                toml::Value::Integer(value) => value.to_string(),
                toml::Value::Float(value) => value.to_string(),
                _ => continue,
            };
            pairs.push(format!("{key}={rendered}"));
        }
        pairs
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct DefaultDevices {
    #[serde(default)]
    pub serial: bool,
    #[serde(default)]
    pub parallel: bool,
    #[serde(default)]
    pub monitor: bool,
    #[serde(default)]
    pub vga: bool,
    #[serde(default)]
    pub floppy: bool,
    #[serde(default)]
    pub cdrom: bool,
}

impl DefaultDevices {
    fn serial_backend(&self) -> bool {
        !self.serial
    }
}

#[derive(Debug, Deserialize)]
pub struct Netdev {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
}

impl Netdev {
    fn to_qemu_value(&self) -> String {
        format!("{},id={}", self.kind, self.id)
    }
}

#[derive(Debug, Deserialize)]
pub struct Device {
    pub driver: String,
    #[serde(flatten)]
    pub props: BTreeMap<String, toml::Value>,
}

impl Device {
    fn string_prop(&self, key: &str) -> Option<String> {
        match self.props.get(key) {
            Some(toml::Value::String(value)) => Some(value.clone()),
            _ => None,
        }
    }

    fn devfn(&self) -> Option<u8> {
        let addr = self.props.get("addr")?;
        let toml::Value::String(addr) = addr else {
            return None;
        };
        parse_qemu_addr(addr)
    }

    fn tpm_kind(&self) -> Option<TpmKind> {
        let driver = self.driver.as_str();
        if driver.contains("tpm-tis") || driver.contains("tpm-spapr") {
            Some(TpmKind::Tcpa)
        } else if driver.contains("tpm-crb") {
            Some(TpmKind::Tpm2)
        } else {
            None
        }
    }

    fn iommu_kind(&self) -> Option<IommuKind> {
        match self.driver.as_str() {
            "intel-iommu" => Some(IommuKind::Dmar),
            "amd-iommu" => Some(IommuKind::Ivrs),
            "virtio-iommu-pci" => Some(IommuKind::Viot),
            _ => None,
        }
    }

    fn is_nvdimm(&self) -> bool {
        self.driver.contains("nvdimm")
    }

    fn is_cxl(&self) -> bool {
        self.driver.contains("cxl")
    }

    fn is_vmgenid(&self) -> bool {
        self.driver.contains("vmgenid")
    }

    fn to_qemu_value(&self) -> String {
        let mut parts = vec![self.driver.clone()];
        for (key, value) in &self.props {
            match value {
                toml::Value::String(value) => {
                    parts.push(format!("{key}={value}"));
                }
                toml::Value::Boolean(value) => {
                    parts.push(format!("{key}={}", if *value { "on" } else { "off" }));
                }
                toml::Value::Integer(value) => parts.push(format!("{key}={value}")),
                toml::Value::Float(value) => parts.push(format!("{key}={value}")),
                _ => {}
            }
        }
        parts.join(",")
    }
}

fn default_devfn(index: usize) -> u8 {
    ((index as u8) + 1) << 3
}

fn parse_qemu_addr(addr: &str) -> Option<u8> {
    let (slot_text, func_text) = match addr.split_once('.') {
        Some((slot, func)) => (slot, func),
        None => (addr, "0"),
    };
    let slot = parse_u8_any_radix(slot_text)?;
    let func = parse_u8_any_radix(func_text)?;
    Some((slot << 3) | (func & 0x07))
}

fn parse_u8_any_radix(text: &str) -> Option<u8> {
    if let Some(hex) = text.strip_prefix("0x") {
        u8::from_str_radix(hex, 16).ok()
    } else {
        text.parse::<u8>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::VmConfig;
    use crate::acpi::config::{IommuKind, TpmKind};

    #[test]
    fn renders_machine_properties_and_devices() {
        let config: VmConfig = toml::from_str(
            r#"
            [qemu]
            binary = "/usr/bin/qemu-system-x86_64"

            [platform]
            machine = "q35"
            bios = "/usr/share/ovmf/OVMF.fd"
            accel = "kvm"
            nodefaults = true

            [cpu]
            model = "host"

            [cpu.topology]
            cpus = 4

            [memory]
            size = "2G"

            [machine.properties]
            kernel_irqchip = "split"
            hpet = false
            smm = false

            [defaults]
            serial = false
            parallel = false
            monitor = false
            vga = false
            floppy = false
            cdrom = false

            [[netdevs]]
            id = "net0"
            type = "user"

            [[devices]]
            driver = "virtio-net-pci"
            id = "net0dev"
            netdev = "net0"
            addr = "0x02"
            multifunction = false
            "#,
        )
        .unwrap();

        let args = config.qemu_args().join(" ");
        assert!(args.contains("-machine q35,hpet=off,kernel_irqchip=split,smm=off"));
        assert!(args.contains("-serial stdio"));
        assert!(args.contains("-netdev user,id=net0"));
        assert!(
            args.contains(
                "-device virtio-net-pci,addr=0x02,id=net0dev,multifunction=off,netdev=net0"
            )
        );
    }

    #[test]
    fn keeps_empty_device_properties() {
        let config: VmConfig = toml::from_str(
            r#"
            [qemu]
            binary = "/usr/bin/qemu-system-x86_64"

            [platform]
            machine = "q35"
            bios = "/usr/share/ovmf/OVMF.fd"
            accel = "kvm"

            [cpu]
            model = "host"

            [cpu.topology]
            cpus = 16

            [memory]
            size = "2G"

            [[devices]]
            driver = "virtio-net-pci"
            netdev = "net0"
            romfile = ""
            "#,
        )
        .unwrap();

        let args = config.qemu_args().join(" ");
        assert!(args.contains("-device virtio-net-pci,netdev=net0,romfile="));
        assert_eq!(config.platform_config().pci_devices.len(), 1);
        let platform = config.platform_config();
        assert_eq!(platform.pci_devices[0].devfn, 0x08);
        assert_eq!(platform.pci_devices[0].driver, "virtio-net-pci");
    }

    #[test]
    fn sorts_pci_devices_by_devfn() {
        let config: VmConfig = toml::from_str(
            r#"
            [qemu]
            binary = "/usr/bin/qemu-system-x86_64"

            [platform]
            machine = "q35"
            bios = "/usr/share/ovmf/OVMF.fd"
            accel = "kvm"

            [cpu]
            model = "host"

            [cpu.topology]
            cpus = 16

            [memory]
            size = "2G"

            [[devices]]
            driver = "virtio-net-pci"
            addr = "0x04"

            [[devices]]
            driver = "virtio-blk-pci"
            addr = "0x02"
            "#,
        )
        .unwrap();

        let platform = config.platform_config();
        assert_eq!(platform.pci_devices[0].devfn, 0x10);
        assert_eq!(platform.pci_devices[1].devfn, 0x20);
    }

    #[test]
    fn preserves_bus_name_in_pci_device_config() {
        let config: VmConfig = toml::from_str(
            r#"
            [qemu]
            binary = "/usr/bin/qemu-system-x86_64"

            [platform]
            machine = "q35"
            bios = "/usr/share/ovmf/OVMF.fd"
            accel = "kvm"

            [cpu]
            model = "host"

            [cpu.topology]
            cpus = 16

            [memory]
            size = "2G"

            [[devices]]
            driver = "virtio-net-pci"
            bus = "rp0"
            addr = "0x03"
            "#,
        )
        .unwrap();

        let platform = config.platform_config();
        assert_eq!(platform.pci_devices.len(), 1);
        assert_eq!(platform.pci_devices[0].bus.as_deref(), Some("rp0"));
        assert!(!platform.pci_devices[0].is_root_bus_device());
    }

    #[test]
    fn detects_optional_acpi_features() {
        let config: VmConfig = toml::from_str(
            r#"
            [qemu]
            binary = "/usr/bin/qemu-system-x86_64"

            [platform]
            machine = "q35"
            bios = "/usr/share/ovmf/OVMF.fd"
            accel = "kvm"

            [cpu]
            model = "host"

            [cpu.topology]
            cpus = 16

            [memory]
            size = "2G"

            [machine.properties]
            hpet = true
            mcfg = false
            numa = true
            slit = true
            hmat = true

            [[devices]]
            driver = "tpm-crb"

            [[devices]]
            driver = "virtio-iommu-pci"

            [[devices]]
            driver = "nvdimm"

            [[devices]]
            driver = "cxl-type3"
            "#,
        )
        .unwrap();

        let platform = config.platform_config();
        assert!(platform.has_hpet);
        assert!(!platform.has_mcfg);
        assert!(platform.has_numa);
        assert!(platform.has_slit);
        assert!(platform.has_hmat);
        assert_eq!(platform.tpm_kind, Some(TpmKind::Tpm2));
        assert_eq!(platform.iommu_kind, Some(IommuKind::Viot));
        assert!(platform.nvdimm_enabled);
        assert!(platform.cxl_enabled);
    }
}
