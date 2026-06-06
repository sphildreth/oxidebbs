//! CRC helpers for caller file-transfer protocols.

const XMODEM_POLY: u16 = 0x1021;

/// Compute CRC-16/XMODEM over a byte slice.
///
/// This is the CCITT polynomial `0x1021` with initial value `0x0000`, no input
/// or output reflection, and no final XOR.
#[must_use]
pub fn crc16_xmodem(bytes: &[u8]) -> u16 {
    let mut crc = 0_u16;
    for byte in bytes {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ XMODEM_POLY;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc16_xmodem_matches_standard_check_vector() {
        assert_eq!(crc16_xmodem(b"123456789"), 0x31C3);
    }

    #[test]
    fn crc16_xmodem_empty_is_zero() {
        assert_eq!(crc16_xmodem(b""), 0);
    }
}
