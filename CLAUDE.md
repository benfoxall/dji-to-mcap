# djicap — Claude session guide

## What this is

A Rust CLI that converts DJI flight logs (`.txt`) to Foxglove `.mcap` files. Optionally embeds matching MP4 video and JPEG photos from a media directory into the same MCAP on a shared timeline.

## Key commands

```bash
# Build
cargo build

# Run against real data (flight logs and video are gitignored)
djicap FlightRecord/FlightRecord_2026-04-25_\[12-16-41\].txt --media Video/

# With transcoding to reduce file size
djicap FlightRecord/... --media Video/ --scale 1280

# Install as system binary
cargo install --path .
```

The `DJI_OPEN_API_KEY` is in `.env` (gitignored). Required for v13+ logs.

## Source layout

```
src/
  main.rs          CLI (clap): --media, --scale, --api-key, --output
  lib.rs           Module declarations
  pipeline.rs      Orchestrates parse → transform → MCAP write
  transformers/
    mod.rs         TransformedMessage struct + Transformer trait
    raw.rs         Raw /dji/* topics (one per frame type)
    foxglove.rs    Fused Foxglove topics (GPS, FrameTransform)
    joints.rs      JointState for gimbal URDF animation
  video.rs         --media: find/match MP4+JPG, Annex-B BSF, protobuf encode
  schema_compressed_video.bin   } Binary FileDescriptorSet blobs embedded
  schema_compressed_image.bin   } via include_bytes! for protobuf schemas
mini4pro.urdf      Embedded at compile time for /robot_description topic
```

## Non-obvious design decisions

**Timestamps:** DJI filenames use local time (BST = UTC+1 here). The UTC offset is derived at runtime by comparing the MP4 filename timestamp against the container's `creation_time` metadata tag (which FFmpeg stores as UTC). This offset is then applied to all JPG filenames.

**Annex-B BSF:** DJI HEVC video is HVCC (length-prefixed NAL units, SPS/PPS in extradata). Foxglove needs Annex-B (start-code NAL units with inline SPS/PPS). The `hevc_mp4toannexb` / `h264_mp4toannexb` FFmpeg BSF handles this. `av_bsf_*` functions are not in `ffmpeg-sys-next`'s bindings so they're declared via `extern "C"` with a manually defined `AVBSFContext` struct matching the FFmpeg 7 layout (no `internal` field between `filter` and `priv_data`).

**Protobuf without prost:** `foxglove.CompressedVideo` and `foxglove.CompressedImage` are encoded manually with varint helpers. The schema data (FileDescriptorSet) is pre-extracted from the `foxglove-schemas-protobuf` Python package.

**`--scale` flag:** Triggers a full decode → libswscale resize → libx264 encode pipeline instead of passthrough demux+BSF.

## Test data locations

```
FlightRecord/    DJI .txt flight logs (gitignored)
Video/           MP4 + JPG files from the drone (gitignored)
```

Real logs for the April 2026 flight: `FlightRecord_2026-04-25_[12-16-41].txt` with media in `Video/`.
