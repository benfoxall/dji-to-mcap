use anyhow::Result;
use dji_log_parser::frame::Frame;
use serde::Serialize;

use super::{TransformedMessage, Transformer};

// Generic open schemas — Foxglove renders all JSON fields from any object schema.
const OSD_SCHEMA: &str = r#"{"type":"object","title":"dji.OSD","description":"DJI on-screen display: position, attitude, speed, GPS status"}"#;
const GIMBAL_SCHEMA: &str = r#"{"type":"object","title":"dji.Gimbal","description":"Gimbal pitch/roll/yaw angles and limit flags"}"#;
const BATTERY_SCHEMA: &str = r#"{"type":"object","title":"dji.Battery","description":"Battery voltage, current, charge level and cell voltages"}"#;
const RC_SCHEMA: &str = r#"{"type":"object","title":"dji.RC","description":"Remote control stick inputs and signal strength"}"#;
const HOME_SCHEMA: &str = r#"{"type":"object","title":"dji.Home","description":"Home point location and safety parameters"}"#;

pub struct RawTransformer;

impl RawTransformer {
    pub fn new() -> Self {
        Self
    }
}

fn make_msg<T: Serialize>(
    topic: &str,
    schema_name: &str,
    schema: &str,
    data: &T,
) -> Result<TransformedMessage> {
    Ok(TransformedMessage {
        topic: topic.to_string(),
        schema_name: schema_name.to_string(),
        schema_encoding: "jsonschema".to_string(),
        schema_data: schema.as_bytes().to_vec(),
        payload: serde_json::to_vec(data)?,
    })
}

impl Transformer for RawTransformer {
    fn transform(&mut self, frame: &Frame, _timestamp_ns: u64) -> Result<Vec<TransformedMessage>> {
        Ok(vec![
            make_msg("/dji/osd", "dji.OSD", OSD_SCHEMA, &frame.osd)?,
            make_msg("/dji/gimbal", "dji.Gimbal", GIMBAL_SCHEMA, &frame.gimbal)?,
            make_msg("/dji/battery", "dji.Battery", BATTERY_SCHEMA, &frame.battery)?,
            make_msg("/dji/rc", "dji.RC", RC_SCHEMA, &frame.rc)?,
            make_msg("/dji/home", "dji.Home", HOME_SCHEMA, &frame.home)?,
        ])
    }
}
