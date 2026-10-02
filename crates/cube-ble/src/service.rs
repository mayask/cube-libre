use crate::{
    model::*,
    store,
    transport::{self, Message, ScanLock},
};
use anyhow::{Context, Result, bail};
use cube_core::protocol::Event;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, mpsc, watch},
    task::JoinHandle,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Backend {
    inner: Arc<Handle>,
}
struct Handle {
    commands: mpsc::UnboundedSender<Command>,
    state: watch::Receiver<AppState>,
}
impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
    }
}
impl Backend {
    pub fn spawn() -> Self {
        Self::start(store::default_path())
    }
    pub fn spawn_at(path: PathBuf) -> Self {
        Self::start(Ok(path))
    }
    fn start(path: Result<PathBuf>) -> Self {
        let (commands, rx) = mpsc::unbounded_channel();
        let (updates, state) = watch::channel(AppState::default());
        let failure_updates = updates.clone();
        let result = std::thread::Builder::new()
            .name("cube-bluetooth".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => runtime.block_on(Actor::new(path, updates).run(rx)),
                    Err(e) => {
                        failure_updates.send_modify(|s| {
                            s.error = Some(format!("Cannot start Bluetooth worker: {e}"))
                        });
                    }
                }
            });
        if let Err(e) = result {
            tracing::error!(%e, "Cannot start Bluetooth worker thread");
        }
        Self {
            inner: Arc::new(Handle { commands, state }),
        }
    }
    pub fn send(&self, command: Command) {
        let _ = self.inner.commands.send(command);
    }
    pub fn subscribe(&self) -> watch::Receiver<AppState> {
        self.inner.state.clone()
    }
    pub fn current(&self) -> AppState {
        self.inner.state.borrow().clone()
    }
}

