// Serial I/O with one fan: open the port, send commands, stream frames.
use crate::protocol::{check, frame, read_reply, stream_frame};
use crate::video::FPS;
use crate::R;
use std::{
    io::Write,
    path::Path,
    thread,
    time::{Duration, Instant},
};

type Port = Box<dyn serialport::SerialPort>;
const RESET: [u8; 4] = [0xFF, 0xD9, 0xFF, 0xD9];

pub(crate) fn open(path: &Path) -> R<Port> {
    let mut p = serialport::new(path.to_string_lossy(), 115200)
        .timeout(Duration::from_secs(3))
        .open()?;
    p.write_data_terminal_ready(true)?; // the Windows app (node serialport) asserts DTR
    p.write_all(&RESET)?;
    thread::sleep(Duration::from_millis(100));
    p.clear(serialport::ClearBuffer::Input)?;
    Ok(p)
}

pub(crate) fn cmd(p: &mut Port, c: u8, payload: &[u8]) -> R<Vec<u8>> {
    p.write_all(&frame(c, payload))?;
    check(c, read_reply(p)?)
}

// Startup sequence from local/1-startup.pcapng, then loop the frames forever with a 0x11 keepalive.
pub(crate) fn play(path: &Path, frames: &[Vec<u8>]) -> R<()> {
    let mut p = open(path)?;
    cmd(&mut p, 0x15, &[0, 3])?;
    cmd(&mut p, 0x11, &[])?;
    let (tick, mut keepalive) = (Duration::from_secs(1) / FPS, Instant::now());
    for f in frames.iter().cycle() {
        let t = Instant::now();
        p.write_all(&stream_frame(f))?;
        if keepalive.elapsed() >= Duration::from_millis(1500) {
            p.write_all(&frame(0x11, &[]))?;
            p.clear(serialport::ClearBuffer::Input)?; // keepalive replies are ignored
            keepalive = Instant::now();
        }
        thread::sleep(tick.saturating_sub(t.elapsed()));
    }
    Ok(())
}
