// Wire format of the fan's serial protocol. Pure byte work, no I/O beyond a Read.
use crate::R;
use std::io::Read;

const STATUS: [&str; 8] = [
    "ok",
    "operation failed",
    "out of memory",
    "internal storage full",
    "SD card full",
    "file not found",
    "file open failed",
    "file write failed",
];

fn sum(b: &[u8]) -> [u8; 2] {
    b.iter()
        .fold(0u16, |s, &x| s.wrapping_add(x as u16))
        .to_le_bytes()
}

// `55 AA | LEN u16 | CMD | payload | SUM u16`, LEN = whole frame, SUM = byte sum of the rest.
pub(crate) fn frame(cmd: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = vec![0x55, 0xAA];
    f.extend((payload.len() as u16 + 7).to_le_bytes());
    f.push(cmd);
    f.extend(payload);
    f.extend(sum(&f));
    f
}

// Live frame: `LEN u32 | JPEG | SUM u16`, no 55 AA header.
pub(crate) fn stream_frame(jpeg: &[u8]) -> Vec<u8> {
    let mut f = (jpeg.len() as u32).to_le_bytes().to_vec();
    f.extend(jpeg);
    f.extend(sum(&f));
    f
}

// Reads one reply frame, skipping any bytes before `55 AA`. Returns (cmd, payload).
pub(crate) fn read_reply(port: &mut dyn Read) -> R<(u8, Vec<u8>)> {
    let (mut b, mut prev) = ([0u8; 1], 0u8);
    loop {
        port.read_exact(&mut b)?;
        if prev == 0x55 && b[0] == 0xAA {
            break;
        }
        prev = b[0];
    }
    let mut len = [0u8; 2];
    port.read_exact(&mut len)?;
    let n = u16::from_le_bytes(len) as usize;
    if n < 7 {
        return Err(format!("bad reply length {n}").into());
    }
    let mut rest = vec![0u8; n - 4];
    port.read_exact(&mut rest)?;
    let body = &rest[..n - 6];
    if sum(&[&[0x55, 0xAA, len[0], len[1]], body].concat()) != rest[n - 6..] {
        return Err("reply checksum mismatch".into());
    }
    Ok((body[0], body[1..].to_vec()))
}

// A 1-byte reply is a status code; anything else (JSON) passes through.
pub(crate) fn check(cmd: u8, (rc, p): (u8, Vec<u8>)) -> R<Vec<u8>> {
    if rc != cmd {
        return Err(format!("expected reply to 0x{cmd:02x}, got 0x{rc:02x}").into());
    }
    match p.as_slice() {
        [s] if *s != 0 => Err(format!(
            "fan: {}",
            STATUS.get(*s as usize).unwrap_or(&"unknown error")
        )
        .into()),
        _ => Ok(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_capture() {
        // bytes taken from local/1-startup.pcapng
        assert_eq!(frame(0x06, &[]), [0x55, 0xAA, 0x07, 0x00, 0x06, 0x0C, 0x01]);
        assert_eq!(
            frame(0x15, &[0, 3]),
            [0x55, 0xAA, 0x09, 0x00, 0x15, 0x00, 0x03, 0x20, 0x01]
        );
        assert_eq!(frame(0x11, &[]), [0x55, 0xAA, 0x07, 0x00, 0x11, 0x17, 0x01]);
        let reply: &[u8] = &[0xFF, 0x55, 0xAA, 0x08, 0x00, 0x15, 0x00, 0x1C, 0x01];
        assert_eq!(
            check(0x15, read_reply(&mut &reply[..]).unwrap()).unwrap(),
            [0]
        );
        let bad: &[u8] = &[0x55, 0xAA, 0x08, 0x00, 0x15, 0x03, 0x1F, 0x01];
        assert!(check(0x15, read_reply(&mut &bad[..]).unwrap()).is_err());
        // live frame: LEN u32 LE covers the JPEG only; SUM covers LEN + JPEG
        assert_eq!(
            stream_frame(&[0xFF, 0xD8, 0xFF, 0xD9]),
            [4, 0, 0, 0, 0xFF, 0xD8, 0xFF, 0xD9, 0xB3, 0x03]
        );
    }
}
