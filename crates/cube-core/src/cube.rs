use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Face {
    U,
    R,
    F,
    D,
    L,
    B,
}

impl Face {
    pub const ALL: [Face; 6] = [Self::U, Self::R, Self::F, Self::D, Self::L, Self::B];
    pub fn code(self) -> &'static str {
        ["U", "R", "F", "D", "L", "B"][self as usize]
    }
    pub fn color_name(self) -> &'static str {
        ["White", "Red", "Green", "Yellow", "Orange", "Blue"][self as usize]
    }
    pub fn css_color(self) -> &'static str {
        [
            "#f0f2f5", "#ef5261", "#36c68e", "#f4d35e", "#f89a4a", "#548cf1",
        ][self as usize]
    }
}
impl fmt::Display for Face {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub face: Face,
    pub turns: u8,
}
impl Move {
    pub fn quarter(face: Face, inverse: bool) -> Self {
        Self {
            face,
            turns: if inverse { 3 } else { 1 },
        }
    }
    pub fn inverse(self) -> Self {
        Self {
            face: self.face,
            turns: (4 - self.turns % 4) % 4,
        }
    }
}
impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            self.face,
            match self.turns {
                2 => "2",
                3 => "'",
                _ => "",
            }
        )
    }
}
impl FromStr for Move {
    type Err = CubeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        let face = Face::ALL
            .into_iter()
            .find(|f| Some(f.code().as_bytes()[0]) == bytes.first().copied())
            .ok_or(CubeError::InvalidMove)?;
        let turns = match &bytes[1..] {
            [] => 1,
            [b'2'] => 2,
            [b'\''] => 3,
            _ => return Err(CubeError::InvalidMove),
        };
        Ok(Self { face, turns })
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CubeError {
    #[error("Invalid cube move")]
    InvalidMove,
    #[error("Invalid cubie permutation")]
    Permutation,
    #[error("Invalid cubie orientation")]
    Orientation,
    #[error("Corner and edge parity do not match")]
    Parity,
}

/// Kociemba ordering: URF UFL ULB UBR DFR DLF DBL DRB;
/// UR UF UL UB DR DF DL DB FR FL BL BR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cubies {
    pub cp: [u8; 8],
    pub co: [u8; 8],
    pub ep: [u8; 12],
    pub eo: [u8; 12],
}
impl Cubies {
    pub fn solved() -> Self {
        Self {
            cp: std::array::from_fn(|i| i as u8),
            co: [0; 8],
            ep: std::array::from_fn(|i| i as u8),
            eo: [0; 12],
        }
    }
    pub fn validate(&self) -> Result<(), CubeError> {
        let mut cp = self.cp;
        cp.sort_unstable();
        let mut ep = self.ep;
        ep.sort_unstable();
        if cp != std::array::from_fn(|i| i as u8) || ep != std::array::from_fn(|i| i as u8) {
            return Err(CubeError::Permutation);
        }
        if self.co.iter().any(|&x| x >= 3)
            || self.eo.iter().any(|&x| x >= 2)
            || self.co.iter().map(|&x| x as u16).sum::<u16>() % 3 != 0
            || self.eo.iter().map(|&x| x as u16).sum::<u16>() % 2 != 0
        {
            return Err(CubeError::Orientation);
        }
        fn parity(p: &[u8]) -> usize {
            p.iter()
                .enumerate()
                .map(|(i, a)| p[i + 1..].iter().filter(|b| a > b).count())
                .sum::<usize>()
                % 2
        }
        if parity(&self.cp) != parity(&self.ep) {
            return Err(CubeError::Parity);
        }
        Ok(())
    }
}

