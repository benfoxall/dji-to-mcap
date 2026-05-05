use anyhow::Result;
use dji_log_parser::frame::Frame;
use serde::Serialize;

use super::{TransformedMessage, Transformer};
use crate::schemas::{BATTERY_SCHEMA, GIMBAL_SCHEMA, HOME_SCHEMA, OSD_SCHEMA, RC_SCHEMA};

pub struct RawTransformer;

impl RawTransformer {
    pub fn new() -> Self { Self }
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
            make_msg("/dji/osd",     "dji.OSD",     OSD_SCHEMA,     &frame.osd)?,
            make_msg("/dji/gimbal",  "dji.Gimbal",  GIMBAL_SCHEMA,  &frame.gimbal)?,
            make_msg("/dji/battery", "dji.Battery", BATTERY_SCHEMA, &frame.battery)?,
            make_msg("/dji/rc",      "dji.RC",      RC_SCHEMA,      &frame.rc)?,
            make_msg("/dji/home",    "dji.Home",    HOME_SCHEMA,    &frame.home)?,
        ])
    }
}
