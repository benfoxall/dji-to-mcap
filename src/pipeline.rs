use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::PathBuf,
};

use anyhow::{Context, Result};
use dji_log_parser::DJILog;
use mcap::{records::MessageHeader, Writer};

use crate::transformers::{foxglove::FoxgloveFusedTransformer, raw::RawTransformer, Transformer};

struct Channel {
    channel_id: u16,
    sequence: u32,
}

pub fn process(input: PathBuf, output: PathBuf, api_key: Option<String>) -> Result<()> {
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

    eprintln!(
        "Parsed {} frames from {}",
        frames.len(),
        input.display()
    );

    if frames.is_empty() {
        anyhow::bail!("No frames decoded — the log may be encrypted. Provide a DJI API key.");
    }

    let out_file =
        fs::File::create(&output).with_context(|| format!("Cannot create {}", output.display()))?;
    let mut writer = Writer::new(out_file)?;

    // Keyed by (topic, schema_name) to match arducap pattern
    let mut channel_map: HashMap<(String, String), Channel> = HashMap::new();

    let mut transformers: Vec<Box<dyn Transformer>> = vec![
        Box::new(RawTransformer::new()),
        Box::new(FoxgloveFusedTransformer::new()),
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
                    let schema_id =
                        writer.add_schema(&msg.schema_name, &msg.schema_encoding, &msg.schema_data)?;
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

    writer.finish()?;
    eprintln!("Written to {}", output.display());
    Ok(())
}
