# jlfan

`jlfan` shows an image or GIF on the round displays of **Jungle Leopard Interstellar V4 LCS** fans from Linux. The official app only runs on Windows.

Each fan shows up as USB device `33c3:7792` ("HONGTAI MONITOR"), a plain USB serial (CDC-ACM) device at `/dev/ttyACM*`. `jlfan` streams JPEG frames to it, the same way the Windows app does.

## Build and install

`build.sh` does everything:

| Command | What it does |
|---|---|
| `./build.sh` | Release build, then the tests |
| `./build.sh setup` | Installs `rust` and `ffmpeg`, and adds you to the `uucp` group so you can write to `/dev/ttyACM*`. Log in again afterwards. |
| `./build.sh build` | Release build only |
| `./build.sh install` | Installs `/usr/bin/jlfan`, the sample GIFs in `/usr/share/jlfan/` and the user service. `DESTDIR=dir` stages the install into `dir` without sudo. |
| `./build.sh run` | Installs, then enables and restarts the background service |
| `./build.sh uninstall` | Stops the service and removes everything `install` placed |

## Usage

| Command | What it does |
|---|---|
| `jlfan info` | Lists the detected fans as `fan1`, `fan2`, … with their USB port, tty and saved GIF |
| `jlfan info fan2` | Prints that fan's info as JSON: resolution, firmware, brightness, storage |
| `jlfan set anim.gif` | Saves the GIF or image for every fan, replacing any per-fan choices |
| `jlfan set fan2 anim.gif` | Saves it for one fan only |
| `jlfan set fan1 a.gif fan3 b.png` | Saves a file for each named fan. Fans you don't name keep their current GIF. |
| `jlfan rotate 1` | Rotates every fan by 1 × 90° clockwise. `N` is 0, 1, 2 or 3 (0°, 90°, 180°, 270°). |
| `jlfan rotate fan2 3 fan4 2` | Rotates only the named fans |
| `jlfan play` | Streams the saved GIFs and keeps running. This is what the service runs. |

File paths can be relative or absolute.

Things to know:

- **Fans are numbered by USB port** (for example `8-10.2.3`), not by `ttyACM` number. So `fan2` stays the same fan after a replug, as long as it goes back in the same port. `set` prints which fan got which file.
- **Changes are remembered.** `set` and `rotate` save the choice and restart the service, so it applies immediately and survives reboots.
- **One program per fan.** A fan can only be used by one `jlfan` at a time. Stop the service before running `info fanN` or a manual `play`: `systemctl --user stop jlfan`. `info` and `set` work while it runs.
- **The picture only stays while `jlfan` runs.** Live mode lasts as long as the stream keeps coming, so the service runs `jlfan play` in the background.

### Background service

`./build.sh run` enables the `jlfan` user service, which runs `jlfan play` and starts at login. If a fan disconnects, the process exits and systemd restarts it after 3 s. Logs: `journalctl --user -u jlfan`.

Your choices are saved in `~/.config/jlfan/config.toml`. `set` and `rotate` write it, and you can also edit it by hand:

```toml
[all]
gif = "/home/me/gifs/hue.gif"
rotate = 1               # extra quarter turns clockwise, 0..3

[fan2]
gif = "/home/me/gifs/bar.gif"
```

- **Lookup:** a fan uses its own section's value if it has one, otherwise `[all]`'s. Here, `fan2` shows `bar.gif` rotated by 1.
- **Setting `[all]`:** `set FILE` or `rotate N` without a fan clears that setting from every fan section, so it really applies to all fans.
- **Defaults:** with no GIF saved anywhere, every fan shows `/usr/share/jlfan/test.gif`.
- **After a hand edit:** run `systemctl --user restart jlfan`.

## Protocol

The protocol was reverse-engineered from the Windows app (an Electron app that uses Node `serialport`) and checked against USB captures. It has no encryption and needs no special driver. The baud rate doesn't matter.

### Command frame (both directions)

```
55 AA | LEN u16 LE | CMD u8 | PAYLOAD | SUM u16 LE
LEN = total frame length (payload + 7)
SUM = sum of all preceding bytes, & 0xFFFF
```

Example: `get info` is `55 AA 07 00 06 0C 01`.

A reply echoes `CMD`. Its payload is either JSON or a single status byte:

| Status | Meaning |
|---|---|
| `00` | ok |
| `01` | operation failed |
| `02` | out of memory |
| `03` | internal storage full |
| `04` | SD card full |
| `05` | file not found |
| `06` | file open failed |
| `07` | file write failed |

### Commands

| CMD | Meaning | Payload |
|---|---|---|
| `06` | Get device info | none; replies with JSON |
| `03` | Brightness | `[0..100]` |
| `11` | Start live mode / keepalive | none |
| `14` | Stop (the screen goes blank) | `[0, 0]` |
| `15` | Motion timeout | `[0, 3]` (the app sends this at startup) |
| `01` | Restart | none |
| `12` / `13` | Upload startup / standby animation | file size `u32 LE`, then the raw file |

The raw 4-byte marker `FF D9 FF D9` has no framing. It resets or ends the live stream.

Device info for this fan: 360×360 panel, `angle: 180`, model `TXW818-ST7716-1.5inch-hor`, firmware 3.2.

### Live streaming

Live frames have no `55 AA` header:

```
LEN u32 LE | JPEG bytes | SUM u16 LE
LEN = JPEG length
SUM = sum of LEN bytes + JPEG bytes, & 0xFFFF
```

Sequence:

```
FF D9 FF D9          reset
55 AA … 15 [0,3]     -> status 00
55 AA … 11           -> status 00, live mode on
frame, frame, …      360x360 baseline JPEG, 4:2:0 (4:4:4 shows stripes), rotated 180°, max 30 fps (jlfan sends 20)
55 AA … 11           keepalive every 1.5 s; frames get no reply
```

To exit, the app sends `FF D9 FF D9` and then `14 [0,0]`.

## Repository layout

| Path | Contents |
|---|---|
| `src/main.rs` | The whole tool, plus a test against bytes captured from the Windows app |
| `build.sh` | Build, test, install and run (see above) |
| `misc/` | `jlfan.service` (systemd user service) and the sample GIFs that `build.sh install` puts in `/usr/share/jlfan/` |
| `local/` | Reverse-engineering notes (`system-design.md`) and the USB captures of the Windows app (`*.pcapng`) |

## Known limitations

- **Storing an animation on the fan doesn't work yet.** Uploading an H.264 MP4 to `12`/`13` is accepted, but the screen then plays black, so `jlfan` streams instead. The correct file format is still unknown.
- **Brightness can't be changed yet.** It isn't exposed, so the fans keep their current setting.
- **Stopping `jlfan` doesn't send `14`.** The last frame stays on screen.
