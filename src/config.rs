/*
Saved choices in ~/.config/jlfan/config.toml. A fan uses its own section's value, else [all]'s:
  [all]
  gif = "/usr/share/jlfan/test.gif"
  rotate = 0            # extra quarter turns clockwise, 0..3
  [fan2]
  gif = "/home/me/b.gif"
*/
use crate::R;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub(crate) struct Fan {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) gif: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rotate: Option<u32>,
}
pub(crate) type Config = BTreeMap<String, Fan>; // "all" | "fanN" -> settings
const DEFAULT_GIF: &str = "/usr/share/jlfan/test.gif";

fn config_path() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(&env::var_os("HOME").unwrap_or_default()).join(".config"))
        .join("jlfan/config.toml")
}

pub(crate) fn load() -> R<Config> {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).map_err(|e| format!("{}: {e}", path.display()).into()),
        Err(_) => Ok(Config::new()), // nothing saved yet
    }
}

// Sets one field on `key`. Setting it on "all" clears it from the fan sections so it applies everywhere.
pub(crate) fn put<T>(cfg: &mut Config, key: &str, val: T, field: fn(&mut Fan) -> &mut Option<T>) {
    if key == "all" {
        cfg.values_mut().for_each(|f| *field(f) = None);
    }
    *field(cfg.entry(key.into()).or_default()) = Some(val);
    cfg.retain(|_, f| *f != Fan::default());
}

// Value for fan index i: its own section, else [all].
fn get<T: Clone>(cfg: &Config, i: usize, field: fn(&Fan) -> &Option<T>) -> Option<T> {
    let own = cfg.get(&format!("fan{}", i + 1)).and_then(|f| field(f).clone());
    own.or_else(|| cfg.get("all").and_then(|f| field(f).clone()))
}

pub(crate) fn gif_for(cfg: &Config, i: usize) -> Option<String> {
    if !cfg.values().any(|f| f.gif.is_some()) {
        return Some(DEFAULT_GIF.into()); // no GIF saved anywhere yet
    }
    get(cfg, i, |f| &f.gif)
}

pub(crate) fn rotate_for(cfg: &Config, i: usize) -> u32 {
    get(cfg, i, |f| &f.rotate).unwrap_or(0)
}

// Writes the config, then restarts the service if it runs so the change shows at once.
pub(crate) fn save(cfg: &Config) -> R<()> {
    let path = config_path();
    fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    fs::write(&path, toml::to_string(cfg)?)?;
    let _ = Command::new("systemctl").args(["--user", "try-restart", "jlfan"]).status();
    let active = Command::new("systemctl").args(["--user", "is-active", "--quiet", "jlfan"])
        .status().is_ok_and(|s| s.success());
    if active {
        println!("saved and applied (service restarted)");
    } else {
        println!("saved to {}; start the service: systemctl --user enable --now jlfan", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_choices() {
        let mut cfg: Config = toml::from_str("[all]\ngif = \"/a\"\nrotate = 1\n[fan2]\ngif = \"/my gifs/b\"\n").unwrap();
        assert_eq!((gif_for(&cfg, 0).unwrap(), gif_for(&cfg, 1).unwrap()), ("/a".into(), "/my gifs/b".into()));
        assert_eq!((rotate_for(&cfg, 0), rotate_for(&cfg, 1)), (1, 1)); // fan2 inherits [all] rotate
        put(&mut cfg, "fan2", 3, |f| &mut f.rotate);
        assert_eq!(rotate_for(&cfg, 1), 3);
        put(&mut cfg, "all", "/e".into(), |f| &mut f.gif); // all clears fan2's gif, keeps its rotate
        assert_eq!((gif_for(&cfg, 1).unwrap(), rotate_for(&cfg, 1)), ("/e".into(), 3));
        put(&mut cfg, "all", 0, |f| &mut f.rotate); // fan2 section is now empty and dropped
        assert_eq!(cfg.keys().collect::<Vec<_>>(), ["all"]);
        let back: Config = toml::from_str(&toml::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(back, cfg); // round-trips
        assert_eq!(gif_for(&Config::new(), 0).unwrap(), DEFAULT_GIF); // nothing saved
        let only_fan2: Config = toml::from_str("[fan2]\ngif = \"/b\"").unwrap();
        assert_eq!(gif_for(&only_fan2, 0), None); // fan1 left alone
    }
}