// Adapted from gan-web-bluetooth, MIT; see THIRD_PARTY_NOTICES.md.
const CORNERS: [[usize; 3]; 8] = [
    [8, 9, 20],
    [6, 18, 38],
    [0, 36, 47],
    [2, 45, 11],
    [29, 26, 15],
    [27, 44, 24],
    [33, 53, 42],
    [35, 17, 51],
];
const EDGES: [[usize; 2]; 12] = [
    [5, 10],
    [7, 19],
    [3, 37],
    [1, 46],
    [32, 16],
    [28, 25],
    [30, 43],
    [34, 52],
    [23, 12],
    [21, 41],
    [50, 39],
    [48, 14],
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CubeState {
    stickers: [Face; 54],
}
impl CubeState {
    pub fn solved() -> Self {
        Self {
            stickers: std::array::from_fn(|i| Face::ALL[i / 9]),
        }
    }
    pub fn from_cubies(c: &Cubies) -> Result<Self, CubeError> {
        c.validate()?;
        let mut state = Self::solved();
        for (i, slots) in CORNERS.iter().enumerate() {
            for p in 0..3 {
                state.stickers[slots[(p + c.co[i] as usize) % 3]] =
                    Face::ALL[CORNERS[c.cp[i] as usize][p] / 9];
            }
        }
        for (i, slots) in EDGES.iter().enumerate() {
            for p in 0..2 {
                state.stickers[slots[(p + c.eo[i] as usize) % 2]] =
                    Face::ALL[EDGES[c.ep[i] as usize][p] / 9];
            }
        }
        Ok(state)
    }
    pub fn stickers(&self) -> &[Face; 54] {
        &self.stickers
    }
    pub fn is_solved(&self) -> bool {
        self.stickers
            .iter()
            .enumerate()
            .all(|(i, &f)| f == Face::ALL[i / 9])
    }
    pub fn facelet_string(&self) -> String {
        self.stickers.iter().map(|f| f.code()).collect()
    }
    /// Rotate facelets geometrically. This keeps the cube-fixed reference frame;
    /// changing the camera never invokes this method.
    pub fn apply(&mut self, movement: Move) {
        let normal = face_normal(movement.face);
        let axis = normal.iter().position(|&x| x != 0).expect("unit normal");
        let layer = normal[axis];
        for _ in 0..movement.turns % 4 {
            let before = self.stickers;
            for (i, &sticker) in before.iter().enumerate() {
                let (mut p, mut n) = sticker_geometry(i);
                if p[axis] != layer {
                    continue;
                }
                p = rotate(p, axis, -layer);
                n = rotate(n, axis, -layer);
                let dest = (0..54)
                    .find(|&j| sticker_geometry(j) == (p, n))
                    .expect("rotation stays on cube");
                self.stickers[dest] = sticker;
            }
        }
    }
}
fn face_normal(face: Face) -> [i8; 3] {
    [
        [0, 1, 0],
        [1, 0, 0],
        [0, 0, 1],
        [0, -1, 0],
        [-1, 0, 0],
        [0, 0, -1],
    ][face as usize]
}
fn sticker_geometry(index: usize) -> ([i8; 3], [i8; 3]) {
    let face = Face::ALL[index / 9];
    let row = ((index % 9) / 3) as i8;
    let col = (index % 3) as i8;
    let p = match face {
        Face::U => [col - 1, 1, row - 1],
        Face::R => [1, 1 - row, 1 - col],
        Face::F => [col - 1, 1 - row, 1],
        Face::D => [col - 1, -1, 1 - row],
        Face::L => [-1, 1 - row, col - 1],
        Face::B => [1 - col, 1 - row, -1],
    };
    (p, face_normal(face))
}
fn rotate([x, y, z]: [i8; 3], axis: usize, sign: i8) -> [i8; 3] {
    match axis {
        0 => [x, -sign * z, sign * y],
        1 => [sign * z, y, -sign * x],
        _ => [-sign * y, sign * x, z],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn four_turns_and_inverses_restore_every_face() {
        for face in Face::ALL {
            let mut c = CubeState::solved();
            for _ in 0..4 {
                c.apply(Move::quarter(face, false));
            }
            assert!(c.is_solved());
            c.apply(Move::quarter(face, false));
            c.apply(Move::quarter(face, true));
            assert!(c.is_solved());
        }
    }
    #[test]
    fn geometric_turns_match_independent_gan_reference() {
        let cubies = Cubies {
            cp: [0, 5, 2, 1, 7, 4, 6, 3],
            co: [1, 2, 0, 2, 1, 1, 0, 2],
            ep: [1, 9, 2, 3, 11, 8, 6, 7, 4, 5, 10, 0],
            eo: [1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0],
        };
        let expected = "UUFUUFLLFUUURRRRRRFFRFFDFFDRRBDDBDDBLLDLLDLLDLBBUBBUBB";
        assert_eq!(
            CubeState::from_cubies(&cubies).unwrap().facelet_string(),
            expected
        );
        let mut cube = CubeState::solved();
        cube.apply("F".parse().unwrap());
        cube.apply("R".parse().unwrap());
        assert_eq!(cube.facelet_string(), expected);
    }
    #[test]
    fn long_sequence_keeps_centers_counts_and_is_reversible() {
        let mut cube = CubeState::solved();
        let sequence: Vec<Move> = (0..503)
            .map(|i| Move {
                face: Face::ALL[(i * 17 + i / 9) % 6],
                turns: (i % 3 + 1) as u8,
            })
            .collect();
        for &m in &sequence {
            cube.apply(m);
        }
        for f in Face::ALL {
            assert_eq!(cube.stickers.iter().filter(|&&x| x == f).count(), 9);
            assert_eq!(cube.stickers[f as usize * 9 + 4], f);
        }
        for m in sequence.into_iter().rev() {
            cube.apply(m.inverse());
        }
        assert!(cube.is_solved());
    }
    #[test]
    fn reject_impossible_states_and_invalid_moves() {
        let mut c = Cubies::solved();
        c.cp[0] = 1;
        assert_eq!(c.validate(), Err(CubeError::Permutation));
        c = Cubies::solved();
        c.co[0] = 1;
        assert_eq!(c.validate(), Err(CubeError::Orientation));
        c = Cubies::solved();
        c.ep.swap(0, 1);
        assert_eq!(c.validate(), Err(CubeError::Parity));
        for text in ["", "X", "U3", "r", "🧊", "R''"] {
            assert!(text.parse::<Move>().is_err());
        }
    }
}
