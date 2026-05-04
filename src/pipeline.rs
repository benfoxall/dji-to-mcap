use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
};

use anyhow::{Context, Result};
use dji_log_parser::DJILog;
use mcap::{records::MessageHeader, Writer};
use serde_json::json;

use crate::transformers::{
    foxglove::FoxgloveFusedTransformer,
    joints::JointStateTransformer,
    raw::RawTransformer,
    Transformer,
};

// Embedded at compile time so djicap carries the URDF without extra files.
const MINI4PRO_URDF: &str = include_str!("../mini4pro.urdf");

const ROBOT_DESC_SCHEMA: &str =
    r#"{"type":"object","properties":{"data":{"type":"string"}}}"#;

struct Channel {
    channel_id: u16,
    sequence: u32,
}

pub fn process(
    input: PathBuf,
    output: PathBuf,
    api_key: Option<String>,
    media_dir: Option<PathBuf>,
    scale_width: Option<u32>,
) -> Result<()> {
    let bytes = fs::read(&input)
        .with_context(|| format!("Failed to read {}", input.display()))?;

    let parser =
        DJILog::from_bytes(bytes).with_context(|| "Failed to parse DJI log header")?;

    eprintln!("Log version: {}", parser.version);

    if parser.version >= 13 && api_key.is_none() {
        eprintln!(
            "Warning: log version {} requires a DJI API key for decryption. \
             Pass --api-key or set DJI_OPEN_API_KEY. Output may be empty.",
            parser.version
        );
    }

    let keychains = match api_key {
        Some(ref key) => Some(
            parser
                .fetch_keychains(key)
                .with_context(|| "Failed to fetch DJI keychains — check your API key")?,
        ),
        None => None,
    };

    let frames = parser.frames(keychains)?;

    eprintln!("Parsed {} frames from {}", frames.len(), input.display());

    if frames.is_empty() {
        anyhow::bail!("No frames decoded — the log may be encrypted. Provide a DJI API key.");
    }

    let out_file =
        fs::File::create(&output).with_context(|| format!("Cannot create {}", output.display()))?;
    let mut writer = Writer::new(out_file)?;

    // Keyed by (topic, schema_name) to match arducap pattern
    let mut channel_map: HashMap<(String, String), Channel> = HashMap::new();

    // Collect valid timestamps to know the flight window.
    let timestamps_ns: Vec<u64> = frames
        .iter()
        .filter_map(|f| f.custom.date_time.timestamp_nanos_opt().map(|ns| ns as u64))
        .filter(|&ns| ns > 0)
        .collect();

    let first_ts_ns = timestamps_ns.iter().copied().min();
    let last_ts_ns  = timestamps_ns.iter().copied().max();

    // Publish the robot URDF description on /robot_description at the first
    // valid timestamp. Foxglove's 3D panel auto-loads it from this topic.
    if let Some(ts) = first_ts_ns {
        let schema_id =
            writer.add_schema("std_msgs/String", "jsonschema", ROBOT_DESC_SCHEMA.as_bytes())?;
        let channel_id =
            writer.add_channel(schema_id, "/robot_description", "json", &BTreeMap::new())?;
        let payload = serde_json::to_vec(&json!({ "data": MINI4PRO_URDF }))?;
        writer.write_to_known_channel(
            &MessageHeader { channel_id, sequence: 0, log_time: ts, publish_time: ts },
            &payload,
        )?;
    }

    let mut transformers: Vec<Box<dyn Transformer>> = vec![
        Box::new(RawTransformer::new()),
        Box::new(FoxgloveFusedTransformer::new()),
        Box::new(JointStateTransformer::new()),
    ];

    for frame in &frames {
        let ts_ns = match frame.custom.date_time.timestamp_nanos_opt() {
            Some(ns) if ns > 0 => ns as u64,
            _ => continue,
        };

        for transformer in &mut transformers {
            let messages = transformer.transform(frame, ts_ns)?;

            for msg in messages {
                let key = (msg.topic.clone(), msg.schema_name.clone());

                if !channel_map.contains_key(&key) {
                    let schema_id = writer.add_schema(
                        &msg.schema_name,
                        &msg.schema_encoding,
                        &msg.schema_data,
                    )?;
                    let channel_id =
                        writer.add_channel(schema_id, &msg.topic, "json", &BTreeMap::new())?;
                    channel_map.insert(key.clone(), Channel { channel_id, sequence: 0 });
                }

                let ch = channel_map.get_mut(&key).unwrap();
                writer.write_to_known_channel(
                    &MessageHeader {
                        channel_id: ch.channel_id,
                        sequence: ch.sequence,
                        log_time: ts_ns,
                        publish_time: ts_ns,
                    },
                    &msg.payload,
                )?;
                ch.sequence += 1;
            }
        }
    }

    // Write matching video and images after telemetry (same MCAP, shared timeline).
    #[cfg(feature = "video")]
    if let Some(ref dir) = media_dir {
        if let (Some(start), Some(end)) = (first_ts_ns, last_ts_ns) {
            eprintln!("Scanning {} for media matching flight window…", dir.display());
            let files = crate::video::find_media(dir, start, end)?;
            if files.is_empty() {
                eprintln!("  No matching media found.");
            } else {
                eprintln!("  Found {} file(s):", files.len());
                crate::video::write_media(&files, &mut writer, scale_width)?;
            }
        }
    }

    #[cfg(not(feature = "video"))]
    if media_dir.is_some() {
        eprintln!("Warning: djicap was built without the 'video' feature; --media is ignored.");
    }

    writer.finish()?;
    eprintln!("Written to {}", output.display());
    Ok(())
}
