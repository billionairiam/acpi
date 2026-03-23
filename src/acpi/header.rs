use crate::acpi::checksum::acpi_checksum;

pub const ACPI_HEADER_LEN: usize = 36;

#[derive(Debug, Clone, Copy)]
pub struct AcpiHeader {
    pub signature: [u8; 4],
    pub revision: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: [u8; 4],
    pub creator_revision: u32,
}

impl AcpiHeader {
    pub fn encode(&self, total_len: u32) -> [u8; ACPI_HEADER_LEN] {
        let mut out = [0u8; ACPI_HEADER_LEN];
        out[0..4].copy_from_slice(&self.signature);
        out[4..8].copy_from_slice(&total_len.to_le_bytes());
        out[8] = self.revision;
        out[9] = 0;
        out[10..16].copy_from_slice(&self.oem_id);
        out[16..24].copy_from_slice(&self.oem_table_id);
        out[24..28].copy_from_slice(&self.oem_revision.to_le_bytes());
        out[28..32].copy_from_slice(&self.creator_id);
        out[32..36].copy_from_slice(&self.creator_revision.to_le_bytes());
        out
    }
}

pub fn finalize_table(header: AcpiHeader, body: &[u8]) -> Vec<u8> {
    let total_len = (ACPI_HEADER_LEN + body.len()) as u32;
    let mut out = Vec::with_capacity(total_len as usize);
    out.extend_from_slice(&header.encode(total_len));
    out.extend_from_slice(body);
    out[9] = acpi_checksum(&out);
    out
}
