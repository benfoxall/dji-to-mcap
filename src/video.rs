//! Video and photo processing for DJI media files.
//!
//! Reads MP4 and JPEG files from a directory, matches them to the flight
//! time window (using the MP4's embedded UTC creation_time), converts
//! H.265/H.264 bitstreams to Annex-B via the mp4toannexb BSF (fixes
//! "waiting for keyframe"), and writes foxglove.CompressedVideo /
//! foxglove.CompressedImage protobuf messages into the MCAP.
//!
//! With --scale <WIDTH> the video is transcoded to H.264 at the given
//! pixel width (height computed proportionally) before writing.

use std::collections::BTreeMap;
use std::ffi::{CString, c_char, c_int, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;

use anyhow::{bail, Context, Result};
use ffmpeg_next as ffmpeg;
use ffmpeg_sys_next as ffsys;
use mcap::records::MessageHeader;
use mcap::Writer;
use prost::Message as _;

use crate::schemas::CAMERA_CALIBRATION_SCHEMA;

// Protobuf types generated from proto/foxglove/*.proto at build time.
mod proto {
    include!(concat!(env!("OUT_DIR"), "/foxglove.rs"));
}

// Combined FileDescriptorSet for foxglove.CompressedVideo and foxglove.CompressedImage.
// Passed to add_schema for both — MCAP readers find the right message by name.
const FOXGLOVE_SCHEMA: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/foxglove_descriptor.bin"));

const MATCH_PADDING_NS: u64 = 5 * 60 * 1_000_000_000;

// ── BSF types not yet exposed by ffmpeg-sys-next ─────────────────────────────
//
// av_bsf_* lives in libavcodec/bsf.h which the crate's bindgen pass does not
// currently include. We declare the opaque struct and the functions we need
// manually; the linker picks them up from libavcodec.

// FFmpeg 7.x AVBSFContext layout (bsf.h).
// In FFmpeg 6.x there was an extra "internal" field between filter and priv_data;
// that field was removed in 7.x, shifting par_in from offset 32 to 24.
#[repr(C)]
struct AVBSFContext {
    av_class:         *const c_void,
    filter:           *const c_void,
    priv_data:        *mut   c_void,
    pub par_in:       *mut ffsys::AVCodecParameters,
    par_out:          *mut ffsys::AVCodecParameters,
    pub time_base_in: ffsys::AVRational,
    time_base_out:    ffsys::AVRational,
}

extern "C" {
    fn av_bsf_get_by_name(name: *const c_char) -> *const c_void;
    fn av_bsf_alloc(filter: *const c_void, ctx: *mut *mut AVBSFContext) -> c_int;
    fn av_bsf_init(ctx: *mut AVBSFContext) -> c_int;
    fn av_bsf_send_packet(ctx: *mut AVBSFContext, pkt: *mut ffsys::AVPacket) -> c_int;
    fn av_bsf_receive_packet(ctx: *mut AVBSFContext, pkt: *mut ffsys::AVPacket) -> c_int;
    fn av_bsf_free(ctx: *mut *mut AVBSFContext);
}

// ── Protobuf encoding ─────────────────────────────────────────────────────────

fn ts_proto(ns: u64) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: (ns / 1_000_000_000) as i64,
        nanos:   (ns % 1_000_000_000) as i32,
    }
}

fn encode_video(ts_ns: u64, frame_id: &str, data: &[u8], format: &str) -> Vec<u8> {
    proto::CompressedVideo {
        timestamp: Some(ts_proto(ts_ns)),
        frame_id:  frame_id.to_string(),
        data:      data.to_vec(),
        format:    format.to_string(),
    }.encode_to_vec()
}

fn encode_image(ts_ns: u64, frame_id: &str, data: &[u8], format: &str) -> Vec<u8> {
    proto::CompressedImage {
        timestamp: Some(ts_proto(ts_ns)),
        frame_id:  frame_id.to_string(),
        data:      data.to_vec(),
        format:    format.to_string(),
    }.encode_to_vec()
}

// ── MCAP channel bookkeeping ──────────────────────────────────────────────────

struct Channels {
    video:       Option<(u16, u32)>,
    image:       Option<(u16, u32)>,
    camera_info: Option<u16>,
}

