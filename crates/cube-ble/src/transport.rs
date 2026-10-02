use crate::model::*;
use anyhow::{Context, Result, anyhow, bail};
use btleplug::{
    api::{
        Central, CentralState, Characteristic, Manager as _, Peripheral as _, PeripheralProperties,
        ScanFilter, WriteType,
    },
    platform::{Adapter, Manager, Peripheral},
};
use cube_core::{
    GanCipher, MacAddress,
    protocol::{self, Event, ReadRequest},
    sync::{ObservedMove, Tracker},
};
use futures_util::StreamExt;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, mpsc},
    time::{Instant, interval, sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(crate) enum Message {
    Adapter {
        scan: u64,
        name: String,
    },
    Nearby {
        scan: u64,
        device: NearbyDevice,
    },
    ScanDone {
        scan: u64,
        error: Option<String>,
    },
    Status {
        session: u64,
        status: ConnectionStatus,
        detail: String,
    },
    State {
        session: u64,
        tracker: Tracker,
    },
    Turn {
        session: u64,
        record: MoveRecord,
    },
    Metadata {
        session: u64,
        event: Event,
    },
}
pub(crate) type Events = mpsc::UnboundedSender<Message>;
pub(crate) type ScanLock = Arc<Mutex<()>>;

async fn adapter() -> Result<Adapter> {
    let manager = timeout(Duration::from_secs(5), Manager::new())
        .await
        .context("Bluetooth manager timed out")?
        .context("Cannot access the operating system Bluetooth service")?;
    let adapters = timeout(Duration::from_secs(5), manager.adapters()).await??;
    if adapters.is_empty() {
        bail!(
            "No Bluetooth adapter found. Enable Bluetooth, or attach a BLE adapter. A virtual machine needs USB Bluetooth passthrough."
        );
    }
    for adapter in adapters {
        if matches!(
            timeout(Duration::from_secs(3), adapter.adapter_state()).await,
            Ok(Ok(CentralState::PoweredOn))
        ) {
            return Ok(adapter);
        }
    }
    bail!("Bluetooth is off or unavailable. Turn it on in system settings, then try again.")
}

fn real_mac(props: &PeripheralProperties) -> Option<MacAddress> {
    props
        .manufacturer_data
        .iter()
        .find_map(|(&id, bytes)| MacAddress::from_manufacturer(id, bytes))
        .or_else(|| {
            // CoreBluetooth's address is not the hardware MAC. Never derive a key
            // from an Apple UUID or spoofed address; ask the user if ads lack it.
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            {
                props.address.to_string().parse().ok()
            }
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                None
            }
        })
}
async fn nearby(peripheral: &Peripheral) -> Option<NearbyDevice> {
    let props = timeout(Duration::from_secs(2), peripheral.properties())
        .await
        .ok()?
        .ok()??;
    let name = props
        .local_name
        .clone()
        .or(props.advertisement_name.clone())?;
    if !name.starts_with("GAN") {
        return None;
    }
    Some(NearbyDevice {
        id: peripheral.id().to_string(),
        name,
        mac: real_mac(&props),
        rssi: props.rssi,
    })
}

