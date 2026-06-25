# ScreenStream

ScreenStream sends your screen and audio over a local network to OBS using encrypted SRT.

This release-focused guide is for users, not developers.

## Quick Start

1. Download the release package for your operating system.
2. Extract the contents.
3. Run the `screenstream` binary.
4. Open OBS.
5. Add a **Media Source**.
6. Uncheck **Local File** and paste the SRT URL shown by ScreenStream.
7. Set **Input Format** to `mpegts`.

## Windows Notes

- Run `screenstream-windows-x86_64.exe` from the extracted folder.
- If Windows reports `avformat-62.dll was not found`, do not move the exe away from the extracted package.
- The bundled release already includes the DLLs the app needs.

## OBS Setup

1. Start ScreenStream.
2. Copy the **OBS Source URL** shown in the app.
3. In OBS:
   - Add **Media Source**
   - Uncheck **Local File**
   - Paste the URL into **Input**
   - Set **Input Format** to `mpegts`
   - Click **OK**

The URL includes the passphrase so OBS can decrypt the incoming stream.

## Configuration

ScreenStream saves settings in `config.json` next to the binary.
Edit that file and restart the app.

Example `config.json`:

```json
{
  "port": 4200,
  "passphrase": "your-secret-passphrase",
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

Recommended values:

- `port`: `4200`
- `passphrase`: use a strong secret
- `video_codec`: `H264`
- `resolution`: `R1080p` or `R720p`
- `capture_system_audio`: `true` to stream desktop audio

## Common Problems

- OBS will not connect:
  - Make sure the URL is copied correctly.
  - Use `mpegts` as the input format.
  - Confirm both machines are on the same LAN.

- Missing DLLs on Windows:
  - Run the exe from the extracted release folder.
  - Do not run the executable alone without the packaged files.

- macOS screen capture blocked:
  - Open **System Settings → Privacy & Security → Screen Recording**.
  - Allow ScreenStream and restart the app.

## License

MIT