impl Channels {
    fn new() -> Self { Self { video: None, image: None, camera_info: None } }

    fn video_id<W: std::io::Write + std::io::Seek>(&mut self, w: &mut Writer<W>) -> Result<u16> {
        if let Some((id, _)) = self.video { return Ok(id); }
        let sid = w.add_schema("foxglove.CompressedVideo", "protobuf", FOXGLOVE_SCHEMA)?;
        let cid = w.add_channel(sid, "/video", "protobuf", &BTreeMap::new())?;
        self.video = Some((cid, 0));
        Ok(cid)
    }

    fn image_id<W: std::io::Write + std::io::Seek>(&mut self, w: &mut Writer<W>) -> Result<u16> {
        if let Some((id, _)) = self.image { return Ok(id); }
        let sid = w.add_schema("foxglove.CompressedImage", "protobuf", FOXGLOVE_SCHEMA)?;
        let cid = w.add_channel(sid, "/image", "protobuf", &BTreeMap::new())?;
        self.image = Some((cid, 0));
        Ok(cid)
    }

    fn write_video<W: std::io::Write + std::io::Seek>(
        &mut self, w: &mut Writer<W>, ts: u64, payload: &[u8],
    ) -> Result<()> {
        let channel_id = self.video_id(w)?;
        let (_, seq) = self.video.as_mut().unwrap();
        let s = *seq; *seq += 1;
        w.write_to_known_channel(
            &MessageHeader { channel_id, sequence: s, log_time: ts, publish_time: ts },
            payload,
        )?;
        Ok(())
    }

    fn write_image<W: std::io::Write + std::io::Seek>(
        &mut self, w: &mut Writer<W>, ts: u64, payload: &[u8],
    ) -> Result<()> {
        let channel_id = self.image_id(w)?;
        let (_, seq) = self.image.as_mut().unwrap();
        let s = *seq; *seq += 1;
        w.write_to_known_channel(
            &MessageHeader { channel_id, sequence: s, log_time: ts, publish_time: ts },
            payload,
        )?;
        Ok(())
    }

    fn write_camera_info<W: std::io::Write + std::io::Seek>(
        &mut self, w: &mut Writer<W>, ts: u64, width: u32, height: u32,
    ) -> Result<()> {
        if self.camera_info.is_some() { return Ok(()); }
        let sid = w.add_schema("foxglove.CameraCalibration", "jsonschema", CAMERA_CALIBRATION_SCHEMA.as_bytes())?;
        let cid = w.add_channel(sid, "/camera/camera_info", "json", &BTreeMap::new())?;
        self.camera_info = Some(cid);

        // Mini 4 Pro intrinsics: 82.1° HFOV at the recorded resolution.
        let fx = (width as f64 / 2.0) / (41.05_f64.to_radians().tan());
        let cx = width  as f64 / 2.0;
        let cy = height as f64 / 2.0;
        let payload = serde_json::to_vec(&serde_json::json!({
            "timestamp": { "sec": ts / 1_000_000_000, "nsec": ts % 1_000_000_000 },
            "frame_id": "camera",
            "width": width, "height": height,
            "distortion_model": "plumb_bob",
            "D": [0.0, 0.0, 0.0, 0.0, 0.0],
            "K": [fx, 0.0, cx, 0.0, fx, cy, 0.0, 0.0, 1.0],
            "P": [fx, 0.0, cx, 0.0, 0.0, fx, cy, 0.0, 0.0, 0.0, 1.0, 0.0],
            "R": [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
        }))?;
        w.write_to_known_channel(
            &MessageHeader { channel_id: cid, sequence: 0, log_time: ts, publish_time: ts },
            &payload,
        )?;
        Ok(())
    }
}

// ── Timestamp utilities ───────────────────────────────────────────────────────

/// Read the `creation_time` metadata tag from an MP4 (stored as UTC by FFmpeg).
fn mp4_creation_time_ns(path: &Path) -> Result<u64> {
    let ctx = ffmpeg::format::input(path)?;
    let raw_owned = ctx.metadata().get("creation_time").context("no creation_time tag")?.to_string();
    let raw = raw_owned.trim_end_matches('Z');
    let base = raw.split_once('.').map(|(b, _)| b).unwrap_or(raw);
    let dt = chrono::NaiveDateTime::parse_from_str(base, "%Y-%m-%dT%H:%M:%S")
        .context("bad creation_time format")?;
    let utc = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc);
    Ok(utc.timestamp_nanos_opt().context("timestamp out of range")? as u64)
}

