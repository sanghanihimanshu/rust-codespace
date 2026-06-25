# ScreenStream

High-quality, **encrypted** local-network screen streaming built in Rust.  
OBS connects via a standard SRT URL — no internet required.

```
┌──────────────────────────┐        LAN (Wi-Fi / USB / Ethernet)
│  ScreenStream (sender)   │ ──────────────────────────────────▶  OBS
│  Screen + Audio capture  │   srt://192.168.x.x:4200           Media Source
│  H.264 / H.265 encode   │   AES-256 encrypted in-transit
│  SRT mux + AES-256 enc  │
└──────────────────────────┘
```

---

## Features

| Feature | Detail |
|---|---|
| **Codec** | H.264 (`libx264`) or H.265 (`libx265`) via FFmpeg |
| **Container** | MPEG-TS over SRT |
| **Encryption** | AES-256 (SRT built-in, passphrase-based) |
| **Audio** | AAC via FFmpeg; mic or system-audio loopback |
| **Transport** | SRT — ultra-low latency, reliable UDP |
| **Discovery** | mDNS (`_srt._tcp.local.`) — auto-discovered on LAN |
| **Networks** | Any interface — Wi-Fi, USB tethering, Ethernet, USB-C |
| **UI** | GPUI (GPU-accelerated, from the Zed editor) |
| **OBS compat** | Media Source with native SRT input, no plugin needed |

---

## Installation

### Linux (Debian / Ubuntu)

```bash
sudo apt-get update
sudo apt-get install -y \
  libavcodec-dev libavformat-dev libavutil-dev \
  libswscale-dev libswresample-dev libavdevice-dev \
  libsrt-openssl-dev \
  libxkbcommon-dev libwayland-dev \
  libx11-dev libxrandr-dev libxcb1-dev \
  libfontconfig1-dev libfreetype6-dev \
  libasound2-dev libpulse-dev \
  libpipewire-0.3-dev libspa-0.2-dev \
  libdbus-1-dev \
  clang cmake pkg-config \
  libx264-dev

cargo build --release
./target/release/screenstream
```

### macOS

```bash
brew install ffmpeg srt pkg-config
cargo build --release
./target/release/screenstream
```

> On macOS 12+, grant **Screen Recording** permission in  
> **System Settings → Privacy & Security → Screen Recording**.

### Windows

