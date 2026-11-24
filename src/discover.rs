// Finding connected fans through sysfs.
use std::{
    fs,
    path::{Path, PathBuf},
};

// USB path of a tty, e.g. "8-10.5.3". Stable across replugs, unlike the ttyACM number.
pub(crate) fn usb_path(tty: &Path) -> String {
    let dev = Path::new("/sys/class/tty").join(tty.file_name().unwrap_or_default()).join("device/..");
    fs::canonicalize(dev).ok().and_then(|p| Some(p.file_name()?.to_string_lossy().into_owned())).unwrap_or_default()
}

// Every tty whose USB device is 33c3:7792, ordered by USB path so fan order survives replugs.
pub(crate) fn all_fans() -> Vec<PathBuf> {
    let id = |e: &fs::DirEntry, f| {
        fs::read_to_string(e.path().join("device/..").join(f)).unwrap_or_default()
    };
    let mut v: Vec<_> = fs::read_dir("/sys/class/tty")
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| id(e, "idVendor").trim() == "33c3" && id(e, "idProduct").trim() == "7792")
        .map(|e| Path::new("/dev").join(e.file_name()))
        .collect();
    // numeric sort: "8-10.5.4.3" before "8-10.5.4.4.3", "9" before "10"
    v.sort_by_key(|p| {
        usb_path(p).split(|c: char| !c.is_ascii_digit()).filter_map(|s| s.parse().ok()).collect::<Vec<u32>>()
    });
    v
}
