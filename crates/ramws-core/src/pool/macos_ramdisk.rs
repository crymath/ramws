use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use plist::Value;

use crate::config::ManagedRamdiskPoolConfig;
use crate::pool::model::ManagedPoolState;
use crate::util::{
    cmd::{run_checked_owned, run_plist, try_run_owned},
    time::unix_timestamp_secs,
};

pub fn create_pool(
    pool_name: &str,
    cfg: &ManagedRamdiskPoolConfig,
    state_path: &Path,
) -> Result<ManagedPoolState> {
    let size_bytes = cfg
        .size_gib
        .checked_mul(1024_u64.pow(3))
        .ok_or_else(|| anyhow!("managed pool size_gib is too large"))?;
    let sectors = size_bytes.div_ceil(512);

    let mut attach_args = vec![
        "attach".to_string(),
        "-nomount".to_string(),
        "-plist".to_string(),
        format!("ram://{sectors}"),
    ];
    attach_args.extend(cfg.extra_hdiutil_args.clone());

    let attach_plist = run_plist("hdiutil", &attach_args)?;
    let device = parse_attach_device(&attach_plist)?;

    validate_device_name(&device)?;

    let mut erase_args = vec![
        "eraseVolume".to_string(),
        cfg.filesystem.clone(),
        cfg.volume_name.clone(),
        device.clone(),
    ];
    erase_args.extend(cfg.extra_diskutil_args.clone());
    run_checked_owned("diskutil", &erase_args).context("failed to erase managed RAM disk")?;

    let volume_path = format!("/Volumes/{}", cfg.volume_name);
    let mut volume_info = diskutil_info(&volume_path)?;
    let mut mountpoint = PathBuf::from(require_string(&volume_info, "MountPoint")?);
    if mountpoint.as_os_str().is_empty() {
        bail!("diskutil reported empty mountpoint for {}", volume_path);
    }

    let mounted_device = require_string(&volume_info, "DeviceIdentifier")?.to_string();

    let dev_info = diskutil_info(&device)?;
    validate_virtual_device(&dev_info, &device)?;

    if cfg.nobrowse {
        let unmount_args = vec!["unmount".to_string(), mountpoint.display().to_string()];
        run_checked_owned("diskutil", &unmount_args).with_context(|| {
            format!(
                "failed to unmount managed volume before remount nobrowse: {}",
                mountpoint.display()
            )
        })?;

        let mount_args = vec!["mount".to_string(), "nobrowse".to_string(), mounted_device.clone()];
        run_checked_owned("diskutil", &mount_args).context("failed to mount with nobrowse")?;

        volume_info = diskutil_info(&format!("/Volumes/{}", cfg.volume_name))?;
        mountpoint = PathBuf::from(require_string(&volume_info, "MountPoint")?);
    }

    let state = ManagedPoolState {
        pool_name: pool_name.to_string(),
        device,
        volume_name: cfg.volume_name.clone(),
        mountpoint,
        mounted_device,
        created_at: unix_timestamp_secs(),
    };
    persist_managed_state(state_path, &state)?;
    Ok(state)
}

pub fn destroy_pool(state: &ManagedPoolState, force: bool) -> Result<()> {
    let unmount_args = vec!["unmount".to_string(), state.mountpoint.display().to_string()];
    let unmount_output = try_run_owned("diskutil", &unmount_args)?;

    if !unmount_output.status.success() {
        let unmount_disk_args = vec!["unmountDisk".to_string(), state.device.clone()];
        let fallback = try_run_owned("diskutil", &unmount_disk_args)?;
        if !fallback.status.success() {
            let stderr = String::from_utf8_lossy(&fallback.stderr);
            bail!(
                "failed to unmount managed pool {} ({}) using both unmount and unmountDisk: {}",
                state.pool_name,
                state.device,
                stderr
            );
        }
    }

    let mut detach_args = vec!["detach".to_string(), state.device.clone()];
    let detach = try_run_owned("hdiutil", &detach_args)?;
    if !detach.status.success() {
        if !force {
            let stderr = String::from_utf8_lossy(&detach.stderr);
            bail!("failed to detach {}: {} (retry with --force)", state.device, stderr);
        }
        detach_args.push("-force".to_string());
        let forced = try_run_owned("hdiutil", &detach_args)?;
        if !forced.status.success() {
            let stderr = String::from_utf8_lossy(&forced.stderr);
            bail!("failed to force-detach {}: {}", state.device, stderr);
        }
    }

    Ok(())
}

