//! GAN Gen1–Gen4 protocols. Adapted from gan-web-bluetooth and its MIT fork
//! smartcube-web-bluetooth; see THIRD_PARTY_NOTICES.md.
//! The only outbound commands exposed here are read-only requests.
use crate::{
    CubeState, Face, Move,
    cube::{CubeError, Cubies},
};
use aes::{
    Aes128,
    cipher::{Block, BlockDecrypt, BlockEncrypt, KeyInit},
};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use thiserror::Error;

pub mod gen1;
mod gen2;
mod gen3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generation {
    Gen1,
    Gen2,
    Gen3,
    Gen4,
}
impl Generation {
    pub const ALL: [Self; 4] = [Self::Gen1, Self::Gen2, Self::Gen3, Self::Gen4];
    pub fn label(self) -> &'static str {
        match self {
            Self::Gen1 => "GAN Gen1",
            Self::Gen2 => "GAN Gen2",
            Self::Gen3 => "GAN Gen3",
            Self::Gen4 => "GAN Gen4",
        }
    }
    pub fn service(self) -> &'static str {
        match self {
            Self::Gen1 => gen1::SERVICE,
            Self::Gen2 => "6e400001-b5a3-f393-e0a9-e50e24dc4179",
            Self::Gen3 => "8653000a-43e6-47b7-9cb0-5fc21d4ae340",
            Self::Gen4 => "00000010-0000-fff7-fff6-fff5fff4fff0",
        }
    }
    /// Gen1 reads GATT values; later generations use a command/notify pair.
    pub fn notification_profile(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Gen1 => None,
            Self::Gen2 => Some((
                "28be4a4a-cd67-11e9-a32f-2a2ae2dbcce4",
                "28be4cb6-cd67-11e9-a32f-2a2ae2dbcce4",
            )),
            Self::Gen3 => Some((
                "8653000c-43e6-47b7-9cb0-5fc21d4ae340",
                "8653000b-43e6-47b7-9cb0-5fc21d4ae340",
            )),
            Self::Gen4 => Some((
                "0000fff5-0000-1000-8000-00805f9b34fb",
                "0000fff6-0000-1000-8000-00805f9b34fb",
            )),
        }
    }
    /// Service discovery is authoritative, never a guess based on model name.
    pub fn from_services(services: &[&str]) -> Option<Self> {
        Self::ALL
            .into_iter()
            .rev()
            .find(|g| services.iter().any(|s| s.eq_ignore_ascii_case(g.service())))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyProfile {
    Gan,
    /// The AiCube / MoYu AI 2023 variant of the Gen2 wire protocol.
    MoyuAi2023,
}

pub struct Decoder {
    generation: Generation,
    gen2: gen2::Decoder,
}
impl Decoder {
    pub fn new(generation: Generation) -> Self {
        Self {
            generation,
            gen2: gen2::Decoder::default(),
        }
    }
    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<Event>, ProtocolError> {
        match self.generation {
            Generation::Gen1 => Err(ProtocolError::Field),
            Generation::Gen2 => self.gen2.decode(data),
            Generation::Gen3 => gen3::decode(data),
            Generation::Gen4 => decode(data).map(|event| vec![event]),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("Enter the cube's hardware address as six hexadecimal pairs (AA:BB:CC:DD:EE:FF)")]
    Mac,
    #[error("Truncated or invalid GAN packet length")]
    Length,
    #[error("Invalid GAN packet field")]
    Field,
    #[error("Unsupported GAN Gen1 firmware or invalid device-information key data")]
    Firmware,
    #[error(transparent)]
    Cube(#[from] CubeError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MacAddress([u8; 6]);
impl MacAddress {
    pub fn bytes(self) -> [u8; 6] {
        self.0
    }
    /// GAN advertises the real MAC in reverse order at the end of manufacturer data.
    /// This also works on Apple platforms where the OS exposes an opaque device ID.
    pub fn from_manufacturer(company_id: u16, data: &[u8]) -> Option<Self> {
        if company_id & 0xff != 1 || data.len() < 6 {
            return None;
        }
        let mut bytes: [u8; 6] = data[data.len() - 6..].try_into().ok()?;
        bytes.reverse();
        (bytes != [0; 6] && bytes != [255; 6]).then_some(Self(bytes))
    }
}
impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}
impl FromStr for MacAddress {
    type Err = ProtocolError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let compact: String = s.trim().chars().filter(|&c| c != ':' && c != '-').collect();
        if compact.len() != 12 || !compact.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ProtocolError::Mac);
        }
        let mut bytes = [0; 6];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&compact[i * 2..i * 2 + 2], 16)
                .map_err(|_| ProtocolError::Mac)?;
        }
        if bytes == [0; 6] || bytes == [255; 6] {
            return Err(ProtocolError::Mac);
        }
        Ok(Self(bytes))
    }
}
impl TryFrom<String> for MacAddress {
    type Error = ProtocolError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}