pub(crate) async fn scan(generation: u64, tx: Events, cancel: CancellationToken, lock: ScanLock) {
    let result = scan_inner(generation, &tx, &cancel, lock).await;
    let error = if cancel.is_cancelled() {
        None
    } else {
        result.err().map(|e| format!("{e:#}"))
    };
    let _ = tx.send(Message::ScanDone {
        scan: generation,
        error,
    });
}
async fn scan_inner(
    generation: u64,
    tx: &Events,
    cancel: &CancellationToken,
    lock: ScanLock,
) -> Result<()> {
    let _guard =
        tokio::select! { _ = cancel.cancelled() => return Ok(()), guard = lock.lock() => guard };
    let adapter = adapter().await?;
    if cancel.is_cancelled() {
        return Ok(());
    }
    let info = timeout(Duration::from_secs(2), adapter.adapter_info())
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_else(|| "Native Bluetooth LE".into());
    let _ = tx.send(Message::Adapter {
        scan: generation,
        name: info,
    });
    // iCarry E does NOT advertise its service UUID, so do not filter by service.
    timeout(
        Duration::from_secs(5),
        adapter.start_scan(ScanFilter::default()),
    )
    .await??;
    let result: Result<()> = async {
        let end = Instant::now() + Duration::from_secs(12);
        let mut known = HashMap::new();
        while Instant::now() < end && !cancel.is_cancelled() {
            let peripherals = timeout(Duration::from_secs(3), adapter.peripherals()).await??;
            for peripheral in peripherals {
                if cancel.is_cancelled() { break; }
                if let Some(device) = nearby(&peripheral).await
                    && known.get(&device.id) != Some(&device) {
                    known.insert(device.id.clone(), device.clone());
                    let _ = tx.send(Message::Nearby { scan:generation, device });
                }
            }
            tokio::select! { _ = cancel.cancelled() => break, _ = sleep(Duration::from_millis(400)) => {} }
        }
        Ok(())
    }.await;
    let _ = timeout(Duration::from_secs(3), adapter.stop_scan()).await;
    result
}

async fn find_cube(
    device: &SavedDevice,
    cancel: &CancellationToken,
    lock: ScanLock,
) -> Result<Peripheral> {
    let _guard = tokio::select! { _ = cancel.cancelled() => bail!("Cancelled"), guard = lock.lock() => guard };
    let adapter = adapter().await?;
    if cancel.is_cancelled() {
        bail!("Cancelled");
    }
    timeout(
        Duration::from_secs(5),
        adapter.start_scan(ScanFilter::default()),
    )
    .await??;
    let result = async {
        let deadline = Instant::now() + Duration::from_secs(18);
        while Instant::now() < deadline && !cancel.is_cancelled() {
            for peripheral in timeout(Duration::from_secs(3), adapter.peripherals()).await?? {
                if cancel.is_cancelled() { bail!("Cancelled"); }
                if peripheral.id().to_string() == device.peripheral_id { return Ok(peripheral); }
                if let Some(props) = timeout(Duration::from_secs(2), peripheral.properties()).await??
                    && real_mac(&props) == Some(device.mac) { return Ok(peripheral); }
            }
            tokio::select! { _ = cancel.cancelled() => bail!("Cancelled"), _ = sleep(Duration::from_millis(400)) => {} }
        }
        bail!("Cube not found. Wake it with four quick turns of the white face, keep it near the antenna, and disconnect CubeStation.")
    }.await;
    let _ = timeout(Duration::from_secs(3), adapter.stop_scan()).await;
    result
}

pub(crate) fn retry_delay(attempt: u32) -> Duration {
    Duration::from_secs([1, 2, 4, 8, 15, 30][attempt.min(5) as usize])
}
fn status(tx: &Events, session: u64, status: ConnectionStatus, detail: impl Into<String>) {
    let detail = detail.into();
    tracing::info!(?status, %detail, "Cube connection");
    let _ = tx.send(Message::Status {
        session,
        status,
        detail,
    });
}
#[derive(Debug)]
struct UnsupportedProtocol;
impl std::fmt::Display for UnsupportedProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("This cube does not expose the GAN Gen4 service. This version supports GAN Gen4, including the tested iCarry E; other generations need a driver.")
    }
}
impl std::error::Error for UnsupportedProtocol {}

