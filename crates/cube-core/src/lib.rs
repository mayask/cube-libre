//! Platform-independent GAN protocol, cube mathematics, and state reconciliation.
//! No radio, UI, filesystem or OS dependencies. See THIRD_PARTY_NOTICES.md.
pub mod cube;
pub mod protocol;
pub mod sync;

pub use cube::{CubeState, Face, Move};
pub use protocol::{GanCipher, MacAddress, ProtocolError};