impl From<MacAddress> for String {
    fn from(mac: MacAddress) -> Self {
        mac.to_string()
    }
}

pub struct GanCipher {
    cipher: Aes128,
    iv: [u8; 16],
}
impl GanCipher {
    pub fn new(mac: MacAddress) -> Self {
        Self::for_profile(mac, KeyProfile::Gan)
    }
    pub fn for_profile(mac: MacAddress, profile: KeyProfile) -> Self {
        let mut key = [
            0x01, 0x02, 0x42, 0x28, 0x31, 0x91, 0x16, 0x07, 0x20, 0x05, 0x18, 0x54, 0x42, 0x11,
            0x12, 0x53,
        ];
        let mut iv = [
            0x11, 0x03, 0x32, 0x28, 0x21, 0x01, 0x76, 0x27, 0x20, 0x95, 0x78, 0x14, 0x32, 0x12,
            0x02, 0x43,
        ];
        if profile == KeyProfile::MoyuAi2023 {
            key = [
                0x05, 0x12, 0x02, 0x45, 0x02, 0x01, 0x29, 0x56, 0x12, 0x78, 0x12, 0x76, 0x81, 0x01,
                0x08, 0x03,
            ];
            iv = [
                0x01, 0x44, 0x28, 0x06, 0x86, 0x21, 0x22, 0x28, 0x51, 0x05, 0x08, 0x31, 0x82, 0x02,
                0x21, 0x06,
            ];
        }
        for (i, salt) in mac.0.iter().rev().enumerate() {
            key[i] = ((key[i] as u16 + *salt as u16) % 255) as u8;
            iv[i] = ((iv[i] as u16 + *salt as u16) % 255) as u8;
        }
        Self {
            cipher: Aes128::new_from_slice(&key).expect("16-byte key"),
            iv,
        }
    }
    fn chunk(&self, data: &mut [u8], offset: usize, encrypt: bool) {
        let mut block = Block::<Aes128>::default();
        block.copy_from_slice(&data[offset..offset + 16]);
        if encrypt {
            for (b, v) in block.iter_mut().zip(self.iv) {
                *b ^= v;
            }
            self.cipher.encrypt_block(&mut block);
        } else {
            self.cipher.decrypt_block(&mut block);
            for (b, v) in block.iter_mut().zip(self.iv) {
                *b ^= v;
            }
        }
        data[offset..offset + 16].copy_from_slice(&block);
    }
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        if data.len() < 16 {
            return Err(ProtocolError::Length);
        }
        let mut out = data.to_vec();
        self.chunk(&mut out, 0, true);
        if data.len() > 16 {
            self.chunk(&mut out, data.len() - 16, true);
        }
        Ok(out)
    }
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        if data.len() < 16 {
            return Err(ProtocolError::Length);
        }
        let mut out = data.to_vec();
        if data.len() > 16 {
            self.chunk(&mut out, data.len() - 16, false);
        }
        self.chunk(&mut out, 0, false);
        Ok(out)
    }
    pub fn request(
        &self,
        generation: Generation,
        request: ReadRequest,
    ) -> Result<Vec<u8>, ProtocolError> {
        let prefix: &[u8] = match (generation, request) {
            (Generation::Gen1, _) => return Err(ProtocolError::Field),
            (Generation::Gen2, ReadRequest::State) => &[0x04],
            (Generation::Gen2, ReadRequest::Battery) => &[0x09],
            (Generation::Gen2, ReadRequest::Hardware) => &[0x05],
            (Generation::Gen3, ReadRequest::State) => &[0x68, 0x01],
            (Generation::Gen3, ReadRequest::Battery) => &[0x68, 0x07],
            (Generation::Gen3, ReadRequest::Hardware) => &[0x68, 0x04],
            (Generation::Gen4, ReadRequest::State) => &[0xdd, 4, 0, 0xed, 0, 0],
            (Generation::Gen4, ReadRequest::Battery) => &[0xdd, 4, 0, 0xef, 0, 0],
            (Generation::Gen4, ReadRequest::Hardware) => &[0xdf, 3, 0, 0, 0],
        };
        let mut frame = vec![
            0;
            if generation == Generation::Gen3 {
                16
            } else {
                20
            }
        ];
        frame[..prefix.len()].copy_from_slice(prefix);
        self.encrypt(&frame)
    }
}
#[derive(Debug, Clone, Copy)]
pub enum ReadRequest {
    State,
    Battery,
    Hardware,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Move {
        counter: u8,
        cube_time_ms: u32,
        movement: Move,
    },
    Snapshot {
        counter: u8,
        state: CubeState,
    },
    Battery(u8),
    HardwareName(String),
    HardwareVersion(String),
    Firmware(String),
    ProductDate(String),
    Disconnect,
    Unknown(u8),
}
fn ascii(data: &[u8]) -> Result<String, ProtocolError> {
    if !data
        .iter()
        .all(|b| *b == 0 || b.is_ascii_graphic() || *b == b' ')
    {
        return Err(ProtocolError::Field);
    }
    Ok(String::from_utf8_lossy(data)
        .trim_end_matches('\0')
        .to_owned())
}
fn snapshot(
    data: &[u8],
    cp: usize,
    co: usize,
    ep: usize,
    eo: usize,
) -> Result<CubeState, ProtocolError> {
    let mut c = Cubies::solved();
    for i in 0..7 {
        c.cp[i] = word(data, cp + 3 * i, 3)? as u8;
        c.co[i] = word(data, co + 2 * i, 2)? as u8;
    }
    c.cp[7] = 28u8
        .checked_sub(c.cp[..7].iter().sum())
        .ok_or(ProtocolError::Field)?;
    c.co[7] = (3 - c.co[..7].iter().sum::<u8>() % 3) % 3;
    for i in 0..11 {
        c.ep[i] = word(data, ep + 4 * i, 4)? as u8;
        c.eo[i] = word(data, eo + i, 1)? as u8;
    }
    c.ep[11] = 66u8
        .checked_sub(c.ep[..11].iter().sum())
        .ok_or(ProtocolError::Field)?;
    c.eo[11] = (2 - c.eo[..11].iter().sum::<u8>() % 2) % 2;
    Ok(CubeState::from_cubies(&c)?)
}
fn word(data: &[u8], start: usize, count: usize) -> Result<u32, ProtocolError> {
    if count > 32 || start + count > data.len() * 8 {
        return Err(ProtocolError::Length);
    }
    Ok((start..start + count).fold(0, |v, i| {
        (v << 1) | ((data[i / 8] >> (7 - i % 8)) & 1) as u32
    }))
}

