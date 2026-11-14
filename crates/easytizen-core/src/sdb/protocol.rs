//! Wire format of SDB (Smart Development Bridge) transport messages.
//!
//! SDB is Tizen's bridge protocol. It is *not* ADB: the version word, the connection banner,
//! the service names and the device-side commands differ, and an Android `adb` client cannot
//! talk to a Samsung TV. The framing, however, is the same 24-byte little-endian header:
//!
//! ```text
//! command | arg0 | arg1 | data_length | data_check | magic (= command ^ 0xffffffff)
//! ```
//!
//! Constants were checked against the official `sdb` 4.2.36 client distributed with Tizen SDK.

pub const A_CNXN: u32 = 0x4e58_4e43;
pub const A_OPEN: u32 = 0x4e45_504f;
pub const A_OKAY: u32 = 0x5941_4b4f;
pub const A_CLSE: u32 = 0x4553_4c43;
pub const A_WRTE: u32 = 0x4554_5257;
/// Android's authentication request. A Samsung TV never sends it; seeing it means the address
/// belongs to an Android device, not a Tizen one.
pub const A_AUTH: u32 = 0x4854_5541;

/// SDB protocol version announced by the host.
pub const SDB_VERSION: u32 = 0x0200_0000;
/// Payload size we announce. Conservative on purpose: every sdbd generation accepts it.
pub const ANNOUNCED_MAX_PAYLOAD: u32 = 4096;
/// Never send more than this in one WRTE even when the device announces a larger buffer.
pub const SEND_PAYLOAD_CAP: usize = 64 * 1024;
/// Refuse absurd incoming frames (memory-exhaustion guard).
pub const RECV_PAYLOAD_LIMIT: usize = 1024 * 1024;

pub const HEADER_LEN: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub command: u32,
    pub arg0: u32,
    pub arg1: u32,
    pub data: Vec<u8>,
}

pub fn checksum(data: &[u8]) -> u32 {
    data.iter().fold(0u32, |acc, b| acc.wrapping_add(*b as u32))
}

impl Message {
    pub fn new(command: u32, arg0: u32, arg1: u32, data: impl Into<Vec<u8>>) -> Self {
        Self {
            command,
            arg0,
            arg1,
            data: data.into(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.data.len());
        for word in [
            self.command,
            self.arg0,
            self.arg1,
            self.data.len() as u32,
            checksum(&self.data),
            self.command ^ 0xffff_ffff,
        ] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&self.data);
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub command: u32,
    pub arg0: u32,
    pub arg1: u32,
    pub data_length: u32,
    pub data_check: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum HeaderError {
    BadMagic,
    TooLarge,
}

pub fn decode_header(raw: &[u8; HEADER_LEN]) -> Result<Header, HeaderError> {
    let word = |i: usize| u32::from_le_bytes([raw[i], raw[i + 1], raw[i + 2], raw[i + 3]]);
    let command = word(0);
    if word(20) != command ^ 0xffff_ffff {
        return Err(HeaderError::BadMagic);
    }
    let data_length = word(12);
    if data_length as usize > RECV_PAYLOAD_LIMIT {
        return Err(HeaderError::TooLarge);
    }
    Ok(Header {
        command,
        arg0: word(4),
        arg1: word(8),
        data_length,
        data_check: word(16),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cnxn_frame_matches_the_documented_layout() {
        let m = Message::new(
            A_CNXN,
            SDB_VERSION,
            ANNOUNCED_MAX_PAYLOAD,
            b"host::\0".to_vec(),
        );
        let raw = m.encode();
        assert_eq!(&raw[0..4], b"CNXN");
        assert_eq!(&raw[4..8], &[0, 0, 0, 2]);
        assert_eq!(u32::from_le_bytes(raw[12..16].try_into().unwrap()), 7);
        // 'h'+'o'+'s'+'t'+':'+':'+0
        assert_eq!(u32::from_le_bytes(raw[16..20].try_into().unwrap()), 562);
        assert_eq!(&raw[20..24], &[!b'C', !b'N', !b'X', !b'N']);
        let header = decode_header(raw[..24].try_into().unwrap()).unwrap();
        assert_eq!((header.command, header.data_length), (A_CNXN, 7));
    }

    #[test]
    fn rejects_bad_magic_and_oversized_frames() {
        let mut raw = Message::new(A_OKAY, 1, 2, vec![]).encode();
        raw[20] ^= 1;
        assert_eq!(
            decode_header(raw[..24].try_into().unwrap()),
            Err(HeaderError::BadMagic)
        );
        let mut big = Message::new(A_WRTE, 1, 2, vec![]).encode();
        big[12..16].copy_from_slice(&(RECV_PAYLOAD_LIMIT as u32 + 1).to_le_bytes());
        assert_eq!(
            decode_header(big[..24].try_into().unwrap()),
            Err(HeaderError::TooLarge)
        );
    }
}
