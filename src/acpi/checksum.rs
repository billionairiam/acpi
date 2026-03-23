pub fn acpi_checksum(bytes: &[u8]) -> u8 {
    let sum = bytes.iter().fold(0u8, |acc, byte| acc.wrapping_add(*byte));
    (0u8).wrapping_sub(sum)
}

#[cfg(test)]
mod tests {
    use super::acpi_checksum;

    #[test]
    fn checksum_zeroes_sum() {
        let mut bytes = vec![1u8, 2, 3, 0];
        bytes[3] = acpi_checksum(&bytes);
        let sum = bytes.iter().fold(0u8, |acc, byte| acc.wrapping_add(*byte));
        assert_eq!(sum, 0);
    }
}
