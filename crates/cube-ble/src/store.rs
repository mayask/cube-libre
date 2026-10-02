use crate::model::Registry;
use anyhow::{Context, Result, bail};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn default_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("CUBE_LIBRE_CONFIG") {
        return Ok(PathBuf::from(path));
    }
    directories::ProjectDirs::from("io.github", "CubeLibre", "CubeLibre")
        .map(|dirs| dirs.config_dir().join("devices.json"))
        .context("Cannot locate your application configuration directory")
}
pub fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control)
}
pub fn validate(registry: &Registry) -> Result<()> {
    if registry.version != 1 {
        bail!("Unsupported device registry version {}", registry.version);
    }
    if registry.devices.len() > 128 {
        bail!("Too many saved devices");
    }
    let mut ids = HashSet::new();
    for device in &registry.devices {
        let expected = device
            .mac
            .map(|mac| mac.to_string())
            .unwrap_or_else(|| format!("ble:{}", device.peripheral_id));
        if device.id != expected || !ids.insert(&device.id) {
            bail!("Invalid or duplicated saved device identity");
        }
        if !valid_name(&device.name)
            || device.peripheral_id.is_empty()
            || device.peripheral_id.len() > 512
            || device.peripheral_id.chars().any(char::is_control)
        {
            bail!("Invalid saved device details");
        }
    }
    if registry
        .selected
        .as_ref()
        .is_some_and(|id| !ids.contains(id))
    {
        bail!("The selected device is not present in the registry");
    }
    Ok(())
}
pub fn load(path: &Path) -> Result<Registry> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Registry::default()),
        Err(e) => return Err(e).with_context(|| format!("Cannot read {}", path.display())),
    };
    if bytes.len() > 256 * 1024 {
        bail!("Device registry is unexpectedly large");
    }
    let registry: Registry = serde_json::from_slice(&bytes)
        .context("Device registry is not valid JSON; it has not been overwritten")?;
    validate(&registry)?;
    Ok(registry)
}
pub fn save(path: &Path, registry: &Registry) -> Result<()> {
    validate(registry)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).context("Cannot create configuration directory")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .context("Cannot create a temporary configuration file")?;
    temp.write_all(&serde_json::to_vec_pretty(registry)?)?;
    temp.write_all(b"\n")?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .map_err(|e| e.error)
        .context("Cannot atomically save device registry")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SavedDevice;
    #[test]
    fn round_trip_and_atomic_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/devices.json");
        assert_eq!(load(&path).unwrap(), Registry::default());
        let mac: cube_core::MacAddress = "01:02:03:04:05:06".parse().unwrap();
        let mut r = Registry::default();
        r.devices.push(SavedDevice {
            id: format!("{mac}"),
            name: "My GAN".into(),
            peripheral_id: "opaque-apple-id".into(),
            mac: Some(mac),
        });
        r.selected = Some(format!("{mac}"));
        save(&path, &r).unwrap();
        assert_eq!(load(&path).unwrap(), r);
        r.devices[0].name = "Renamed cube".into();
        save(&path, &r).unwrap();
        assert_eq!(load(&path).unwrap(), r);
    }
    #[test]
    fn existing_mac_records_load_without_migration_or_identity_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("devices.json");
        let old = br#"{"version":1,"devices":[{"id":"01:02:03:04:05:06","name":"Old cube","peripheral_id":"native-id","mac":"01:02:03:04:05:06"}],"selected":"01:02:03:04:05:06","auto_connect":true}"#;
        fs::write(&path, old).unwrap();
        let registry = load(&path).unwrap();
        assert_eq!(
            registry.devices[0].mac,
            Some("01:02:03:04:05:06".parse().unwrap())
        );
        assert_eq!(fs::read(&path).unwrap(), old);
    }
    #[test]
    fn gen1_can_persist_a_native_identity_without_a_mac() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("devices.json");
        let mut registry = Registry::default();
        registry.devices.push(SavedDevice {
            id: "ble:native-id".into(),
            name: "Gen1 cube".into(),
            peripheral_id: "native-id".into(),
            mac: None,
        });
        registry.selected = Some("ble:native-id".into());
        save(&path, &registry).unwrap();
        assert_eq!(load(&path).unwrap(), registry);
        registry.devices[0].id = "wrong-device".into();
        assert!(save(&path, &registry).is_err());
    }
    #[test]
    fn corrupt_or_future_data_is_not_silently_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("devices.json");
        fs::write(&path, b"{broken").unwrap();
        assert!(load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{broken");
        let r = Registry {
            version: 9,
            ..Registry::default()
        };
        assert!(save(&path, &r).is_err());
        assert!(!valid_name("   "));
        assert!(!valid_name("Cube\nname"));
    }
}
