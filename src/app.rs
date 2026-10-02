use cube_ble::{
    ActiveCube, AppState, Backend, Command, ConnectionStatus, NearbyDevice, SavedDevice,
};
use cube_core::{CubeState, Face, MacAddress, protocol::Generation};
use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
enum Modal {
    Add,
    Rename(SavedDevice),
    Forget(SavedDevice),
}

#[component]
pub fn App() -> Element {
    let initial = use_context::<Backend>();
    let backend = use_signal(|| initial);
    use_context_provider(|| backend);
    let mut state = use_signal(|| backend.read().current());
    use_future(move || {
        let mut updates = backend.read().subscribe();
        async move {
            loop {
                let next = updates.borrow_and_update().clone();
                state.set(next);
                if updates.changed().await.is_err() {
                    break;
                }
            }
        }
    });
    use_drop(move || backend.read().send(Command::Shutdown));
    let mut modal = use_signal(|| None::<Modal>);
    let data = state.read().clone();
    let close = use_callback(move |()| {
        modal.set(None);
        backend.read().send(Command::StopScan);
    });
    rsx! {
        style { {include_str!("styles.css")} }
        div { class: "app-shell", "inert": if modal.read().is_some() { Some("") } else { None },
            aside { class: "sidebar",
                div { class: "brand", div { class: "brand-mark", aria_hidden: "true", dangerous_inner_html: include_str!("../assets/cube-libre.svg") } div { strong { "Cube Libre" } span { "YOUR LOCAL WORKSPACE" } } }
                div { class: "side-section-heading", span { "DEVICES" } span { class: "count", "{data.registry.devices.len()}" } }
                h2 { "Your cubes" }
                p { class: "muted side-description", "A place for every turn." }
                button { class: "button primary add-device", onclick: move |_| { modal.set(Some(Modal::Add)); backend.read().send(Command::Scan); }, Icon { name: "plus" } "Add a device" }
                div { class: "device-list",
                    if data.registry.devices.is_empty() {
                        div { class: "empty-devices", Icon { name: "bluetooth", size: 28 } strong { "No devices yet" } p { "Add your GAN cube to see its state, one turn at a time." } }
                    }
                    for device in data.registry.devices.clone() {
                        DeviceCard {
                            key: "{device.id}", device: device.clone(),
                            active: data.active.clone().filter(|a| a.id == device.id),
                            on_rename: move |d| modal.set(Some(Modal::Rename(d))),
                            on_forget: move |d| modal.set(Some(Modal::Forget(d))),
                        }
                    }
                }
                div { class: "sidebar-bottom",
                    label { class: "check-row", input { r#type: "checkbox", checked: data.registry.auto_connect, onchange: move |e| backend.read().send(Command::SetAutoConnect(e.checked())) } span { "Reconnect on launch" } }
                    p { class: "tiny muted", "The last selected cube reconnects automatically." }
                    div { class: "local-badge", Icon { name: "shield", size: 17 } span { "On your device. Only." } }
                    div { class: "version", "DIOXUS + RUST", span { "v0.1.0" } }
                }
            }
            main { class: "main-content",
                header { class: "topbar", div { span { class: "eyebrow", "WORKSPACE" } h1 { "Your cube, in sync." } } span { class: "offline-pill", Icon { name: "shield", size: 15 } "Local-first" } }
                if let Some(error) = &data.error {
                    div { class: "banner error", role: "alert", Icon { name: "info" } span { "{error}" } button { class: "icon-button", aria_label: "Dismiss error", onclick: move |_| backend.read().send(Command::DismissError), Icon { name: "close" } } }
                }
                if let Some(notice) = &data.notice {
                    div { class: "banner", role: "status", Icon { name: "info" } span { "{notice}" } button { class: "icon-button", aria_label: "Dismiss notice", onclick: move |_| backend.read().send(Command::DismissError), Icon { name: "close" } } }
                }
                div { class: "workspace-grid",
                    CubeWorkspace { active: data.active.clone(), on_add: move |_| { modal.set(Some(Modal::Add)); backend.read().send(Command::Scan); } }
                    aside { class: "inspector",
                        TurnStream { active: data.active.clone() }
                        ConnectionCard { active: data.active.clone(), adapter: data.adapter.clone() }
                    }
                }
                footer { class: "workspace-footer", span { "No accounts. No cloud. Just your cube." } span { "GAN Gen1–4 · desktop preview" } }
            }
        }
        if let Some(dialog) = modal.read().clone() {
            div { class: "modal-backdrop", onclick: move |_| close.call(()), onkeydown: move |e| { if e.key() == Key::Escape { modal.set(None); backend.read().send(Command::StopScan); } },
                section { class: "modal", role: "dialog", aria_modal: "true", tabindex: "-1", onmounted: move |event| async move { let _ = event.set_focus(true).await; }, aria_label: match &dialog { Modal::Add => "Add a smart cube", Modal::Rename(_) => "Rename device", Modal::Forget(_) => "Forget device" }, onclick: move |e| e.stop_propagation(),
                    match dialog.clone() {
                        Modal::Add => rsx! { AddDevice { state: data.clone(), on_close: close } },
                        Modal::Rename(device) => rsx! { RenameDevice { device, on_close: close } },
                        Modal::Forget(device) => rsx! { ForgetDevice { device, on_close: close } },
                    }
                }
            }
        }
    }
}

#[component]
fn DeviceCard(
    device: SavedDevice,
    active: Option<ActiveCube>,
    on_rename: EventHandler<SavedDevice>,
    on_forget: EventHandler<SavedDevice>,
) -> Element {
    let backend = use_context::<Signal<Backend>>();
    let connected = active
        .as_ref()
        .is_some_and(|a| a.status == ConnectionStatus::Connected);
    let busy = active.as_ref().is_some_and(|a| a.status.is_busy());
    let id = device.id.clone();
    let rename = device.clone();
    let forget = device.clone();
    rsx! {
        article { class: if active.is_some() { "device-card selected" } else { "device-card" },
            div { class: "device-card-top", div { class: "device-symbol", Icon { name: "cube", size: 23 } }
                div { class: "device-title", strong { "{device.name}" } span { class: "tiny muted", "GAN smart cube" } }
                span { class: if connected { "status-dot live" } else if busy { "status-dot pending" } else { "status-dot" }, title: if connected { "Connected" } else { "Not connected" } }
            }
            div { class: "device-address mono", if let Some(mac) = device.mac { "{mac}" } else { "Native device identity · no MAC set" } }
            div { class: "device-actions",
                if connected || busy {
                    button { class: "button subtle compact", onclick: move |_| backend.read().send(Command::Disconnect), if busy { "Cancel connection" } else { "Disconnect" } }
                } else {
                    button { class: "button subtle compact", onclick: move |_| backend.read().send(Command::Connect(id.clone())), "Connect" }
                }
                button { class: "icon-button", title: "Rename device", aria_label: "Rename {device.name}", onclick: move |_| on_rename.call(rename.clone()), Icon { name: "edit", size: 16 } }
                button { class: "icon-button danger-hover", title: "Forget device", aria_label: "Forget {device.name}", onclick: move |_| on_forget.call(forget.clone()), Icon { name: "trash", size: 16 } }
            }
        }
    }
}

#[component]
fn CubeWorkspace(active: Option<ActiveCube>, on_add: EventHandler<()>) -> Element {
    let backend = use_context::<Signal<Backend>>();
    let mut net = use_signal(|| false);
    let mut yaw = use_signal(|| -35i32);
    let mut pitch = use_signal(|| -25i32);
    let demo = active
        .as_ref()
        .is_some_and(|a| a.status == ConnectionStatus::Demo);
    let synced = active.as_ref().is_some_and(|a| a.synced);
    let cube = active.as_ref().and_then(|a| a.cube.clone());
    let title = active
        .as_ref()
        .map(|a| a.name.as_str())
        .unwrap_or("A new perspective.");
    let status = active.as_ref().map(|a| a.status);
    let solved = cube.as_ref().is_some_and(CubeState::is_solved) && synced;
    let label = if demo {
        "SIMULATED"
    } else if synced {
        "LIVE STATE"
    } else if cube.is_some() {
        "LAST KNOWN STATE"
    } else {
        "AWAITING CONNECTION"
    };
    rsx! {
        section { class: "cube-workspace card",
            div { class: "card-heading", div { class: "section-label", Icon { name: "cube", size: 17 } "CUBE VIEW" }
                div { class: "view-switch", role: "group", aria_label: "Cube display mode",
                    button { class: if !net() { "selected" } else { "" }, aria_pressed: !net(), onclick: move |_| net.set(false), "3D" }
                    button { class: if net() { "selected" } else { "" }, aria_pressed: net(), onclick: move |_| net.set(true), "Net" }
                }
            }
            div { class: "cube-title-row", h2 { "{title}" } if solved { span { class: "solved-pill", Icon { name: "check", size: 13 } "Solved" } } }
            div { class: if demo { "state-label demo" } else if synced { "state-label live-text" } else { "state-label" },
                span { class: if synced && !demo { "status-dot live" } else { "status-dot" } } "{label}"
            }
            div { class: if synced { "cube-stage" } else { "cube-stage stale" },
                if net() { CubeNet { cube: cube.clone() } }
                else { Cube3d { cube: cube.clone(), yaw: yaw(), pitch: pitch() } }
            }
            if !net() {
                div { class: "view-controls", role: "group", aria_label: "Rotate cube view",
                    button { class: "icon-button", aria_label: "Rotate view left", title: "Rotate view left", onclick: move |_| yaw -= 90, Icon { name: "left", size: 18 } }
                    button { class: "icon-button", aria_label: "Tilt view up", title: "Tilt view up", onclick: move |_| pitch.set((pitch()-20).max(-85)), Icon { name: "up", size: 18 } }
                    button { class: "button subtle compact", title: "White up, green front", onclick: move |_| { yaw.set(-35); pitch.set(-25); }, Icon { name: "reset", size: 14 } "Reset view" }
                    button { class: "icon-button", aria_label: "Tilt view down", title: "Tilt view down", onclick: move |_| pitch.set((pitch()+20).min(85)), Icon { name: "down", size: 18 } }
                    button { class: "icon-button", aria_label: "Rotate view right", title: "Rotate view right", onclick: move |_| yaw += 90, Icon { name: "right", size: 18 } }
                }
            }
            if let Some(a) = &active {
                div { class: "connection-summary", role: "status",
                    span { class: if a.status == ConnectionStatus::Connected { "live-text" } else { "muted" }, "{a.status.label()}" }
                    p { "{a.detail}" }
                }
                if a.status == ConnectionStatus::Connected || a.status == ConnectionStatus::Synchronizing {
                    button { class: "button subtle resync-button", onclick: move |_| backend.read().send(Command::Resync), Icon { name: "refresh", size: 15 } "Resync from cube" }
                }
            } else {
                div { class: "start-prompt", p { "Connect a cube to bring this view to life." }
                    div { class: "inline-actions", button { class: "button primary", onclick: move |_| on_add.call(()), Icon { name: "bluetooth", size: 16 } "Connect your cube" }
                        button { class: "button subtle", onclick: move |_| backend.read().send(Command::StartDemo), "Try the demo" }
                    }
                }
            }
            if demo {
                div { class: "demo-controls", div { class: "demo-heading", span { "TRY A TURN" } button { class: "text-button", onclick: move |_| backend.read().send(Command::ResetDemo), "Reset demo" } }
                    div { class: "demo-moves",
                        for text in ["U", "R", "F", "D", "L", "B", "U'", "R'", "F'", "D'", "L'", "B'"] {
                            button { class: "move-button mono", aria_label: "Demo turn {text}", onclick: move |_| backend.read().send(Command::DemoMove(text.parse().expect("known move"))), "{text}" }
                        }
                    }
                    button { class: "text-button", onclick: move |_| backend.read().send(Command::StopDemo), "Leave demo" }
                }
            }
            div { class: "reference-note", Icon { name: "compass", size: 19 } div { strong { "A fixed reference frame" } p { "White on top, green in front. Face turns use a fixed reference frame. View controls only move the camera." } } }
            div { class: "face-legend", for face in Face::ALL { span { i { style: "background:{face.css_color()}" } "{face}" } } }
            if status == Some(ConnectionStatus::Unsupported) { p { class: "tiny muted", "Nothing on this device has been reset or modified." } }
        }
    }
}

#[component]
fn Cube3d(cube: Option<CubeState>, yaw: i32, pitch: i32) -> Element {
    rsx! {
        div { class: "perspective", role: "img", aria_label: if cube.is_some() { "Cube state in a manually controlled 3D view" } else { "Cube preview: state not yet known" },
            div { class: "cube-shadow" }
            div { class: "cube-3d", style: "transform:rotateX({pitch}deg) rotateY({yaw}deg)",
                for face in Face::ALL {
                    div { class: "cube-face face-{face}", aria_hidden: "true",
                        FaceStickers { cube: cube.clone(), face }
                    }
                }
            }
        }
    }
}
#[component]
fn CubeNet(cube: Option<CubeState>) -> Element {
    rsx! { div { class: "cube-net", role: "img", aria_label: "Unfolded cube: all six faces",
        for face in Face::ALL {
            div { class: "net-face net-{face}", FaceStickers { cube: cube.clone(), face } }
        }
    } }
}
#[component]
fn FaceStickers(cube: Option<CubeState>, face: Face) -> Element {
    rsx! {
        for index in 0..9 {
            {
                let sticker = cube.as_ref().map(|c| c.stickers()[face as usize * 9 + index]);
                let color = sticker.map(Face::css_color).unwrap_or("#293746");
                let name = sticker.map(Face::color_name).unwrap_or("Unknown");
                rsx! { div { class: "sticker", style: "background:{color}", title: "{face} sticker {index + 1}: {name}",
                    if index == 4 { span { "{face}" } }
                } }
            }
        }
    }
}

#[component]
fn TurnStream(active: Option<ActiveCube>) -> Element {
    let history = active
        .as_ref()
        .map(|a| a.history.clone())
        .unwrap_or_default();
    let turns = active.as_ref().map(|a| a.observed_turns).unwrap_or(0);
    let latest = history
        .back()
        .map(|m| m.movement.to_string())
        .unwrap_or_else(|| "—".into());
    rsx! {
        section { class: "card turn-stream", div { class: "card-heading", div { class: "section-label", Icon { name: "activity", size: 17 } "TURN STREAM" } span { class: "tiny muted", "{turns} turns" } }
            div { class: "latest-turn mono", "{latest}" }
            p { class: "latest-caption", if history.is_empty() { "Your next turn starts here." } else { "MOST RECENT TURN" } }
            div { class: "turn-chips", aria_label: "Recent turns",
                for (i, movement) in history.iter().enumerate().skip(history.len().saturating_sub(18)) {
                    span { class: if i+1 == history.len() { "turn-chip newest mono" } else { "turn-chip mono" }, title: "Counter {movement.counter}", "{movement.movement}" }
                }
                if history.is_empty() { span { class: "tiny muted empty-stream", "Moves appear here as you turn the physical cube." } }
            }
            div { class: "stream-footer", span { "This connection" } span { class: "mono", if let Some(a) = active { if let Some(n) = a.counter { "SEQ {n:03}" } else { "SEQ —" } } else { "SEQ —" } } }
        }
    }
}

#[component]
fn ConnectionCard(active: Option<ActiveCube>, adapter: String) -> Element {
    let status = active
        .as_ref()
        .map(|a| a.status.label())
        .unwrap_or("No active device");
    let battery = active.as_ref().and_then(|a| a.battery);
    let battery_text = battery
        .map(|n| format!("{n}%"))
        .unwrap_or_else(|| "—".into());
    let firmware = active
        .as_ref()
        .and_then(|a| a.hardware.firmware.clone())
        .unwrap_or_else(|| "—".into());
    let model = active
        .as_ref()
        .and_then(|a| a.hardware.name.clone())
        .unwrap_or_else(|| "—".into());
    let demo = active
        .as_ref()
        .is_some_and(|a| a.status == ConnectionStatus::Demo);
    let protocol = if demo {
        "Simulator"
    } else if let Some(protocol) = active.as_ref().and_then(|a| a.protocol) {
        protocol.label()
    } else {
        "Detect on connect"
    };
    rsx! {
        section { class: "card connection-card", div { class: "section-label", Icon { name: "bluetooth", size: 17 } "CONNECTION" }
            div { class: "connection-status", span { class: if active.as_ref().is_some_and(|a| a.status == ConnectionStatus::Connected) { "status-dot live" } else { "status-dot" } } strong { "{status}" } }
            div { class: "battery-row", span { class: "muted", "Battery" } strong { "{battery_text}" } }
            div { class: "battery-track", aria_hidden: "true", div { style: "width:{battery.unwrap_or(0)}%" } }
            dl { class: "metadata", dt { "Model" } dd { "{model}" } dt { "Protocol" } dd { "{protocol}" } dt { "Firmware" } dd { "{firmware}" } dt { "Keep-alive" } dd { if active.as_ref().is_some_and(|a| matches!(a.status, ConnectionStatus::Connected | ConnectionStatus::Synchronizing)) { if active.as_ref().is_some_and(|a| a.protocol == Some(Generation::Gen1)) { "Polling" } else { "Every 5s" } } else { "Paused" } } }
            details { class: "diagnostics", summary { "Connection details" }
                p { class: "tiny muted", "{adapter}" }
                if let Some(a) = &active {
                    dl { class: "metadata", dt { "Missed events" } dd { "{a.missed_turns}" } dt { "State repairs" } dd { "{a.corrections}" } }
                    p { class: "tiny muted", "A gap triggers a fresh cube snapshot. Missing turns are not invented in the move log." }
                }
            }
        }
    }
}

#[component]
fn AddDevice(state: AppState, on_close: EventHandler<()>) -> Element {
    let backend = use_context::<Signal<Backend>>();
    rsx! {
        div { class: "modal-heading", div { span { class: "eyebrow", "DEVICE MANAGEMENT" } h2 { "Add a smart cube" } } button { class: "icon-button", aria_label: "Close device manager", onclick: move |_| on_close.call(()), Icon { name: "close" } } }
        p { class: "muted", "GAN Gen1–Gen4, including iCarry, iCarry E, GAN i-series and compatible MG/AiCube models. No system pairing needed." }
        div { class: "setup-note", Icon { name: "info", size: 20 } p { "Wake the cube with four quick turns of its white face. Keep it near the antenna and disconnect CubeStation or other cube apps." } }
        div { class: "scan-heading", span { class: "section-label", if state.scanning { span { class: "spinner" } "SCANNING NEARBY" } else { "NEARBY CUBES" } }
            if state.scanning { button { class: "button subtle compact", onclick: move |_| backend.read().send(Command::StopScan), "Stop scan" } }
            else { button { class: "button subtle compact", onclick: move |_| backend.read().send(Command::Scan), Icon { name: "refresh", size: 14 } "Scan again" } }
        }
        if let Some(error) = &state.error { div { class: "banner error", role: "alert", "{error}" } }
        div { class: "nearby-list",
            for device in state.nearby.clone() {
                NearbyCard { key: "{device.id}", saved: state.registry.devices.iter().any(|d| d.peripheral_id == device.id || (device.mac.is_some() && d.mac == device.mac)), device, on_added: on_close }
            }
            if state.nearby.is_empty() {
                div { class: "scan-empty", Icon { name: "bluetooth", size: 35 } strong { if state.scanning { "Looking for your cube…" } else { "No cubes found" } } p { "GAN, MG and AiCube devices are listed; services are checked on connection. A VM needs Bluetooth USB passthrough." } }
            }
        }
        div { class: "modal-footer", Icon { name: "shield", size: 16 } span { "Saved locally. You can rename or forget devices anytime." } }
    }
}
#[component]
fn NearbyCard(device: NearbyDevice, saved: bool, on_added: EventHandler<()>) -> Element {
    let backend = use_context::<Signal<Backend>>();
    let mut address = use_signal(|| device.mac.map(|m| m.to_string()).unwrap_or_default());
    let mut error = use_signal(String::new);
    let candidate = device.clone();
    rsx! {
        article { class: "nearby-card", div { class: "nearby-heading", div { class: "device-symbol", Icon { name: "cube", size: 24 } } div { strong { "{device.name}" } p { class: "tiny muted mono", if let Some(mac) = device.mac { "{mac}" } else { "MAC not advertised · optional for Gen1" } } }
                span { class: "rssi mono", if let Some(rssi) = device.rssi { "{rssi} dBm" } }
            }
            details { open: device.mac.is_none(), summary { "Hardware address" }
                label { class: "field-label", "Cube MAC address" input { class: "text-input mono", aria_label: "Cube hardware MAC address", value: "{address}", placeholder: "AA:BB:CC:DD:EE:FF", oninput: move |e| address.set(e.value()) } }
                p { class: "tiny muted", "Required for Gen2–Gen4; auto-detected when available. Apple devices may need manual entry. Gen1 reads its key from device information and can leave this blank." }
            }
            if !error.read().is_empty() { p { class: "field-error", role: "alert", "{error}" } }
            button { class: "button primary", onclick: move |_| {
                let mac = address.read().trim().to_owned();
                if !mac.is_empty() && let Err(e) = mac.parse::<MacAddress>() { error.set(e.to_string()); return; }
                backend.read().send(Command::Save { device:candidate.clone(), mac_override:mac });
                on_added.call(());
            }, Icon { name: "bluetooth", size: 16 } if saved { "Connect saved cube" } else { "Add & connect" } }
        }
    }
}
#[component]
fn RenameDevice(device: SavedDevice, on_close: EventHandler<()>) -> Element {
    let backend = use_context::<Signal<Backend>>();
    let mut name = use_signal(|| device.name.clone());
    let mut error = use_signal(String::new);
    rsx! {
        div { class: "modal-heading", h2 { "Make it yours" } button { class: "icon-button", aria_label: "Close rename dialog", onclick: move |_| on_close.call(()), Icon { name: "close" } } }
        p { class: "muted", "This name is only stored in your local device list." }
        form { onsubmit: move |e| { e.prevent_default(); let value=name.read().trim().to_owned(); if !cube_ble::store::valid_name(&value) { error.set("Enter a name between 1 and 80 characters.".into()); return; } backend.read().send(Command::Rename { id:device.id.clone(), name:value }); on_close.call(()); },
            label { class: "field-label", "Device name" input { class: "text-input", value: "{name}", maxlength: "80", autofocus: true, oninput: move |e| name.set(e.value()) } }
            if !error.read().is_empty() { p { class: "field-error", "{error}" } }
            div { class: "dialog-actions", button { r#type: "button", class: "button subtle", onclick: move |_| on_close.call(()), "Cancel" } button { r#type: "submit", class: "button primary", "Save name" } }
        }
    }
}
#[component]
fn ForgetDevice(device: SavedDevice, on_close: EventHandler<()>) -> Element {
    let backend = use_context::<Signal<Backend>>();
    rsx! {
        div { class: "modal-heading", h2 { "Forget this cube?" } button { class: "icon-button", aria_label: "Close forget dialog", onclick: move |_| on_close.call(()), Icon { name: "close" } } }
        p { class: "muted", "Remove {device.name} from this app and stop its connection. Nothing on the physical cube is reset. You can add it again later." }
        div { class: "dialog-actions", button { class: "button subtle", onclick: move |_| on_close.call(()), "Keep device" } button { class: "button danger", onclick: move |_| { backend.read().send(Command::Forget(device.id.clone())); on_close.call(()); }, "Forget device" } }
    }
}

#[component]
fn Icon(name: &'static str, #[props(default = 20)] size: u32) -> Element {
    let path = match name {
        "cube" => "M12 2 3 7v10l9 5 9-5V7L12 2ZM3 7l9 5 9-5M12 12v10M7.5 4.5l9 5",
        "bluetooth" => "m7 7 10 10-5 5V2l5 5L7 17",
        "plus" => "M12 5v14M5 12h14",
        "shield" => "M12 2 4 5v6c0 5 8 10 8 10s8-5 8-10V5l-8-3Zm-4 9 3 3 5-5",
        "info" => "M12 8h.01M12 11v6M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0",
        "close" => "m6 6 12 12M6 18 18 6",
        "refresh" | "reset" => "M3 10a9 9 0 1 1 2 9M3 3v7h7",
        "activity" => "M2 12h4l3-8 6 16 3-8h4",
        "compass" => "M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0ZM16 8l-3 5-5 3 3-5 5-3Z",
        "left" => "m15 6-6 6 6 6",
        "right" => "m9 6 6 6-6 6",
        "up" => "m6 15 6-6 6 6",
        "down" => "m6 9 6 6 6-6",
        "check" => "m5 12 4 4L19 6",
        "edit" => "m15 5 4 4M4 20l4-1L20 7a2 2 0 0 0-4-4L4 15v5Z",
        "trash" => "M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7",
        _ => "",
    };
    rsx! { svg { width: "{size}", height: "{size}", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "1.7", stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true", path { d: path } } }
}
