// Image/GIF -> 360x360 JPEG frames for the fan, via ffmpeg.
use crate::R;
use std::{env, fs, process::Command};

pub(crate) const FPS: u32 = 20; // fan max is 30

// Filter matching the app: scale keeping aspect until it covers 360x360, crop the centre,
// black out everything outside the round panel. Rotation is added by to_jpegs.
const FILTER: &str = "scale=360:360:force_original_aspect_ratio=increase,crop=360:360,format=rgb24,\
geq=r='r(X,Y)*lte(hypot(X-179.5,Y-179.5),180)':g='g(X,Y)*lte(hypot(X-179.5,Y-179.5),180)':\
b='b(X,Y)*lte(hypot(X-179.5,Y-179.5),180)'";

// Filter suffix for `rot` extra quarter turns clockwise, on top of the 180° the fan needs (angle 180).
fn rotation(rot: u32) -> &'static str {
    match (2 + rot) % 4 {
        0 => "",
        1 => ",transpose=clock",
        2 => ",hflip,vflip",
        _ => ",transpose=cclock",
    }
}

// ffmpeg → 360x360 JPEG frames at FPS, through FILTER, then rotated.
pub(crate) fn to_jpegs(input: &str, rot: u32) -> R<Vec<Vec<u8>>> {
    let dir = env::temp_dir().join(format!("jlfan-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    let ok = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i", input, "-vf"])
        .arg(format!("fps={FPS},{FILTER}{}", rotation(rot)))
        .args(["-pix_fmt", "yuvj420p", "-q:v", "3"]) // the fan's decoder wants 4:2:0 like the app sends; 4:4:4 shows stripes
        .arg(dir.join("%05d.jpg"))
        .status()?
        .success();
    let mut files: Vec<_> = fs::read_dir(&dir)?.flatten().map(|e| e.path()).collect();
    files.sort();
    let frames = files.iter().map(fs::read).collect::<Result<Vec<_>, _>>();
    let _ = fs::remove_dir_all(&dir);
    if !ok || files.is_empty() {
        return Err("ffmpeg failed".into());
    }
    Ok(frames?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates() {
        // base 180° for the fan, plus the user's turns
        assert_eq!([0, 1, 2, 3].map(rotation), [",hflip,vflip", ",transpose=cclock", "", ",transpose=clock"]);
    }
}
