//! MIT protocol adaptation: Andy Fedotov, gan-web-bluetooth; see notices.
use super::{Event, Face, Move, ProtocolError, ascii, snapshot};

pub(super) fn decode(data: &[u8]) -> Result<Vec<Event>, ProtocolError> {
    if data.len() < 3 {
        return Err(ProtocolError::Length);
    }
    if data[0] != 0x55 || data[2] == 0 {
        return Err(ProtocolError::Field);
    }
    let end = 3 + usize::from(data[2]);
    let p = data.get(..end).ok_or(ProtocolError::Length)?;
    let event = match data[1] {
        1 => {
            if p.len() < 10 {
                return Err(ProtocolError::Length);
            }
            let face = [2, 32, 8, 1, 16, 4]
                .iter()
                .position(|&v| v == p[9] & 63)
                .ok_or(ProtocolError::Field)?;
            let direction = p[9] >> 6;
            if direction > 1 {
                return Err(ProtocolError::Field);
            }
            Event::Move {
                // The shared history/reconciliation window is modulo 256, as upstream.
                counter: p[7],
                cube_time_ms: u32::from_le_bytes(p[3..7].try_into().unwrap()),
                movement: Move::quarter(Face::ALL[face], direction == 1),
            }
        }
        2 => Event::Snapshot {
            counter: *p.get(3).ok_or(ProtocolError::Length)?,
            state: snapshot(p, 40, 61, 77, 121)?,
        },
        7 => {
            if p.len() < 11 {
                return Err(ProtocolError::Length);
            }
            return Ok(vec![
                Event::HardwareName(ascii(&p[4..9])?),
                Event::Firmware(format!("{}.{}", p[9] >> 4, p[9] & 15)),
                Event::HardwareVersion(format!("{}.{}", p[10] >> 4, p[10] & 15)),
            ]);
        }
        0x10 => {
            let level = *p.get(3).ok_or(ProtocolError::Length)?;
            if level > 100 {
                return Err(ProtocolError::Field);
            }
            Event::Battery(level)
        }
        0x11 => Event::Disconnect,
        other => Event::Unknown(other), // History is repaired by snapshots, not fabricated.
    };
    Ok(vec![event])
}