/// Parse the local timestamp from a DJI filename (DJI_YYYYMMDDHHMMSS_…).
/// Returns nanoseconds treating the filename as if it were UTC (caller applies offset).
fn dji_filename_ns_as_utc(path: &Path) -> Option<u64> {
    let name = path.file_name()?.to_str()?;
    let after = name.strip_prefix("DJI_")?;
    let ts = after.get(..14)?;
    let dt = chrono::NaiveDateTime::parse_from_str(ts, "%Y%m%d%H%M%S").ok()?;
    let fake_utc = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc);
    Some(fake_utc.timestamp_nanos_opt()? as u64)
}

// ── Media-file discovery ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum MediaFile {
    Video { path: PathBuf, start_ns: u64, duration_ns: u64 },
    Image { path: PathBuf, timestamp_ns: u64 },
}

impl MediaFile {
    fn window_start(&self) -> u64 {
        match self {
            MediaFile::Video { start_ns, .. }      => *start_ns,
            MediaFile::Image { timestamp_ns, .. }  => *timestamp_ns,
        }
    }
}

/// Scan `dir` for MP4 and JPEG files whose time window overlaps the flight.
pub fn find_media(dir: &Path, flight_start_ns: u64, flight_end_ns: u64) -> Result<Vec<MediaFile>> {
    let entries: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("cannot read {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();

    // Derive UTC offset from the first MP4 whose creation_time we can read.
    // DJI filenames carry local time; creation_time in the container is UTC.
    let mut utc_offset_ns: i64 = 0;
    for p in &entries {
        let ext = ext_upper(p);
        if ext != "MP4" && ext != "MOV" { continue; }
        if let (Ok(utc), Some(local)) = (mp4_creation_time_ns(p), dji_filename_ns_as_utc(p)) {
            utc_offset_ns = local as i64 - utc as i64;
            eprintln!("  UTC offset derived from {}: {:+} s",
                p.file_name().unwrap().to_string_lossy(), utc_offset_ns / 1_000_000_000);
            break;
        }
    }

    let pad_start = flight_start_ns.saturating_sub(MATCH_PADDING_NS);
    let pad_end   = flight_end_ns.saturating_add(MATCH_PADDING_NS);
    let mut files: Vec<MediaFile> = Vec::new();

    for path in &entries {
        match ext_upper(path).as_str() {
            "MP4" | "MOV" => {
                let start_ns = match mp4_creation_time_ns(path) {
                    Ok(t)  => t,
                    Err(e) => { eprintln!("  skip {}: {e}", path.file_name().unwrap().to_string_lossy()); continue; }
                };
                let duration_ns = {
                    let ctx = ffmpeg::format::input(path)?;
                    (ctx.duration().max(0) as u64) * 1_000
                };
                if start_ns < pad_end && start_ns.saturating_add(duration_ns) > pad_start {
                    files.push(MediaFile::Video { path: path.clone(), start_ns, duration_ns });
                }
            }
            "JPG" | "JPEG" | "PNG" => {
                let local = match dji_filename_ns_as_utc(path) { Some(v) => v, None => continue };
                let ts_ns = (local as i64 - utc_offset_ns).max(0) as u64;
                if ts_ns >= pad_start && ts_ns <= pad_end {
                    files.push(MediaFile::Image { path: path.clone(), timestamp_ns: ts_ns });
                }
            }
            _ => {}
        }
    }

    files.sort_by_key(|f| f.window_start());
    Ok(files)
}

fn ext_upper(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_uppercase()
}

// ── Annex-B BSF wrapper ───────────────────────────────────────────────────────
//
// Converts H.264/H.265 MP4 packets (length-prefixed NAL units, SPS/PPS in
// extradata) to Annex-B (start-code NAL units with SPS/PPS inlined before IDR).
// Without this Foxglove shows "waiting for keyframe".

struct AnnexBFilter {
    ctx:     *mut AVBSFContext,
    out_pkt: *mut ffsys::AVPacket,
}

impl AnnexBFilter {
    unsafe fn new(
        codec_id: ffsys::AVCodecID,
        codecpar: *mut ffsys::AVCodecParameters,
        time_base: ffsys::AVRational,
    ) -> Result<Self> {
        let bsf_name = CString::new(match codec_id {
            ffsys::AVCodecID::AV_CODEC_ID_HEVC => "hevc_mp4toannexb",
            ffsys::AVCodecID::AV_CODEC_ID_H264 => "h264_mp4toannexb",
            _ => bail!("no mp4toannexb BSF for codec {:?}", codec_id),
        })?;

        let bsf = av_bsf_get_by_name(bsf_name.as_ptr());
        if bsf.is_null() { bail!("BSF {} not found", bsf_name.to_str().unwrap()); }

        let mut ctx: *mut AVBSFContext = ptr::null_mut();
        if av_bsf_alloc(bsf, &mut ctx) < 0 { bail!("av_bsf_alloc failed"); }

        if ffsys::avcodec_parameters_copy((*ctx).par_in, codecpar) < 0 {
            av_bsf_free(&mut ctx);
            bail!("avcodec_parameters_copy to BSF failed");
        }
        (*ctx).time_base_in = time_base;

        if av_bsf_init(ctx) < 0 {
            av_bsf_free(&mut ctx);
            bail!("av_bsf_init failed");
        }

        let out_pkt = ffsys::av_packet_alloc();
        if out_pkt.is_null() { av_bsf_free(&mut ctx); bail!("av_packet_alloc failed"); }

        Ok(Self { ctx, out_pkt })
    }

    unsafe fn send(&mut self, pkt: *mut ffsys::AVPacket) -> c_int {
        av_bsf_send_packet(self.ctx, pkt)
    }
    unsafe fn receive(&mut self) -> Option<*mut ffsys::AVPacket> {
        if av_bsf_receive_packet(self.ctx, self.out_pkt) >= 0 { Some(self.out_pkt) } else { None }
    }
}

impl Drop for AnnexBFilter {
    fn drop(&mut self) {
        unsafe {
            ffsys::av_packet_free(&mut self.out_pkt);
            av_bsf_free(&mut self.ctx);
        }
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

fn video_dims(path: &Path) -> Option<(u32, u32)> {
    let input = ffmpeg::format::input(path).ok()?;
    let stream = input.streams().best(ffmpeg::media::Type::Video)?;
    let mut params = stream.parameters();
    let (w, h) = unsafe {
        let p = params.as_mut_ptr();
        ((*p).width as u32, (*p).height as u32)
    };
    if w > 0 && h > 0 { Some((w, h)) } else { None }
}

/// Write `files` into `writer`. `scale_width` triggers H.264 transcoding.
/// `video_offset_ns` shifts all timestamps (positive = later).
pub fn write_media<W: std::io::Write + std::io::Seek>(
    files: &[MediaFile],
    writer: &mut Writer<W>,
    scale_width: Option<u32>,
    video_offset_ns: i64,
) -> Result<()> {
    let mut ch = Channels::new();
    for file in files {
        match file {
            MediaFile::Video { path, start_ns, .. } => {
                let adjusted_start = (*start_ns as i64 + video_offset_ns).max(0) as u64;
                if let Some((w, h)) = video_dims(path) {
                    ch.write_camera_info(writer, adjusted_start, w, h)?;
                }
                let result = if scale_width.is_some() {
                    write_video_transcoded(path, adjusted_start, scale_width.unwrap(), writer, &mut ch)
                } else {
                    write_video_passthrough(path, adjusted_start, writer, &mut ch)
                };
                match result {
                    Ok(n)  => eprintln!("  {} → {} video packets", fname(path), n),
                    Err(e) => eprintln!("  {} error: {e}", path.display()),
                }
            }
            MediaFile::Image { path, timestamp_ns } => {
                let adjusted_ts = (*timestamp_ns as i64 + video_offset_ns).max(0) as u64;
                match write_image(path, adjusted_ts, writer, &mut ch) {
                    Ok(())  => eprintln!("  {} → image", fname(path)),
                    Err(e)  => eprintln!("  {} error: {e}", path.display()),
                }
            }
        }
    }
    Ok(())
}

// ── Passthrough (demux + BSF) ─────────────────────────────────────────────────

fn write_video_passthrough<W: std::io::Write + std::io::Seek>(
    path: &Path,
    start_ns: u64,
    writer: &mut Writer<W>,
    ch: &mut Channels,
) -> Result<u32> {
    let mut input = ffmpeg::format::input(path)?;

    let video_idx = input.streams().best(ffmpeg::media::Type::Video)
        .context("no video stream")?.index();
    let stream = input.stream(video_idx).unwrap();
    let tb = stream.time_base();
    let av_tb = ffsys::AVRational { num: tb.numerator(), den: tb.denominator() };

    let mut params = stream.parameters();
    let codec_id_hi = params.id();
    let codecpar = unsafe { params.as_mut_ptr() };

    let (format_str, bsf_codec_id) = match codec_id_hi {
        ffmpeg::codec::Id::HEVC | ffmpeg::codec::Id::H265 => ("h265", ffsys::AVCodecID::AV_CODEC_ID_HEVC),
        ffmpeg::codec::Id::H264 => ("h264", ffsys::AVCodecID::AV_CODEC_ID_H264),
        other => bail!("unsupported codec {:?}", other),
    };

    let mut bsf = unsafe { AnnexBFilter::new(bsf_codec_id, codecpar, av_tb)? };
    let mut count = 0u32;

    unsafe {
        let pkt = ffsys::av_packet_alloc();
        if pkt.is_null() { bail!("av_packet_alloc failed"); }

        loop {
            let ret = ffsys::av_read_frame(input.as_mut_ptr(), pkt);
            if ret < 0 { break; }
            if (*pkt).stream_index != video_idx as c_int {
                ffsys::av_packet_unref(pkt);
                continue;
            }
            bsf.send(pkt);
            ffsys::av_packet_unref(pkt);
            while let Some(out) = bsf.receive() {
                if (*out).pts >= 0 {
                    let abs_ns = start_ns.saturating_add(pts_to_ns((*out).pts, av_tb));
                    let data = std::slice::from_raw_parts((*out).data, (*out).size as usize);
                    ch.write_video(writer, abs_ns, &encode_video(abs_ns, "camera", data, format_str))?;
                    count += 1;
                }
                ffsys::av_packet_unref(out);
            }
        }

        bsf.send(ptr::null_mut());
        while let Some(out) = bsf.receive() {
            if (*out).pts >= 0 {
                let abs_ns = start_ns.saturating_add(pts_to_ns((*out).pts, av_tb));
                let data = std::slice::from_raw_parts((*out).data, (*out).size as usize);
                ch.write_video(writer, abs_ns, &encode_video(abs_ns, "camera", data, format_str))?;
                count += 1;
            }
            ffsys::av_packet_unref(out);
        }

        ffsys::av_packet_free(&mut { pkt });
    }

    Ok(count)
}

// ── Transcode (decode → scale → H.264 encode) ────────────────────────────────

fn write_video_transcoded<W: std::io::Write + std::io::Seek>(
    path: &Path,
    start_ns: u64,
    target_width: u32,
    writer: &mut Writer<W>,
    ch: &mut Channels,
) -> Result<u32> {
    use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
    use ffmpeg::util::format::pixel::Pixel;

    let mut input = ffmpeg::format::input(path)?;
    let video_idx = input.streams().best(ffmpeg::media::Type::Video)
        .context("no video stream")?.index();
    let stream = input.stream(video_idx).unwrap();
    let stream_tb = stream.time_base();
    let frame_rate = stream.avg_frame_rate();

    let dec_ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
    let mut decoder = dec_ctx.decoder().video()?;

    let src_w = decoder.width();
    let src_h = decoder.height();
    let src_fmt = decoder.format();
    let dst_w = (target_width & !1).max(2);
    let dst_h = ((src_h as u64 * dst_w as u64 / src_w as u64) as u32 & !1).max(2);

    eprintln!("  transcoding {}x{} → {}x{} H.264", src_w, src_h, dst_w, dst_h);

    let mut scaler = Scaler::get(src_fmt, src_w, src_h, Pixel::YUV420P, dst_w, dst_h, Flags::BILINEAR)?;

    let h264 = ffmpeg::codec::encoder::find(ffmpeg::codec::Id::H264)
        .context("H.264 encoder not found")?;
    let enc_ctx = ffmpeg::codec::context::Context::new_with_codec(h264);
    let mut encoder = enc_ctx.encoder().video()?;
    encoder.set_width(dst_w);
    encoder.set_height(dst_h);
    encoder.set_format(Pixel::YUV420P);
    encoder.set_time_base(ffmpeg::util::rational::Rational(1, 90000));
    encoder.set_frame_rate(Some(frame_rate));

    let mut opts = ffmpeg::Dictionary::new();
    opts.set("preset", "veryfast");
    opts.set("crf", "23");
    let mut encoder = encoder.open_with(opts)?;

    let src_av_tb = ffsys::AVRational { num: stream_tb.numerator(), den: stream_tb.denominator() };
    let dst_av_tb = ffsys::AVRational { num: 1, den: 90000 };

    let mut count = 0u32;
    let mut src_frame = ffmpeg::util::frame::video::Video::empty();
    let mut dst_frame = ffmpeg::util::frame::video::Video::empty();
    let mut enc_pkt   = ffmpeg::Packet::empty();

    let drain = |encoder: &mut ffmpeg::encoder::video::Video,
                 enc_pkt: &mut ffmpeg::Packet,
                 ch: &mut Channels,
                 writer: &mut Writer<W>,
                 count: &mut u32| -> Result<()> {
        while encoder.receive_packet(enc_pkt).is_ok() {
            let abs_ns = start_ns.saturating_add(pts_to_ns(enc_pkt.pts().unwrap_or(0), dst_av_tb));
            ch.write_video(writer, abs_ns, &encode_video(abs_ns, "camera", enc_pkt.data().unwrap_or(&[]), "h264"))?;
            *count += 1;
        }
        Ok(())
    };

    for (stream, packet) in input.packets() {
        if stream.index() != video_idx { continue; }
        decoder.send_packet(&packet)?;
        while decoder.receive_frame(&mut src_frame).is_ok() {
            scaler.run(&src_frame, &mut dst_frame)?;
            let new_pts = unsafe { ffsys::av_rescale_q(src_frame.pts().unwrap_or(0), src_av_tb, dst_av_tb) };
            dst_frame.set_pts(Some(new_pts));
            dst_frame.set_kind(ffmpeg::util::picture::Type::None);
            encoder.send_frame(&dst_frame)?;
            drain(&mut encoder, &mut enc_pkt, ch, writer, &mut count)?;
        }
    }

    decoder.send_eof()?;
    while decoder.receive_frame(&mut src_frame).is_ok() {
        scaler.run(&src_frame, &mut dst_frame)?;
        let new_pts = unsafe { ffsys::av_rescale_q(src_frame.pts().unwrap_or(0), src_av_tb, dst_av_tb) };
        dst_frame.set_pts(Some(new_pts));
        encoder.send_frame(&dst_frame)?;
        drain(&mut encoder, &mut enc_pkt, ch, writer, &mut count)?;
    }
    encoder.send_eof()?;
    drain(&mut encoder, &mut enc_pkt, ch, writer, &mut count)?;

    Ok(count)
}

// ── Image writing ─────────────────────────────────────────────────────────────

fn write_image<W: std::io::Write + std::io::Seek>(
    path: &Path, timestamp_ns: u64, writer: &mut Writer<W>, ch: &mut Channels,
) -> Result<()> {
    let data = fs::read(path)?;
    let fmt = if ext_upper(path) == "PNG" { "png" } else { "jpeg" };
    ch.write_image(writer, timestamp_ns, &encode_image(timestamp_ns, "camera", &data, fmt))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn pts_to_ns(pts: i64, tb: ffsys::AVRational) -> u64 {
    if pts <= 0 || tb.den == 0 { return 0; }
    (pts as u64).saturating_mul(tb.num as u64).saturating_mul(1_000_000_000) / tb.den as u64
}

fn fname(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}
