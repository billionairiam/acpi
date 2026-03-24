#[derive(Debug, Clone)]
pub enum AmlNode {
    Scope {
        name: String,
        children: Vec<AmlNode>,
    },
    Device {
        name: String,
        children: Vec<AmlNode>,
    },
    Name {
        name: String,
        value: AmlValue,
    },
    Method {
        name: String,
        args: u8,
        serialized: bool,
        body: Vec<AmlNode>,
    },
    Return(AmlValue),
}

#[derive(Debug, Clone)]
pub enum AmlValue {
    Integer(u64),
    String(String),
    EisaId(&'static str),
    Buffer(Vec<u8>),
    Package(Vec<AmlValue>),
    Arg(u8),
}

impl AmlNode {
    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            AmlNode::Scope { name, children } => {
                out.push(0x10);
                encode_pkg(out, |pkg| {
                    encode_name_string(name, pkg);
                    encode_nodes(children, pkg);
                });
            }
            AmlNode::Device { name, children } => {
                out.extend_from_slice(&[0x5b, 0x82]);
                encode_pkg(out, |pkg| {
                    encode_name_string(name, pkg);
                    encode_nodes(children, pkg);
                });
            }
            AmlNode::Name { name, value } => {
                out.push(0x08);
                encode_name_string(name, out);
                value.encode(out);
            }
            AmlNode::Method {
                name,
                args,
                serialized,
                body,
            } => {
                out.extend_from_slice(&[0x14]);
                encode_pkg(out, |pkg| {
                    encode_name_string(name, pkg);
                    pkg.push(args | if *serialized { 0x08 } else { 0 });
                    encode_nodes(body, pkg);
                });
            }
            AmlNode::Return(value) => {
                out.push(0xa4);
                value.encode(out);
            }
        }
    }
}

impl AmlValue {
    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            AmlValue::Integer(value) => encode_integer(*value, out),
            AmlValue::String(value) => {
                out.push(0x0d);
                out.extend_from_slice(value.as_bytes());
                out.push(0);
            }
            AmlValue::EisaId(value) => {
                out.push(0x0c);
                out.extend_from_slice(&encode_eisa_id(value).to_le_bytes());
            }
            AmlValue::Buffer(bytes) => {
                out.push(0x11);
                encode_pkg(out, |pkg| {
                    encode_integer(bytes.len() as u64, pkg);
                    pkg.extend_from_slice(bytes);
                });
            }
            AmlValue::Package(values) => {
                out.push(0x12);
                encode_pkg(out, |pkg| {
                    pkg.push(values.len() as u8);
                    for value in values {
                        value.encode(pkg);
                    }
                });
            }
            AmlValue::Arg(index) => out.push(0x68 + index),
        }
    }
}

fn encode_nodes(nodes: &[AmlNode], out: &mut Vec<u8>) {
    for node in nodes {
        node.encode(out);
    }
}

fn encode_integer(value: u64, out: &mut Vec<u8>) {
    match value {
        0 => out.push(0x00),
        1 => out.push(0x01),
        value if u8::try_from(value).is_ok() => {
            out.push(0x0a);
            out.push(value as u8);
        }
        value if u16::try_from(value).is_ok() => {
            out.push(0x0b);
            out.extend_from_slice(&(value as u16).to_le_bytes());
        }
        value if u32::try_from(value).is_ok() => {
            out.push(0x0c);
            out.extend_from_slice(&(value as u32).to_le_bytes());
        }
        _ => {
            out.push(0x0e);
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn encode_name_string(name: &str, out: &mut Vec<u8>) {
    let mut path = name;
    if let Some(rest) = path.strip_prefix('\\') {
        out.push(b'\\');
        path = rest;
    }
    let segments: Vec<&str> = path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect();
    match segments.len() {
        0 => {}
        1 => out.extend_from_slice(&name_seg(segments[0])),
        2 => {
            out.push(0x2e);
            out.extend_from_slice(&name_seg(segments[0]));
            out.extend_from_slice(&name_seg(segments[1]));
        }
        len => {
            out.push(0x2f);
            out.push(len as u8);
            for segment in segments {
                out.extend_from_slice(&name_seg(segment));
            }
        }
    }
}

fn name_seg(segment: &str) -> [u8; 4] {
    let mut out = [b'_'; 4];
    for (index, byte) in segment.as_bytes().iter().take(4).enumerate() {
        out[index] = *byte;
    }
    out
}

fn encode_pkg(out: &mut Vec<u8>, body: impl FnOnce(&mut Vec<u8>)) {
    let mut pkg = Vec::new();
    body(&mut pkg);
    let length = pkg.len() + pkg_length_size(pkg.len());
    encode_pkg_length(length, out);
    out.extend_from_slice(&pkg);
}

fn encode_pkg_length(length: usize, out: &mut Vec<u8>) {
    if length < 0x40 {
        out.push(length as u8);
        return;
    }

    if length < 0x1_000 {
        out.push(((1u8) << 6) | (length as u8 & 0x0f));
        out.push((length >> 4) as u8);
        return;
    }

    if length < 0x10_0000 {
        out.push(((2u8) << 6) | (length as u8 & 0x0f));
        out.push((length >> 4) as u8);
        out.push((length >> 12) as u8);
        return;
    }

    out.push(((3u8) << 6) | (length as u8 & 0x0f));
    out.push((length >> 4) as u8);
    out.push((length >> 12) as u8);
    out.push((length >> 20) as u8);
}

fn pkg_length_size(body_len: usize) -> usize {
    let total = body_len + 1;
    if total < 0x40 {
        1
    } else if total < 0x1_000 {
        2
    } else if total < 0x10_0000 {
        3
    } else {
        4
    }
}

fn encode_eisa_id(id: &str) -> u32 {
    let bytes = id.as_bytes();
    assert_eq!(bytes.len(), 7, "EISA ID must be 7 characters");
    let manufacturer = ((bytes[0] - b'@') as u32) << 26
        | ((bytes[1] - b'@') as u32) << 21
        | ((bytes[2] - b'@') as u32) << 16;
    let product = u16::from_str_radix(&id[3..], 16).expect("invalid EISA product");
    manufacturer | u32::from(product.swap_bytes())
}

#[cfg(test)]
mod tests {
    use super::{AmlNode, AmlValue};

    #[test]
    fn encodes_simple_scope() {
        let node = AmlNode::Scope {
            name: "\\_SB".to_string(),
            children: vec![AmlNode::Name {
                name: "_STA".to_string(),
                value: AmlValue::Integer(0x0f),
            }],
        };
        let mut out = Vec::new();
        node.encode(&mut out);
        assert!(!out.is_empty());
        assert_eq!(out[0], 0x10);
    }
}
