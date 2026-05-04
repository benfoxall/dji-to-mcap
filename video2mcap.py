#!/usr/bin/env python3
"""
Convert DJI video and photo files to a Foxglove MCAP file.

Usage:
    python3 video2mcap.py ./Video out.mcap

For each .MP4: writes foxglove.CompressedVideo messages on /video
For each .JPG: writes foxglove.CompressedImage messages on /image

Timestamps are derived from DJI filenames (DJI_YYYYMMDDHHMMSS_…)
so the output aligns with djicap telemetry MCAP files from the same flight.
All times are interpreted as UTC.
"""

import sys
import os
import re
import glob
from datetime import datetime, timezone

import av
from google.protobuf.timestamp_pb2 import Timestamp
from foxglove_schemas_protobuf.CompressedVideo_pb2 import CompressedVideo
from foxglove_schemas_protobuf.CompressedImage_pb2 import CompressedImage
from mcap_protobuf.writer import Writer as McapProtobufWriter

# DJI filename timestamp pattern: DJI_YYYYMMDDHHMMSS_NNNN_D.EXT
DJI_TIMESTAMP_RE = re.compile(r"DJI_(\d{4})(\d{2})(\d{2})(\d{2})(\d{2})(\d{2})_")


def parse_dji_timestamp(path: str) -> int | None:
    """Return nanosecond UTC timestamp from a DJI filename, or None."""
    name = os.path.basename(path)
    m = DJI_TIMESTAMP_RE.search(name)
    if not m:
        return None
    yr, mo, dy, hr, mi, se = (int(x) for x in m.groups())
    dt = datetime(yr, mo, dy, hr, mi, se, tzinfo=timezone.utc)
    return int(dt.timestamp() * 1_000_000_000)


def make_timestamp(ns: int) -> Timestamp:
    ts = Timestamp()
    ts.seconds = ns // 1_000_000_000
    ts.nanos = ns % 1_000_000_000
    return ts


def process_video(path: str, writer: McapProtobufWriter, topic: str = "/video") -> int:
    """Demux MP4, write CompressedVideo messages. Returns message count."""
    start_ns = parse_dji_timestamp(path)
    if start_ns is None:
        print(f"  WARNING: cannot parse timestamp from filename, using 0", file=sys.stderr)
        start_ns = 0

    count = 0
    with av.open(path) as container:
        stream = next((s for s in container.streams if s.type == "video"), None)
        if stream is None:
            print(f"  No video stream found in {path}", file=sys.stderr)
            return 0

        codec = stream.codec_context.name  # "hevc", "h264", etc.
        # Foxglove uses "h265" for HEVC
        fmt = "h265" if codec in ("hevc", "h265") else codec
        time_base = stream.time_base  # Fraction

        for packet in container.demux(stream):
            if packet.pts is None or packet.size == 0:
                continue
            pts_ns = int(packet.pts * float(time_base) * 1_000_000_000)
            abs_ns = start_ns + pts_ns

            msg = CompressedVideo(
                timestamp=make_timestamp(abs_ns),
                frame_id="camera",
                data=bytes(packet),
                format=fmt,
            )
            writer.write_message(topic=topic, message=msg, log_time=abs_ns, publish_time=abs_ns)
            count += 1

    return count


def process_image(path: str, writer: McapProtobufWriter, topic: str = "/image") -> int:
    """Write a single JPEG as a CompressedImage message. Returns 1 on success."""
    ts_ns = parse_dji_timestamp(path)
    if ts_ns is None:
        print(f"  WARNING: cannot parse timestamp from {os.path.basename(path)}, skipping", file=sys.stderr)
        return 0

    with open(path, "rb") as f:
        data = f.read()

    msg = CompressedImage(
        timestamp=make_timestamp(ts_ns),
        frame_id="camera",
        data=data,
        format="jpeg",
    )
    writer.write_message(topic=topic, message=msg, log_time=ts_ns, publish_time=ts_ns)
    return 1


def main():
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} <input_dir_or_files...> <output.mcap>")
        sys.exit(1)

    *inputs, output_path = sys.argv[1:]

    # Expand directories to files
    files = []
    for inp in inputs:
        if os.path.isdir(inp):
            files += sorted(glob.glob(os.path.join(inp, "*.MP4")))
            files += sorted(glob.glob(os.path.join(inp, "*.mp4")))
            files += sorted(glob.glob(os.path.join(inp, "*.JPG")))
            files += sorted(glob.glob(os.path.join(inp, "*.jpg")))
        else:
            files.append(inp)

    if not files:
        print("No input files found.", file=sys.stderr)
        sys.exit(1)

    print(f"Converting {len(files)} file(s) → {output_path}")

    with open(output_path, "wb") as out_f:
        with McapProtobufWriter(out_f) as writer:
            for path in files:
                ext = os.path.splitext(path)[1].upper()
                print(f"  {os.path.basename(path)}", end=" ... ", flush=True)
                if ext in (".MP4", ".MOV", ".WEBM"):
                    n = process_video(path, writer)
                    print(f"{n} video packets")
                elif ext in (".JPG", ".JPEG", ".PNG"):
                    n = process_image(path, writer)
                    print(f"ok" if n else "skipped")
                else:
                    print("skipped (unknown type)")

    print(f"Done → {output_path}")


if __name__ == "__main__":
    main()
