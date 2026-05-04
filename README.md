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
| `/foxglove/gimbal/tf` | `foxglove.FrameTransform` | Gimbal orientation (`base_link` → `gimbal_link`) |

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