pub fn parse_attach_device(value: &Value) -> Result<String> {
    let dict =
        value.as_dictionary().ok_or_else(|| anyhow!("hdiutil plist root is not a dictionary"))?;
    let entities = dict
        .get("system-entities")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("hdiutil plist missing system-entities array"))?;

    for entity in entities {
        let Some(entity_dict) = entity.as_dictionary() else {
            continue;
        };
        let Some(dev_entry) = entity_dict.get("dev-entry").and_then(Value::as_string) else {
            continue;
        };
        if is_disk_device_path(dev_entry) {
            return Ok(dev_entry.to_string());
        }
    }

    bail!("no /dev/diskN entry found in hdiutil plist")
}

pub fn persist_managed_state(path: &Path, state: &ManagedPoolState) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create pool state dir {}", parent.display()))?;
    }

    let text = toml::to_string_pretty(state).context("failed to serialize managed pool state")?;
    std::fs::write(path, text)
        .with_context(|| format!("failed to write pool state {}", path.display()))
}

pub fn load_managed_state(path: &Path) -> Result<ManagedPoolState> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read pool state {}", path.display()))?;
    toml::from_str::<ManagedPoolState>(&text)
        .with_context(|| format!("failed to parse pool state {}", path.display()))
}

fn diskutil_info(target: &str) -> Result<Value> {
    let args = vec!["info".to_string(), "-plist".to_string(), target.to_string()];
    run_plist("diskutil", &args)
}

fn require_string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .as_dictionary()
        .and_then(|dict| dict.get(key))
        .and_then(Value::as_string)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| anyhow!("diskutil plist missing non-empty {}", key))
}

fn validate_virtual_device(value: &Value, device: &str) -> Result<()> {
    let dict = value
        .as_dictionary()
        .ok_or_else(|| anyhow!("diskutil info plist root is not a dictionary"))?;

    let virtual_or_physical = dict
        .get("VirtualOrPhysical")
        .and_then(Value::as_string)
        .ok_or_else(|| anyhow!("diskutil info missing VirtualOrPhysical"))?;
    if virtual_or_physical != "Virtual" {
        bail!(
            "refusing to manage non-virtual device {} (VirtualOrPhysical={})",
            device,
            virtual_or_physical
        );
    }

    let bus_protocol = dict
        .get("BusProtocol")
        .and_then(Value::as_string)
        .ok_or_else(|| anyhow!("diskutil info missing BusProtocol"))?;
    if bus_protocol != "Disk Image" {
        bail!("refusing to manage device {} with unexpected BusProtocol={}", device, bus_protocol);
    }

    Ok(())
}

fn validate_device_name(device: &str) -> Result<()> {
    if !is_disk_device_path(device) {
        bail!("invalid managed device returned by hdiutil: {}", device);
    }
    Ok(())
}

fn is_disk_device_path(value: &str) -> bool {
    if !value.starts_with("/dev/disk") {
        return false;
    }
    value[9..].chars().all(|ch| ch.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use plist::Value;
    use tempfile::TempDir;

    use crate::config::ManagedRamdiskPoolConfig;

    use super::{create_pool, destroy_pool, parse_attach_device};

    #[test]
    fn parse_attach_device_happy_path() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>system-entities</key>
<array>
  <dict><key>dev-entry</key><string>/dev/disk42</string></dict>
</array>
</dict></plist>"#;

        let value = Value::from_reader_xml(xml.as_bytes()).expect("parse plist");
        let device = parse_attach_device(&value).expect("device");
        assert_eq!(device, "/dev/disk42");
    }

    #[test]
    fn parse_attach_device_rejects_non_disk() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>system-entities</key>
<array>
  <dict><key>dev-entry</key><string>/dev/rdisk42s1</string></dict>
</array>
</dict></plist>"#;

        let value = Value::from_reader_xml(xml.as_bytes()).expect("parse plist");
        let err = parse_attach_device(&value).expect_err("expected parse failure");
        assert!(err.to_string().contains("no /dev/diskN"));
    }

    #[test]
    #[ignore]
    fn managed_pool_e2e_when_explicitly_enabled() {
        if std::env::var("RAMWS_E2E").ok().as_deref() != Some("1") {
            return;
        }

        let tmp = TempDir::new().expect("tempdir");
        let state_path = tmp.path().join("pool.toml");
        let volume_name = format!("RAMWS_E2E_{}", std::process::id());
        let cfg = ManagedRamdiskPoolConfig {
            volume_name: volume_name.clone(),
            filesystem: "APFS".to_string(),
            size_gib: 1,
            nobrowse: true,
            extra_hdiutil_args: Vec::new(),
            extra_diskutil_args: Vec::new(),
        };

        let state = create_pool("main", &cfg, &state_path).expect("create pool");
        assert_eq!(state.pool_name, "main");
        assert!(state.mountpoint.starts_with(PathBuf::from("/Volumes")));

        destroy_pool(&state, true).expect("destroy pool");
    }
}
