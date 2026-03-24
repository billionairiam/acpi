#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRef {
    pub signature: [u8; 4],
    pub offset: u32,
    pub length: u32,
}

#[derive(Debug, Clone)]
pub struct AcpiBlob {
    pub data: Vec<u8>,
    pub tables: Vec<TableRef>,
}

#[derive(Debug, Clone)]
pub struct AcpiBlobBuilder {
    data: Vec<u8>,
    tables: Vec<TableRef>,
    base_address: u64,
}

impl AcpiBlobBuilder {
    pub fn new(base_address: u64) -> Self {
        Self {
            data: Vec::new(),
            tables: Vec::new(),
            base_address,
        }
    }

    pub fn reserve_front(&mut self, len: usize) {
        if self.data.len() < len {
            self.data.resize(len, 0);
        }
    }

    #[allow(dead_code)]
    pub fn align(&mut self, alignment: usize) {
        let remainder = self.data.len() % alignment;
        if remainder != 0 {
            self.data
                .resize(self.data.len() + (alignment - remainder), 0);
        }
    }

    pub fn append_table(&mut self, signature: [u8; 4], bytes: Vec<u8>) -> u32 {
        let offset = self.data.len() as u32;
        let length = bytes.len() as u32;
        self.data.extend_from_slice(&bytes);
        self.tables.push(TableRef {
            signature,
            offset,
            length,
        });
        offset
    }

    pub fn append_placeholder_table(&mut self, signature: [u8; 4], length: u32) -> u32 {
        let offset = self.data.len() as u32;
        self.data.resize(self.data.len() + length as usize, 0);
        self.tables.push(TableRef {
            signature,
            offset,
            length,
        });
        offset
    }

    pub fn address_of(&self, offset: u32) -> u64 {
        self.base_address + u64::from(offset)
    }

    pub fn finish(self) -> AcpiBlob {
        AcpiBlob {
            data: self.data,
            tables: self.tables,
        }
    }
}
