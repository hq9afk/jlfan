// jlfan: stream an image/GIF to Jungle Leopard fan displays. Protocol: readme.md, notes: local/system-design.md.
mod config;
mod discover;
mod protocol;
mod serial;
mod video;

use config::{gif_for, load, put, rotate_for, save};
use discover::{all_fans, usb_path};
use std::{env, error::Error, fs, thread};

pub(crate) type R<T> = Result<T, Box<dyn Error>>;

// "fan2" -> index 1, for n detected fans.
fn fan_index(s: &str, n: usize) -> R<usize> {
    s.strip_prefix("fan")
        .and_then(|d| d.parse::<usize>().ok())
        .filter(|k| (1..=n).contains(k))
        .map(|k| k - 1)
        .ok_or_else(|| format!("unknown fan {s:?}, see `jlfan info`").into())
}

/*
Parses `set`/`rotate` args into config entries.
  VALUE                   -> ("all", VALUE)
  fanN VALUE [fanM VALUE] -> ("fanN", VALUE), ...
*/
fn parse_set<'a>(n: usize, args: &[&'a str]) -> R<Vec<(String, &'a str)>> {
    if let [f] = args {
        if fan_index(f, n).is_err() {
            return Ok(vec![("all".into(), *f)]);
        }
    }
    if args.is_empty() || args.len() % 2 != 0 {
        return Err("expected VALUE, or pairs of fanN VALUE".into());
    }
    let mut out: Vec<(String, &str)> = Vec::new();
    for pair in args.chunks(2) {
        let key = format!("fan{}", fan_index(pair[0], n)? + 1);
        if out.iter().any(|(k, _)| *k == key) {
            return Err(format!("{} given twice", pair[0]).into());
        }
        out.push((key, pair[1]));
    }
    Ok(out)
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run() -> R<()> {
    let a: Vec<String> = env::args().skip(1).collect();
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let usage = "usage:\n  jlfan info                 list detected fans and their saved GIF\n  \
                 jlfan info fanN            show one fan's device info\n  \
                 jlfan set [fanN] FILE ...  save a GIF/image (no fanN = all fans) and apply it\n  \
                 jlfan rotate [fanN] N ...  rotate by N x 90° clockwise, N = 0..3 (no fanN = all fans)\n  \
                 jlfan play                 stream the saved GIFs (what the service runs)";
    let fans = all_fans();
    let name = |i: usize| format!("fan{} ({}, {})", i + 1, usb_path(&fans[i]), fans[i].display());
    let mut cfg = load()?;

    match a.as_slice() {
        ["info"] => {
            for i in 0..fans.len() {
                let f = gif_for(&cfg, i).unwrap_or("-".into());
                println!("{}: {f}, rotate {}", name(i), rotate_for(&cfg, i));
            }
            if fans.is_empty() { eprintln!("no 33c3:7792 fans found"); }
        }
        ["info", f] => {
            let i = fan_index(f, fans.len())?;
            let j = serial::open(&fans[i]).and_then(|mut p| serial::cmd(&mut p, 0x06, &[]))
                .map_err(|e| format!("{}: {e} (is the jlfan service running?)", name(i)))?;
            println!("{}", String::from_utf8_lossy(&j));
        }
        ["set", rest @ ..] if !rest.is_empty() => {
            for (k, f) in parse_set(fans.len(), rest)? {
                let f = fs::canonicalize(f).map_err(|e| format!("{f}: {e}"))?; // relative or absolute
                println!("{k}: {}", f.display());
                put(&mut cfg, &k, f.to_string_lossy().into_owned(), |f| &mut f.gif);
            }
            save(&cfg)?;
        }
        ["rotate", rest @ ..] if !rest.is_empty() => {
            for (k, n) in parse_set(fans.len(), rest)? {
                let r = n.parse().ok().filter(|r| *r < 4)
                    .ok_or(format!("rotate takes 0, 1, 2 or 3 (x 90°), got {n:?}"))?;
                println!("{k}: rotate {r}");
                put(&mut cfg, &k, r, |f| &mut f.rotate);
            }
            save(&cfg)?;
        }
        ["play"] => {
            if fans.is_empty() { return Err("no 33c3:7792 fans found".into()); }
            let mut jobs = Vec::new();
            for i in 0..fans.len() {
                let Some(f) = gif_for(&cfg, i) else { continue };
                let rot = rotate_for(&cfg, i);
                eprintln!("{}: {f}, rotate {rot}", name(i));
                let frames: &'static [Vec<u8>] = video::to_jpegs(&f, rot).map_err(|e| format!("{f}: {e}"))?.leak();
                jobs.push((fans[i].clone(), frames));
            }
            let threads: Vec<_> = jobs
                .into_iter()
                .map(|(path, frames)| {
                    thread::spawn(move || {
                        let e = serial::play(&path, frames)
                            .err()
                            .map_or("stream ended".into(), |e| e.to_string());
                        // ponytail: any fan error kills the process; systemd Restart= reconnects and rediscovers ports
                        eprintln!("{}: {e}", path.display());
                        std::process::exit(1);
                    })
                })
                .collect();
            for t in threads {
                let _ = t.join();
            }
        }
        _ => return Err(usage.into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_set_args() {
        assert_eq!(parse_set(3, &["a.gif"]).unwrap(), [("all".to_string(), "a.gif")]);
        assert_eq!(parse_set(3, &["fanart.gif"]).unwrap()[0].0, "all"); // a file named fan*, not a fan
        assert_eq!(parse_set(3, &["fan3", "a", "fan1", "b"]).unwrap(),
                   [("fan3".to_string(), "a"), ("fan1".to_string(), "b")]);
        assert!(parse_set(3, &["fan2"]).is_err()); // fan without file
        assert!(parse_set(3, &["fan4", "a"]).is_err()); // no such fan
        assert!(parse_set(3, &["fan0", "a"]).is_err());
        assert!(parse_set(3, &["fan1", "a", "fan1", "b"]).is_err()); // same fan twice
    }
}
