use cube_core::{CubeState, MacAddress, Move};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedDevice {
    /// Stable local identity: the real cube MAC, even on CoreBluetooth.
    pub id: String,
    pub name: String,
    pub peripheral_id: String,
    pub mac: MacAddress,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Registry {
    pub version: u32,
    pub devices: Vec<SavedDevice>,
    pub selected: Option<String>,
    pub auto_connect: bool,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            version: 1,
            devices: vec![],
            selected: None,
            auto_connect: true,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NearbyDevice {
    pub id: String,
    pub name: String,
    pub mac: Option<MacAddress>,
    pub rssi: Option<i16>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Searching,
    Connecting,
    Synchronizing,
    Connected,
    Retrying,
    Unsupported,
    Demo,
}
impl ConnectionStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Searching => "Looking for cube",
            Self::Connecting => "Connecting",
            Self::Synchronizing => "Synchronizing",
            Self::Connected => "Connected",
            Self::Retrying => "Reconnecting",
            Self::Unsupported => "Unsupported protocol",
            Self::Demo => "Demo",
        }
    }
    pub fn is_busy(self) -> bool {
        matches!(
            self,
            Self::Searching | Self::Connecting | Self::Synchronizing | Self::Retrying
        )
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HardwareInfo {
    pub name: Option<String>,
    pub firmware: Option<String>,
    pub hardware_version: Option<String>,
    pub product_date: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveRecord {
    pub movement: Move,
    pub counter: u8,
    pub cube_time_ms: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveCube {
    pub id: String,
    pub name: String,
    pub status: ConnectionStatus,
    pub detail: String,
    pub cube: Option<CubeState>,
    pub synced: bool,
    pub counter: Option<u8>,
    pub battery: Option<u8>,
    pub hardware: HardwareInfo,
    pub history: VecDeque<MoveRecord>,
    pub observed_turns: u64,
    pub missed_turns: u64,
    pub corrections: u64,
    pub last_update_ms: Option<u64>,
}
impl ActiveCube {
    pub fn new(device: &SavedDevice) -> Self {
        Self {
            id: device.id.clone(),
            name: device.name.clone(),
            status: ConnectionStatus::Disconnected,
            detail: "Ready to connect".into(),
            cube: None,
            synced: false,
            counter: None,
            battery: None,
            hardware: HardwareInfo::default(),
            history: VecDeque::new(),
            observed_turns: 0,
            missed_turns: 0,
            corrections: 0,
            last_update_ms: None,
        }
    }
    pub fn demo() -> Self {
        let device = SavedDevice {
            id: "demo".into(),
            name: "Practice cube".into(),
            peripheral_id: String::new(),
            mac: "01:02:03:04:05:06".parse().expect("demo address"),
        };
        let mut active = Self::new(&device);
        active.status = ConnectionStatus::Demo;
        active.detail = "Simulator only — no Bluetooth device connected".into();
        active.cube = Some(CubeState::solved());
        active.synced = true;
        active.counter = Some(0);
        active
    }
    pub fn record(&mut self, movement: MoveRecord) {
        self.observed_turns += 1;
        self.history.push_back(movement);
        if self.history.len() > 48 {
            self.history.pop_front();
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub registry: Registry,
    pub config_path: String,
    pub adapter: String,
    pub scanning: bool,
    pub nearby: Vec<NearbyDevice>,
    pub active: Option<ActiveCube>,
    pub error: Option<String>,
    pub notice: Option<String>,
}
impl Default for AppState {
    fn default() -> Self {
        Self {
            registry: Registry::default(),
            config_path: String::new(),
            adapter: "Bluetooth ready to scan".into(),
            scanning: false,
            nearby: vec![],
            active: None,
            error: None,
            notice: None,
        }
    }
}
#[derive(Debug, Clone)]
pub enum Command {
    Scan,
    StopScan,
    Save {
        device: NearbyDevice,
        mac_override: String,
    },
    Connect(String),
    Disconnect,
    Forget(String),
    Rename {
        id: String,
        name: String,
    },
    SetAutoConnect(bool),
    Resync,
    StartDemo,
    StopDemo,
    DemoMove(Move),
    ResetDemo,
    DismissError,
    Shutdown,
}
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