struct Job {
    cancel: CancellationToken,
    handle: JoinHandle<()>,
}
impl Job {
    async fn stop(self) {
        self.cancel.cancel();
        let mut handle = self.handle;
        if timeout(Duration::from_secs(18), &mut handle).await.is_err() {
            tracing::warn!("Bluetooth job did not finish bounded cleanup; aborting");
            handle.abort();
        }
    }
}
struct Actor {
    state: AppState,
    path: Option<PathBuf>,
    storage_error: Option<String>,
    updates: watch::Sender<AppState>,
    events: mpsc::UnboundedSender<Message>,
    incoming: mpsc::UnboundedReceiver<Message>,
    session: Option<Job>,
    scan: Option<Job>,
    generation: u64,
    scan_generation: u64,
    resync: Option<mpsc::UnboundedSender<()>>,
    scan_lock: ScanLock,
}
impl Actor {
    fn new(path: Result<PathBuf>, updates: watch::Sender<AppState>) -> Self {
        let mut state = AppState::default();
        let mut storage_error = None;
        let path = match path {
            Ok(path) => {
                state.config_path = path.display().to_string();
                match store::load(&path) {
                    Ok(registry) => state.registry = registry,
                    Err(e) => {
                        storage_error = Some(format!(
                            "{e:#}. Device saving is disabled to protect the existing file. Fix {} and restart.",
                            path.display()
                        ))
                    }
                }
                Some(path)
            }
            Err(e) => {
                storage_error = Some(e.to_string());
                None
            }
        };
        state.error = storage_error.clone();
        let (events, incoming) = mpsc::unbounded_channel();
        Self {
            state,
            path,
            storage_error,
            updates,
            events,
            incoming,
            session: None,
            scan: None,
            generation: 0,
            scan_generation: 0,
            resync: None,
            scan_lock: Arc::new(Mutex::new(())),
        }
    }
    fn publish(&self) {
        self.updates.send_replace(self.state.clone());
    }
    fn persist(&mut self, registry: Registry) -> Result<()> {
        if let Some(error) = &self.storage_error {
            bail!("{error}");
        }
        store::save(
            self.path
                .as_ref()
                .context("No configuration path available")?,
            &registry,
        )?;
        self.state.registry = registry;
        Ok(())
    }
    async fn stop_scan(&mut self) {
        self.scan_generation += 1;
        self.state.scanning = false;
        if let Some(job) = self.scan.take() {
            job.stop().await;
        }
    }
    async fn stop_session(&mut self) {
        self.generation += 1;
        self.resync = None;
        if let Some(active) = &mut self.state.active {
            active.synced = false;
            active.status = ConnectionStatus::Disconnected;
            active.detail = "Connection paused. Display shows the last known state.".into();
        }
        self.publish();
        if let Some(job) = self.session.take() {
            job.stop().await;
        }
    }
    async fn connect(&mut self, id: String) -> Result<()> {
        let device = self
            .state
            .registry
            .devices
            .iter()
            .find(|d| d.id == id)
            .cloned()
            .context("Saved device not found")?;
        let mut registry = self.state.registry.clone();
        registry.selected = Some(id);
        self.persist(registry)?;
        self.stop_scan().await;
        self.stop_session().await;
        self.state.active = Some(ActiveCube::new(&device));
        self.state.error = None;
        self.state.notice = None;
        let cancel = CancellationToken::new();
        let (resync, requests) = mpsc::unbounded_channel();
        self.resync = Some(resync);
        let handle = tokio::spawn(transport::session(
            device,
            self.generation,
            self.events.clone(),
            cancel.clone(),
            requests,
            self.scan_lock.clone(),
        ));
        self.session = Some(Job { cancel, handle });
        Ok(())
    }
    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>) {
        self.publish();
        if self.state.registry.auto_connect
            && let Some(id) = self.state.registry.selected.clone()
            && let Err(e) = self.connect(id).await
        {
            self.state.error = Some(format!("{e:#}"));
            self.publish();
        }
        loop {
            tokio::select! {
                command = commands.recv() => {
                    match command {
                        None | Some(Command::Shutdown) => break,
                        Some(command) => {
                            if let Err(e) = self.command(command).await { self.state.error = Some(format!("{e:#}")); }
                        }
                    }
                }
                Some(message) = self.incoming.recv() => self.message(message),
            }
            self.publish();
        }
        self.stop_scan().await;
        self.stop_session().await;
        tracing::info!("Bluetooth worker stopped; connections released");
    }
    async fn command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::Scan => {
                self.stop_scan().await;
                self.state.scanning = true;
                self.state.nearby.clear();
                self.state.error = None;
                let cancel = CancellationToken::new();
                let handle = tokio::spawn(transport::scan(
                    self.scan_generation,
                    self.events.clone(),
                    cancel.clone(),
                    self.scan_lock.clone(),
                ));
                self.scan = Some(Job { cancel, handle });
            }
            Command::StopScan => self.stop_scan().await,
            Command::Save {
                device,
                mac_override,
            } => {
                let mac = if mac_override.trim().is_empty() {
                    device.mac.context("The cube did not advertise its hardware address. Enter it manually; it is required to decode GAN data.")?
                } else {
                    mac_override.parse()?
                };
                let mut registry = self.state.registry.clone();
                let id = mac.to_string();
                if let Some(saved) = registry.devices.iter_mut().find(|d| d.id == id) {
                    saved.peripheral_id = device.id;
                } else {
                    if !store::valid_name(&device.name) {
                        bail!("Invalid device name");
                    }
                    registry.devices.push(SavedDevice {
                        id: id.clone(),
                        name: device.name,
                        peripheral_id: device.id,
                        mac,
                    });
                }
                self.persist(registry)?;
                self.connect(id).await?;
            }
            Command::Connect(id) => self.connect(id).await?,
            Command::Disconnect => self.stop_session().await,
            Command::Forget(id) => {
                let mut registry = self.state.registry.clone();
                registry.devices.retain(|d| d.id != id);
                if registry.selected.as_ref() == Some(&id) {
                    registry.selected = None;
                }
                self.persist(registry)?;
                if self.state.active.as_ref().is_some_and(|a| a.id == id) {
                    self.stop_session().await;
                    self.state.active = None;
                }
                self.state.notice =
                    Some("Device forgotten locally. Nothing was reset on the cube.".into());
            }
            Command::Rename { id, name } => {
                let name = name.trim().to_owned();
                if !store::valid_name(&name) {
                    bail!("Use a name between 1 and 80 characters, without control characters");
                }
                let mut registry = self.state.registry.clone();
                registry
                    .devices
                    .iter_mut()
                    .find(|d| d.id == id)
                    .context("Device not found")?
                    .name = name.clone();
                self.persist(registry)?;
                if let Some(active) = &mut self.state.active
                    && active.id == id
                {
                    active.name = name;
                }
            }
            Command::SetAutoConnect(enabled) => {
                let mut registry = self.state.registry.clone();
                registry.auto_connect = enabled;
                self.persist(registry)?;
            }
            Command::Resync => {
                if let Some(sender) = &self.resync {
                    let _ = sender.send(());
                }
            }
            Command::StartDemo => {
                self.stop_scan().await;
                self.stop_session().await;
                self.state.active = Some(ActiveCube::demo());
                self.state.error = None;
            }
            Command::StopDemo => {
                if self
                    .state
                    .active
                    .as_ref()
                    .is_some_and(|a| a.status == ConnectionStatus::Demo)
                {
                    self.state.active = None;
                }
            }
            Command::DemoMove(movement) => {
                if let Some(active) = &mut self.state.active
                    && active.status == ConnectionStatus::Demo
                {
                    active.cube.as_mut().expect("demo cube").apply(movement);
                    let counter = active.counter.unwrap_or(0).wrapping_add(1);
                    active.counter = Some(counter);
                    active.record(MoveRecord {
                        movement,
                        counter,
                        cube_time_ms: 0,
                    });
                    active.last_update_ms = Some(now_ms());
                }
            }
            Command::ResetDemo => {
                if self
                    .state
                    .active
                    .as_ref()
                    .is_some_and(|a| a.status == ConnectionStatus::Demo)
                {
                    self.state.active = Some(ActiveCube::demo());
                }
            }
            Command::DismissError => {
                self.state.error = None;
                self.state.notice = None;
            }
            Command::Shutdown => {}
        }
        Ok(())
    }
    fn message(&mut self, message: Message) {
        match message {
            Message::Adapter { scan, name } if scan == self.scan_generation => {
                self.state.adapter = name
            }
            Message::Nearby { scan, device } if scan == self.scan_generation => {
                if let Some(old) = self.state.nearby.iter_mut().find(|d| d.id == device.id) {
                    *old = device;
                } else {
                    self.state.nearby.push(device);
                }
                self.state
                    .nearby
                    .sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
            }
            Message::ScanDone { scan, error } if scan == self.scan_generation => {
                self.state.scanning = false;
                if let Some(error) = error {
                    self.state.error = Some(error);
                } else if self.state.nearby.is_empty() {
                    self.state.notice = Some("No GAN cubes found. Wake the cube near the antenna and close CubeStation, then scan again.".into());
                }
            }
            Message::Status {
                session,
                status,
                detail,
            } if session == self.generation => {
                if let Some(active) = &mut self.state.active {
                    active.status = status;
                    active.detail = detail;
                    active.synced = false;
                    if status == ConnectionStatus::Connecting {
                        active.history.clear();
                        active.observed_turns = 0;
                        active.missed_turns = 0;
                        active.corrections = 0;
                    }
                }
            }
            Message::State { session, tracker } if session == self.generation => {
                if let Some(active) = &mut self.state.active {
                    if let Some(cube) = tracker.state {
                        active.cube = Some(cube);
                    }
                    active.synced = tracker.synced;
                    active.counter = tracker.counter;
                    active.missed_turns = tracker.missed_turns;
                    active.corrections = tracker.corrections;
                    active.status = if tracker.synced {
                        ConnectionStatus::Connected
                    } else {
                        ConnectionStatus::Synchronizing
                    };
                    active.detail = if tracker.synced {
                        "Live cube state · keep-alive every 5 seconds"
                    } else {
                        "Waiting for authoritative state; last known state shown"
                    }
                    .into();
                    if tracker.synced {
                        active.last_update_ms = Some(now_ms());
                    }
                }
            }
            Message::Turn { session, record } if session == self.generation => {
                if let Some(active) = &mut self.state.active {
                    active.record(record);
                }
            }
            Message::Metadata { session, event } if session == self.generation => {
                if let Some(active) = &mut self.state.active {
                    match event {
                        Event::Battery(level) => active.battery = Some(level),
                        Event::HardwareName(name) => active.hardware.name = Some(name),
                        Event::HardwareVersion(version) => {
                            active.hardware.hardware_version = Some(version)
                        }
                        Event::Firmware(version) => active.hardware.firmware = Some(version),
                        Event::ProductDate(date) => active.hardware.product_date = Some(date),
                        _ => {}
                    }
                }
            }
            _ => {} // A cancelled session/scan must never overwrite a newer one.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn simulator_uses_real_cube_math_without_touching_bluetooth_or_persistence() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("devices.json");
        let backend = Backend::spawn_at(path.clone());
        let mut rx = backend.subscribe();
        backend.send(Command::StartDemo);
        timeout(Duration::from_secs(3), async {
            while !rx
                .borrow()
                .active
                .as_ref()
                .is_some_and(|a| a.status == ConnectionStatus::Demo)
            {
                rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        backend.send(Command::DemoMove("R".parse().unwrap()));
        timeout(Duration::from_secs(3), async {
            while rx.borrow().active.as_ref().unwrap().observed_turns != 1 {
                rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        assert!(
            !rx.borrow()
                .active
                .as_ref()
                .unwrap()
                .cube
                .as_ref()
                .unwrap()
                .is_solved()
        );
        assert!(!path.exists());
        backend.send(Command::StopDemo);
        timeout(Duration::from_secs(3), async {
            while rx.borrow().active.is_some() {
                rx.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        backend.send(Command::Shutdown);
    }
    #[test]
    fn old_session_messages_are_ignored() {
        let (updates, _) = watch::channel(AppState::default());
        let dir = tempfile::tempdir().unwrap();
        let mut actor = Actor::new(Ok(dir.path().join("devices.json")), updates);
        actor.generation = 10;
        actor.state.active = Some(ActiveCube::demo());
        actor.message(Message::Status {
            session: 9,
            status: ConnectionStatus::Retrying,
            detail: "stale".into(),
        });
        assert_eq!(actor.state.active.unwrap().status, ConnectionStatus::Demo);
    }
}
