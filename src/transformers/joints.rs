use anyhow::Result;
use dji_log_parser::frame::Frame;
use serde_json::json;

use super::{TransformedMessage, Transformer};

pub const JOINT_STATE_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "header": {
      "type": "object",
      "properties": {
        "stamp": {
          "type": "object",
          "properties": {
            "sec": { "type": "integer" },
            "nsec": { "type": "integer" }
          }
        },
        "frame_id": { "type": "string" }
      }
    },
    "name":     { "type": "array", "items": { "type": "string" } },
    "position": { "type": "array", "items": { "type": "number" } },
    "velocity": { "type": "array", "items": { "type": "number" } },
    "effort":   { "type": "array", "items": { "type": "number" } }
  }
}"#;

pub struct JointStateTransformer;

impl JointStateTransformer {
    pub fn new() -> Self {
        Self
    }
}

impl Transformer for JointStateTransformer {
    fn transform(&mut self, frame: &Frame, timestamp_ns: u64) -> Result<Vec<TransformedMessage>> {
        let sec = timestamp_ns / 1_000_000_000;
        let nsec = timestamp_ns % 1_000_000_000;

        // DJI gimbal angles are absolute (world-frame) compass angles.
        // URDF joints live in the drone body frame, so subtract the drone's
        // own attitude to get the camera angle relative to the airframe.
        // Wrap yaw difference into [-π, π] to handle compass wrap-around.
        let pitch_rad = ((frame.gimbal.pitch - frame.osd.pitch) as f64).to_radians();
        let roll_rad = ((frame.gimbal.roll - frame.osd.roll) as f64).to_radians();
        let yaw_diff = (frame.gimbal.yaw - frame.osd.yaw) as f64;
        let yaw_rad = yaw_diff.to_radians().rem_euclid(std::f64::consts::TAU);
        let yaw_rad = if yaw_rad > std::f64::consts::PI { yaw_rad - std::f64::consts::TAU } else { yaw_rad };

        let payload = serde_json::to_vec(&json!({
            "header": {
                "stamp": { "sec": sec, "nsec": nsec },
                "frame_id": ""
            },
            // Names must match joint names declared in mini4pro.urdf
            "name":     ["gimbal_pitch", "gimbal_roll", "gimbal_yaw"],
            "position": [pitch_rad, roll_rad, yaw_rad],
            "velocity": [0.0, 0.0, 0.0],
            "effort":   [0.0, 0.0, 0.0]
        }))?;

        Ok(vec![TransformedMessage {
            topic: "/joint_states".to_string(),
            schema_name: "sensor_msgs/JointState".to_string(),
            schema_encoding: "jsonschema".to_string(),
            schema_data: JOINT_STATE_SCHEMA.as_bytes().to_vec(),
            payload,
        }])
    }
}