pub fn decode(data: &[u8]) -> Result<Event, ProtocolError> {
    if data.len() < 2 {
        return Err(ProtocolError::Length);
    }
    let len = data[1] as usize;
    if len + 2 > data.len() {
        return Err(ProtocolError::Length);
    }
    let payload = &data[..len + 2];
    match data[0] {
        0x01 => {
            if payload.len() < 9 {
                return Err(ProtocolError::Length);
            }
            let face = [2, 32, 8, 1, 16, 4]
                .iter()
                .position(|&v| v == data[8] & 63)
                .ok_or(ProtocolError::Field)?;
            let dir = data[8] >> 6;
            if dir > 1 {
                return Err(ProtocolError::Field);
            }
            Ok(Event::Move {
                counter: data[6],
                cube_time_ms: u32::from_le_bytes(data[2..6].try_into().unwrap()),
                movement: Move::quarter(Face::ALL[face], dir == 1),
            })
        }
        0xed => {
            if payload.len() < 16 {
                return Err(ProtocolError::Length);
            }
            Ok(Event::Snapshot {
                counter: data[2],
                state: snapshot(payload, 32, 53, 69, 113)?,
            })
        }
        0xef => {
            if len == 0 || payload[1 + len] > 100 {
                return Err(ProtocolError::Field);
            }
            Ok(Event::Battery(payload[1 + len]))
        }
        0xfc => {
            if payload.len() < 4 {
                return Err(ProtocolError::Length);
            }
            let name = &payload[3..];
            if !name
                .iter()
                .all(|b| *b == 0 || b.is_ascii_graphic() || *b == b' ')
            {
                return Err(ProtocolError::Field);
            }
            Ok(Event::HardwareName(
                String::from_utf8_lossy(name)
                    .trim_end_matches('\0')
                    .to_owned(),
            ))
        }
        0xfd | 0xfe => {
            if payload.len() < 4 {
                return Err(ProtocolError::Length);
            }
            let version = format!("{}.{}", payload[3] >> 4, payload[3] & 15);
            Ok(if data[0] == 0xfd {
                Event::Firmware(version)
            } else {
                Event::HardwareVersion(version)
            })
        }
        0xfa => {
            if payload.len() < 7 {
                return Err(ProtocolError::Length);
            }
            let year = u16::from_le_bytes([data[3], data[4]]);
            if !(2000..=2100).contains(&year)
                || !(1..=12).contains(&data[5])
                || !(1..=31).contains(&data[6])
            {
                return Err(ProtocolError::Field);
            }
            Ok(Event::ProductDate(format!(
                "{year:04}-{:02}-{:02}",
                data[5], data[6]
            )))
        }
        0xea => Ok(Event::Disconnect),
        other => Ok(Event::Unknown(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
    #[test]
    fn openssl_golden_vectors_check_salt_cbc_and_overlap() {
        // Independently generated with OpenSSL AES-128-CBC, no padding,
        // first block then overlapping final block. MAC 01:02:03:04:05:06.
        let cipher = GanCipher::new("01:02:03:04:05:06".parse().unwrap());
        let expected = hex("5c67ba9856dc67e7113c83fe8cc31dc72345ddf9");
        assert_eq!(
            cipher
                .request(Generation::Gen4, ReadRequest::State)
                .unwrap(),
            expected
        );
        assert_eq!(
            cipher.decrypt(&expected).unwrap(),
            hex("dd0400ed00000000000000000000000000000000")
        );
        let encrypted = hex("ac76652d900f5aee16d088cd60bf4c8fd68ee35f");
        let plain = cipher.decrypt(&encrypted).unwrap();
        assert_eq!(plain, hex("ed0e2a000539700000091a2b3c4d000000000000"));
        assert_eq!(
            decode(&plain).unwrap(),
            Event::Snapshot {
                counter: 42,
                state: CubeState::solved()
            }
        );
    }
    #[test]
    fn validates_mac_and_apple_advertisement_fallback() {
        let mac: MacAddress = "12-34-56-78-9a-bc".parse().unwrap();
        assert_eq!(mac.to_string(), "12:34:56:78:9A:BC");
        assert_eq!(
            MacAddress::from_manufacturer(1, &hex("000000bc9a78563412")),
            Some(mac)
        );
        assert!(MacAddress::from_manufacturer(0x4c, &hex("000000bc9a78563412")).is_none());
        for bad in [
            "",
            "00:00:00:00:00:00",
            "ff:ff:ff:ff:ff:ff",
            "Z1:02:03:04:05:06",
            "🧊🧊🧊",
        ] {
            assert!(bad.parse::<MacAddress>().is_err());
        }
    }
    #[test]
    fn all_face_turns_and_counters_decode() {
        for (i, code) in [2, 32, 8, 1, 16, 4].into_iter().enumerate() {
            for dir in 0..2 {
                let mut p = [0; 20];
                p[..9].copy_from_slice(&[1, 7, 0x78, 0x56, 0x34, 0x12, 255, 0, code | (dir << 6)]);
                assert_eq!(
                    decode(&p).unwrap(),
                    Event::Move {
                        counter: 255,
                        cube_time_ms: 0x12345678,
                        movement: Move::quarter(Face::ALL[i], dir == 1)
                    }
                );
            }
        }
        assert_eq!(decode(&[0xef, 3, 0, 0, 92]).unwrap(), Event::Battery(92));
        assert!(decode(&[0xef, 3, 0, 0, 200]).is_err());
        assert_eq!(decode(&[0xff, 1, 0]).unwrap(), Event::Unknown(0xff));
    }
    #[test]
    fn malformed_packets_never_panic() {
        for size in 0..40 {
            for value in 0..=255u8 {
                let mut p = vec![value; size];
                if size > 1 {
                    p[1] = size.saturating_sub(2) as u8;
                }
                let _ = decode(&p);
            }
        }
        assert!(
            GanCipher::new("01:02:03:04:05:06".parse().unwrap())
                .decrypt(&[0; 15])
                .is_err()
        );
    }
}
