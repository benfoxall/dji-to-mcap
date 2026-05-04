# djicap

Convert DJI flight records to [Foxglove](https://foxglove.dev) `.mcap` files for telemetry visualisation.

> **DJI data usage:** By using this tool you agree to the [DJI Flight Record API Terms](https://developer.dji.com/policies/flight_record/).

## Topics written

| Topic | Schema | Content |
|---|---|---|
| `/dji/osd` | `dji.OSD` | Position, attitude, speed, GPS, flight mode |
| `/dji/gimbal` | `dji.Gimbal` | Gimbal pitch/roll/yaw, limit flags |
| `/dji/battery` | `dji.Battery` | Voltage, current, charge level, cell voltages |
| `/dji/rc` | `dji.RC` | Stick inputs, uplink/downlink signal |
| `/dji/home` | `dji.Home` | Home point, height limit |
| `/foxglove/map_origin` | `foxglove.LocationFix` | GPS anchor for Foxglove 3D scene (emitted once) |
| `/foxglove/gps` | `foxglove.LocationFix` | GPS trace — use with the Map panel |
| `/foxglove/drone/tf` | `foxglove.FrameTransform` | Drone body pose in ENU (`world` → `base_link`) |
| `/joint_states` | `sensor_msgs/JointState` | Gimbal joint angles driving the URDF 3D model |
| `/robot_description` | `std_msgs/String` | DJI Mini 4 Pro URDF for Foxglove 3D panel (emitted once) |

## Usage

```
djicap <input.txt> [--output <out.mcap>] [--api-key <key>]
```

If `--output` is omitted the `.mcap` is written next to the input file.

### DJI API key (required for v13+ logs)

Recent DJI logs (version 13 and above) are AES-256 encrypted. Decryption
requires a **DJI Open API key**:

1. Visit <https://developer.dji.com/user> and log in.
2. Click **Create App**, choose **Open API**, fill in the details.
3. Activate the app via the confirmation email.
4. Copy the **SDK key** from your app's detail page.

Pass the key via `--api-key` or set it in your environment / a `.env` file:

```
DJI_OPEN_API_KEY=your_key_here
```

## Installation

```bash
cargo install --path .
```

Then run from anywhere:

```bash
djicap FlightRecord_2026-01-02_\[11-04-28\].txt
```

## Video and photo conversion

DJI video (`.MP4`) and photos (`.JPG`) can be converted to a separate MCAP file
using the bundled `video2mcap.py` script.  Timestamps are taken from the DJI
filename (`DJI_YYYYMMDDHHMMSS_…`) so they align with the telemetry MCAP from
the same flight.

**One-time setup:**

```bash
python3 -m venv .venv
.venv/bin/pip install av mcap-protobuf-support foxglove-schemas-protobuf
```

**Convert:**

```bash
.venv/bin/python3 video2mcap.py ./Video out_video.mcap
```

This writes:
- `/video` — `foxglove.CompressedVideo` (H.265 packets, ~30 Hz)
- `/image` — `foxglove.CompressedImage` (JPEG, one per photo)

Open both `out.mcap` (telemetry) and `out_video.mcap` (video) in Foxglove
Studio simultaneously — the shared timestamps let you scrub video and 3D
visualisation in sync.

> **Note:** Foxglove Studio's browser-based player has limited H.265 support on
> some platforms. If the video panel shows a blank image, try Chrome on macOS
> (which has native HEVC decoding) or transcode to H.264 first with FFmpeg:
> `ffmpeg -i input.MP4 -c:v libx264 -crf 23 output.mp4`

## Getting flight logs off the drone

```bash
adb pull /sdcard/Android/data/dji.go.v5/files/FlightRecord FlightRecord
```

## Development

```bash
cargo build
cargo test                        # unit tests (coordinate math)
cargo test -- --include-ignored   # also runs the integration test (needs FlightRecord/)

# Inspect the output
mcap info output.mcap
mcap inspect output.mcap
```

## Based on

- <https://github.com/swarmis-us/arducap> — MCAP writing patterns and Foxglove transforms
- <https://github.com/lvauvillier/dji-log-parser> — DJI log decryption and frame parsing
