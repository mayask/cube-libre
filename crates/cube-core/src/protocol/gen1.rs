//! GAN 356i API v1, adapted from Pau Oliva / Andy Fedotov's MIT
//! smartcube-web-bluetooth. See THIRD_PARTY_NOTICES.md for attribution.
//! All operations are GATT reads; there are no write commands in this driver.
use super::{Event, Face, Move, ProtocolError};
use crate::CubeState;
use aes::{
    Aes128,
    cipher::{Block, BlockDecrypt, KeyInit},
};

pub const SERVICE: &str = "0000fff0-0000-1000-8000-00805f9b34fb";
pub const DEVICE_INFO: &str = "0000180a-0000-1000-8000-00805f9b34fb";
pub const FIRMWARE: &str = "00002a28-0000-1000-8000-00805f9b34fb";
pub const HARDWARE: &str = "00002a23-0000-1000-8000-00805f9b34fb";
pub const STATE: &str = "0000fff5-0000-1000-8000-00805f9b34fb";
pub const TIMING: &str = "0000fff6-0000-1000-8000-00805f9b34fb";
pub const FACELETS: &str = "0000fff2-0000-1000-8000-00805f9b34fb";
pub const BATTERY: &str = "0000fff7-0000-1000-8000-00805f9b34fb";

pub struct Cipher(Aes128);
impl Cipher {
    pub fn new(firmware: &[u8], hardware: &[u8]) -> Result<Self, ProtocolError> {
        if firmware.len() < 3 || hardware.len() < 6 {
            return Err(ProtocolError::Firmware);
        }
        let version =
            (u32::from(firmware[0]) << 16) | (u32::from(firmware[1]) << 8) | u32::from(firmware[2]);
        // Only the known encrypted v1 firmware family. Do not guess for other firmware.
        if version <= 0x010007 || version & 0xfffe00 != 0x010000 {
            return Err(ProtocolError::Firmware);
        }
        // Decoded from the upstream LZ-string constants. Device ID addition is mod 256,
        // not the mod 255 MAC salting used by generations 2–4.
        let mut key = if firmware[1] == 0 {
            [
                198u8, 202, 21, 223, 79, 110, 19, 182, 119, 13, 230, 89, 58, 175, 186, 162,
            ]
        } else {
            [
                67u8, 226, 91, 214, 125, 220, 120, 216, 7, 96, 163, 218, 130, 60, 1, 241,
            ]
        };
        for i in 0..6 {
            key[i] = key[i].wrapping_add(hardware[5 - i]);
        }
        Ok(Self(Aes128::new_from_slice(&key).expect("16-byte key")))
    }
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        if data.len() < 16 {
            return Err(ProtocolError::Length);
        }
        let mut out = data.to_vec();
        if out.len() > 16 {
            let offset = out.len() - 16;
            self.block(&mut out, offset);
        }
        self.block(&mut out, 0);
        Ok(out)
    }
    fn block(&self, data: &mut [u8], offset: usize) {
        let mut block = Block::<Aes128>::default();
        block.copy_from_slice(&data[offset..offset + 16]);
        self.0.decrypt_block(&mut block);
        data[offset..offset + 16].copy_from_slice(&block);
    }
}

pub fn counter(state: &[u8]) -> Result<u8, ProtocolError> {
    if state.len() < 19 {
        return Err(ProtocolError::Length);
    }
    if state[13..19].iter().any(|&code| code >= 18) {
        return Err(ProtocolError::Field);
    }
    Ok(state[12])
}
pub fn battery(data: &[u8]) -> Result<Event, ProtocolError> {
    let level = *data.get(7).ok_or(ProtocolError::Length)?;
    if level > 100 {
        return Err(ProtocolError::Field);
    }
    Ok(Event::Battery(level))
}
pub fn facelets(data: &[u8]) -> Result<CubeState, ProtocolError> {
    if data.len() < 18 {
        return Err(ProtocolError::Length);
    }
    let mut colors = String::with_capacity(54);
    for face in 0..6 {
        let i = face * 3;
        let packed = (u32::from(data[i ^ 1]) << 16)
            | (u32::from(data[(i + 1) ^ 1]) << 8)
            | u32::from(data[(i + 2) ^ 1]);
        for index in 0..8 {
            let code = ((packed >> (21 - 3 * index)) & 7) as usize;
            colors.push_str(Face::ALL.get(code).ok_or(ProtocolError::Field)?.code());
            if index == 3 {
                colors.push_str(Face::ALL[face].code());
            }
        }
    }
    Ok(CubeState::from_facelets(&colors)?)
}

#[derive(Default)]
pub struct Decoder {
    last_counter: Option<u8>,
}
impl Decoder {
    /// Gen1 facelets carry no serial. Bracket their read with two counter reads,
    /// and publish only if the cube did not move during the read. Never label a
    /// facelet frame with a guessed counter (including a hardcoded zero).
    pub fn snapshot(
        &mut self,
        before: &[u8],
        colors: &[u8],
        after: &[u8],
    ) -> Result<Option<Event>, ProtocolError> {
        let before = counter(before)?;
        let after = counter(after)?;
        if before != after {
            return Ok(None);
        }
        let state = facelets(colors)?;
        self.last_counter = Some(after);
        Ok(Some(Event::Snapshot {
            counter: after,
            state,
        }))
    }
    pub fn moves(&mut self, state: &[u8], timing: &[u8]) -> Result<Vec<Event>, ProtocolError> {
        let current = counter(state)?;
        if timing.len() < 19 {
            return Err(ProtocolError::Length);
        }
        let Some(previous) = self.last_counter else {
            return Ok(vec![]);
        };
        let delta = current.wrapping_sub(previous);
        if delta == 0 || delta >= 128 {
            return Ok(vec![]);
        }
        let count = usize::from(delta.min(6));
        let mut events = Vec::with_capacity(count);
        for i in (0..count).rev() {
            let code = state[18 - i];
            let stamp = 17 - 2 * i;
            events.push(Event::Move {
                counter: current.wrapping_sub(i as u8),
                // Device timing sample, as in the upstream Gen1 API; not a wall clock.
                cube_time_ms: u32::from(u16::from_le_bytes([timing[stamp], timing[stamp + 1]])),
                movement: Move {
                    face: Face::ALL[usize::from(code / 3)],
                    turns: code % 3 + 1,
                },
            });
        }
        self.last_counter = Some(current);
        Ok(events)
    }
}