pub(crate) async fn session(
    device: SavedDevice,
    generation: u64,
    tx: Events,
    cancel: CancellationToken,
    mut resync: mpsc::UnboundedReceiver<()>,
    lock: ScanLock,
) {
    let mut attempts = 0;
    while !cancel.is_cancelled() {
        status(
            &tx,
            generation,
            ConnectionStatus::Searching,
            "Searching for your saved cube. Keep it awake and disconnect other apps.",
        );
        let found = find_cube(&device, &cancel, lock.clone()).await;
        let result = match found {
            Ok(peripheral) => {
                status(
                    &tx,
                    generation,
                    ConnectionStatus::Connecting,
                    "Opening a native Bluetooth connection",
                );
                let started = Instant::now();
                let result = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => Ok(()),
                    result = connected(&peripheral, &device, generation, &tx, &mut resync) => result,
                };
                // Runs on cancellation, errors and remote disconnects. Never abort
                // a connection task without first giving this cleanup time to run.
                let _ = timeout(Duration::from_secs(4), peripheral.disconnect()).await;
                if started.elapsed() > Duration::from_secs(30) {
                    attempts = 0;
                }
                result
            }
            Err(e) => Err(e),
        };
        if cancel.is_cancelled() {
            break;
        }
        let error = result
            .err()
            .unwrap_or_else(|| anyhow!("The cube closed its connection"));
        if error.downcast_ref::<UnsupportedProtocol>().is_some() {
            status(
                &tx,
                generation,
                ConnectionStatus::Unsupported,
                error.to_string(),
            );
            break;
        }
        let delay = retry_delay(attempts);
        status(
            &tx,
            generation,
            ConnectionStatus::Retrying,
            format!("{error:#} Reconnecting in {}s…", delay.as_secs()),
        );
        attempts = attempts.saturating_add(1);
        tokio::select! { _ = cancel.cancelled() => break, _ = sleep(delay) => {} }
    }
}

