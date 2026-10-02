//! MIT protocol adaptation: Andy Fedotov, gan-web-bluetooth; see notices.
use super::{Event, Face, Move, ProtocolError, ascii, snapshot, word};

#[derive(Default)]
pub(super) struct Decoder {
    last_counter: Option<u8>,
    cube_time_ms: u32,
}
impl Decoder {
    pub(super) fn decode(&mut self, data: &[u8]) -> Result<Vec<Event>, ProtocolError> {
        if data.len() != 20 {
            return Err(ProtocolError::Length);
        }
        match data[0] >> 4 {
            1 => Ok(vec![Event::Unknown(1)]), // Optional gyro; camera remains manual.
            2 => {
                // Validate the entire seven-move window before changing decoder state.
                let mut moves = [Move::quarter(Face::U, false); 7];
                let mut elapsed = [0u32; 7];
                for i in 0..7 {
                    let face = word(data, 12 + 5 * i, 4)? as usize;
                    let face = *Face::ALL.get(face).ok_or(ProtocolError::Field)?;
                    moves[i] = Move::quarter(face, word(data, 16 + 5 * i, 1)? != 0);
                    elapsed[i] = word(data, 47 + 16 * i, 16)?;
                }
                let counter = word(data, 4, 8)? as u8;
                let Some(previous) = self.last_counter else {
                    return Ok(vec![]);
                };
                let delta = counter.wrapping_sub(previous);
                if delta == 0 || delta >= 128 {
                    return Ok(vec![]);
                }
                let count = usize::from(delta.min(7));
                let mut events = Vec::with_capacity(count);
                for i in (0..count).rev() {
                    self.cube_time_ms = self.cube_time_ms.wrapping_add(elapsed[i]);
                    events.push(Event::Move {
                        counter: counter.wrapping_sub(i as u8),
                        cube_time_ms: self.cube_time_ms,
                        movement: moves[i],
                    });
                }
                self.last_counter = Some(counter);
                Ok(events)
            }
            4 => {
                let counter = word(data, 4, 8)? as u8;
                let state = snapshot(data, 12, 33, 47, 91)?;
                if self
                    .last_counter
                    .is_none_or(|old| counter.wrapping_sub(old) < 128)
                {
                    self.last_counter = Some(counter);
                }
                Ok(vec![Event::Snapshot { counter, state }])
            }
            5 => Ok(vec![
                Event::HardwareName(ascii(&data[5..13])?),
                Event::HardwareVersion(format!("{}.{}", data[1], data[2])),
                Event::Firmware(format!("{}.{}", data[3], data[4])),
            ]),
            9 if data[1] <= 100 => Ok(vec![Event::Battery(data[1])]),
            9 => Err(ProtocolError::Field),
            13 => Ok(vec![Event::Disconnect]),
            _ => Err(ProtocolError::Field),
        }
    }
}