Install [vcpkg](https://vcpkg.io) and set `VCPKG_ROOT` to your vcpkg checkout root.

```powershell
cd C:\vcpkg
.\vcpkg.exe install ffmpeg:x64-windows srt:x64-windows --triplet x64-windows
$env:PKG_CONFIG_PATH = "$env:VCPKG_ROOT\installed\x64-windows\lib\pkgconfig"
cargo build --release
.\target\release\screenstream.exe
```

If `srt:x64-windows` fails with `C:\vcpkg\ports\srt: error: srt does not exist`, update and bootstrap your vcpkg tree:

```powershell
git pull
.\bootstrap-vcpkg.bat
.\vcpkg.exe install ffmpeg:x64-windows srt:x64-windows --triplet x64-windows
```

---

## OBS Setup

1. Launch ScreenStream — note the **OBS Source URL** in the window.
2. Click **Start Streaming** — wait for **● Live** status.
3. In OBS:
   - **Sources → + → Media Source**
   - Uncheck **Local File**
   - Paste the SRT URL into **Input** (e.g. `srt://192.168.1.10:4200?passphrase=...&pbkeylen=32`)
   - Set **Input Format** → `mpegts`
   - **OK**

OBS reads the `passphrase=` parameter and decrypts in-transit. No OBS plugin required.

---

## Configuration

On first run, a `config.json` is created next to the binary. Edit it and restart.

```json
{
  "port": 4200,
  "passphrase": "change-me-to-something-secret",
  "video_codec": "H264",
  "video_kbps": 8000,
  "resolution": "R1080p",
  "fps": 60,
  "audio_kbps": 192,
  "display_index": 0,
  "capture_system_audio": false,
  "mic_device": null,
  "bind_interface": null,
  "service_name": "ScreenStream"
}
```

| Field | Values | Default |
|---|---|---|
| `port` | 1–65535 | `4200` |
| `passphrase` | any string; empty = no encryption | `"screenstream_secret"` |
| `video_codec` | `"H264"` or `"H265"` | `"H264"` |
| `video_kbps` | bitrate in kbps | `8000` |
| `resolution` | `"Native"`, `"R1080p"`, `"R720p"`, `"R480p"` | `"R1080p"` |
| `fps` | frames per second | `60` |
| `audio_kbps` | audio bitrate in kbps | `192` |
| `display_index` | 0 = primary display | `0` |
| `capture_system_audio` | `true`/`false` | `false` |
| `mic_device` | device name or `null` for default | `null` |
| `bind_interface` | interface name or `null` for all | `null` |
| `service_name` | mDNS name visible on LAN | `"ScreenStream"` |

### System Audio on Linux

```bash
# Find PulseAudio monitor source
pactl list short sources | grep monitor
# Example output: alsa_output.pci-0000_00_1f.3.analog-stereo.monitor

# Set in config.json:
# "capture_system_audio": true
# "mic_device": "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor"
```

### USB / Tethered Streaming

```bash
# Find the USB network interface
ip addr show | grep -E "usb|rndis|enp"
# Example output: 4: usb0: ...

# Set in config.json:
# "bind_interface": "usb0"
```

---

## Encryption

| Layer | Mechanism |
|---|---|
| Transport | SRT AES-256, encrypted at UDP level |
| Key derivation | PBKDF from passphrase → AES-256 key |
| OBS compat | `?passphrase=KEY&pbkeylen=32` in the SRT URL |
| Isolation | LAN-only listener — no ports open to internet |

> **Do not share your OBS URL publicly** — it contains the passphrase.  
> Use a random passphrase per session on untrusted networks.

---

## GitHub Actions CI / CD

| Workflow | Trigger | Actions |
|---|---|---|
| [`ci.yml`](.github/workflows/ci.yml) | Push / PR to `main` | Build + Clippy on Linux, macOS, Windows |
| [`release.yml`](.github/workflows/release.yml) | `git tag v1.2.3` | Build binaries, bump version, create GitHub Release |

### How to create a release

```bash
# Tag your release (semantic versioning)
git tag v0.1.0
git push origin v0.1.0
```

The `release.yml` workflow will:
1. Build binaries for Linux x86-64, macOS arm64, macOS x86-64, Windows x86-64
2. Package as `.tar.gz` (Linux/macOS) or `.zip` (Windows)
3. Bump `Cargo.toml` version to match the tag
4. Create a GitHub Release with all binaries attached and a generated changelog

### Required Repository Secrets / Permissions

The release workflow needs **write access to contents** (to create releases).  
In GitHub: **Settings → Actions → General → Workflow permissions → Read and write permissions**.

No additional secrets are required — the workflow uses the built-in `GITHUB_TOKEN`.

---

## Architecture

```
src/
├── main.rs              Entry point — GPUI app + shared state init
├── config.rs            StreamConfig struct (serialised to config.json)
├── app_state.rs         AppState (status, stats) — shared via Arc<Mutex<>>
├── pipeline.rs          Orchestrates 5 threads: capture → encode → mux
├── capture/
│   ├── screen.rs        Screen capture via scap (X11/WGC/ScreenCaptureKit)
│   └── audio.rs         Audio capture via cpal (mic / loopback)
├── encode/
│   ├── video.rs         H.264/H.265 encode via FFmpeg (hwaccel optional)
│   └── audio.rs         AAC encode via FFmpeg
├── stream/
│   ├── srt_output.rs    MPEG-TS → SRT with AES-256 via FFmpeg
│   └── discovery.rs     mDNS service announcement (_srt._tcp.local.)
└── ui/
    ├── main_window.rs   GPUI window — Stream / Settings tabs
    └── theme.rs         Catppuccin Macchiato colour palette
```

All threads communicate via bounded `crossbeam_channel` queues.  
UI state is a `parking_lot::Mutex<AppState>` shared by both the GPUI thread and pipeline threads.

---

## License

MIT