async fn request(
    peripheral: &Peripheral,
    characteristic: &Characteristic,
    cipher: &GanCipher,
    req: ReadRequest,
) -> Result<()> {
    let bytes = cipher.request(req);
    let kind = if characteristic
        .properties
        .contains(btleplug::api::CharPropFlags::WRITE)
    {
        WriteType::WithResponse
    } else {
        WriteType::WithoutResponse
    };
    timeout(
        Duration::from_secs(4),
        peripheral.write(characteristic, &bytes, kind),
    )
    .await
    .context("Cube request timed out")?
    .context("Cannot send cube read request")
}
async fn connected(
    peripheral: &Peripheral,
    device: &SavedDevice,
    generation: u64,
    tx: &Events,
    resync: &mut mpsc::UnboundedReceiver<()>,
) -> Result<()> {
    peripheral
        .connect_with_timeout(Duration::from_secs(15))
        .await
        .context("Cannot connect; another app may be using the cube")?;
    peripheral
        .discover_services_with_timeout(Duration::from_secs(10))
        .await
        .context("Cannot discover cube services")?;
    let service_uuid = Uuid::parse_str(protocol::SERVICE)?;
    if !peripheral.services().iter().any(|s| s.uuid == service_uuid) {
        return Err(UnsupportedProtocol.into());
    }
    let chars = peripheral.characteristics();
    let command_uuid = Uuid::parse_str(protocol::COMMAND)?;
    let state_uuid = Uuid::parse_str(protocol::STATE)?;
    let command = chars
        .iter()
        .find(|c| c.uuid == command_uuid && c.service_uuid == service_uuid)
        .context("GAN command characteristic is missing")?;
    let state = chars
        .iter()
        .find(|c| c.uuid == state_uuid && c.service_uuid == service_uuid)
        .context("GAN state characteristic is missing")?;
    let mut notifications = timeout(Duration::from_secs(4), peripheral.notifications()).await??;
    timeout(Duration::from_secs(4), peripheral.subscribe(state)).await??;
    let cipher = GanCipher::new(device.mac);
    status(
        tx,
        generation,
        ConnectionStatus::Synchronizing,
        "Reading authoritative state; no calibration or reset is performed",
    );
    request(peripheral, command, &cipher, ReadRequest::State).await?;
    request(peripheral, command, &cipher, ReadRequest::Battery).await?;
    request(peripheral, command, &cipher, ReadRequest::Hardware).await?;
    let mut tracker = Tracker::default();
    let mut housekeeping = interval(Duration::from_millis(250));
    housekeeping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_packet = Instant::now();
    let mut last_state_request = Instant::now();
    let mut last_battery_request = Instant::now();
    let mut unsynced_since = Some(Instant::now());
    let mut request_state = false;
    let mut consecutive_decode_errors = 0u8;
    loop {
        tokio::select! {
            notification = notifications.next() => {
                let notification = notification.context("Bluetooth notifications ended")?;
                if notification.uuid != state_uuid { continue; }
                let decoded = cipher.decrypt(&notification.value).and_then(|plain| protocol::decode(&plain));
                let event = match decoded {
                    Ok(event) => { consecutive_decode_errors = 0; event }
                    Err(error) => {
                        consecutive_decode_errors += 1;
                        tracker.invalidate(); request_state = true;
                        unsynced_since.get_or_insert_with(Instant::now);
                        let _ = tx.send(Message::State { session:generation, tracker:tracker.clone() });
                        tracing::warn!(%error, "Rejected an invalid cube packet");
                        if consecutive_decode_errors >= 5 { bail!("Cannot decode cube data. Check the hardware MAC address and model."); }
                        continue;
                    }
                };
                if !matches!(event, Event::Unknown(_)) { last_packet = Instant::now(); }
                match event {
                    Event::Move { counter, cube_time_ms, movement } => {
                        let outcome = tracker.on_move(ObservedMove { counter, cube_time_ms, movement });
                        request_state |= outcome.request_state;
                        if outcome.accepted {
                            tracing::debug!(%movement, counter, "Turn");
                            let _ = tx.send(Message::Turn { session:generation, record:MoveRecord { movement, counter, cube_time_ms } });
                            let _ = tx.send(Message::State { session:generation, tracker:tracker.clone() });
                        }
                    }
                    Event::Snapshot { counter, state } => {
                        let outcome = tracker.on_snapshot(counter, state);
                        request_state |= outcome.request_state;
                        if outcome.accepted {
                            if tracker.synced { request_state = false; }
                            let _ = tx.send(Message::State { session:generation, tracker:tracker.clone() });
                        }
                    }
                    Event::Disconnect => bail!("The cube requested disconnection"),
                    Event::Unknown(kind) => tracing::trace!(kind, "Ignored an optional GAN event"),
                    metadata => { let _ = tx.send(Message::Metadata { session:generation, event:metadata }); }
                }
                if tracker.synced { unsynced_since = None; }
                else { unsynced_since.get_or_insert_with(Instant::now); }
            }
            Some(()) = resync.recv() => {
                tracker.invalidate(); request_state = true;
                unsynced_since.get_or_insert_with(Instant::now);
                let _ = tx.send(Message::State { session:generation, tracker:tracker.clone() });
            }
            _ = housekeeping.tick() => {
                if unsynced_since.is_some_and(|t| t.elapsed() > Duration::from_secs(10)) {
                    bail!("No valid cube state received. Check the hardware MAC and keep the cube close to the antenna.");
                }
                if last_packet.elapsed() > Duration::from_secs(15) {
                    bail!("Cube stopped responding to keep-alive requests");
                }
                // A state read every 5s both keeps the application link active and
                // heals undetected drift. Gaps trigger a prompt, coalesced read.
                if last_state_request.elapsed() >= Duration::from_secs(5)
                    || (request_state && last_state_request.elapsed() >= Duration::from_millis(250)) {
                    request(peripheral, command, &cipher, ReadRequest::State).await?;
                    last_state_request = Instant::now(); request_state = false;
                }
                if last_battery_request.elapsed() >= Duration::from_secs(60) {
                    request(peripheral, command, &cipher, ReadRequest::Battery).await?;
                    last_battery_request = Instant::now();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_back_off_and_remain_bounded() {
        assert_eq!(
            (0..8).map(|n| retry_delay(n).as_secs()).collect::<Vec<_>>(),
            vec![1, 2, 4, 8, 15, 30, 30, 30]
        );
    }
    #[tokio::test(start_paused = true)]
    async fn cancelling_retry_wait_is_immediate() {
        let cancel = CancellationToken::new();
        let copy = cancel.clone();
        let start = Instant::now();
        let task = tokio::spawn(async move {
            tokio::select! { _ = copy.cancelled() => true, _ = sleep(retry_delay(5)) => false }
        });
        cancel.cancel();
        assert!(task.await.unwrap());
        assert_eq!(start.elapsed(), Duration::ZERO);
    }
}
