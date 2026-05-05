use anyhow::Result;
use dji_log_parser::frame::Frame;
use serde_json::json;

use super::{TransformedMessage, Transformer};
use super::foxglove::euler_to_quat;
use crate::schemas::{FRAME_TRANSFORM_SCHEMA, JOINT_STATE_SCHEMA};

pub struct JointStateTransformer;

impl JointStateTransformer {
    pub fn new() -> Self { Self }
}

impl Transformer for JointStateTransformer {
    fn transform(&mut self, frame: &Frame, timestamp_ns: u64) -> Result<Vec<TransformedMessage>> {
        let sec  = timestamp_ns / 1_000_000_000;
        let nsec = timestamp_ns % 1_000_000_000;

        // JointState drives the URDF visual model. Simple Euler subtraction is
        // sufficient for the mesh animation; the FrameTransform below uses proper
        // quaternion composition for the accurate TF tree position.
        let pitch_rad = -((frame.gimbal.pitch - frame.osd.pitch) as f64).to_radians();
        let roll_rad  = -((frame.gimbal.roll  - frame.osd.roll)  as f64).to_radians();
        let yaw_diff  =   (frame.gimbal.yaw   - frame.osd.yaw)   as f64;
        let yaw_rad = {
            let r = yaw_diff.to_radians().rem_euclid(std::f64::consts::TAU);
            let r = if r > std::f64::consts::PI { r - std::f64::consts::TAU } else { r };
            -r
        };

        let joint_state = serde_json::to_vec(&json!({
            "header": { "stamp": { "sec": sec, "nsec": nsec }, "frame_id": "" },
            "name":     ["gimbal_pitch", "gimbal_roll", "gimbal_yaw"],
            "position": [pitch_rad, roll_rad, yaw_rad],
            "velocity": [0.0, 0.0, 0.0],
            "effort":   [0.0, 0.0, 0.0]
        }))?;

        // Gimbal FrameTransform: q_rel = conj(q_drone_enu) * q_gimbal_enu.
        // Quaternion composition correctly handles cross-axis coupling when the
        // drone pitches or rolls during a yaw sweep (Euler subtraction does not).
        let q_drone  = euler_to_quat(frame.osd.roll as f64,    frame.osd.pitch as f64,    frame.osd.yaw as f64);
        let q_gimbal = euler_to_quat(frame.gimbal.roll as f64, frame.gimbal.pitch as f64, frame.gimbal.yaw as f64);
        let (dx, dy, dz, dw) = q_drone;
        let (gx, gy, gz, gw) = q_gimbal;
        let (qx, qy, qz, qw) = (
            dw*gx - dx*gw - dy*gz + dz*gy,
            dw*gy + dx*gz - dy*gw - dz*gx,
            dw*gz - dx*gy + dy*gx - dz*gw,
            dw*gw + dx*gx + dy*gy + dz*gz,
        );

        let gimbal_tf = serde_json::to_vec(&json!({
            "timestamp": { "sec": sec, "nsec": nsec },
            "parent_frame_id": "base_link",
            "child_frame_id":  "gimbal_link",
            // Summed URDF translations along gimbal chain (m)
            "translation": { "x": 0.066, "y": 0.0, "z": -0.056 },
            "rotation":    { "x": qx, "y": qy, "z": qz, "w": qw }
        }))?;

        // Static optical-frame offset: gimbal_link (FLU: X=fwd, Y=left, Z=up) →
        // camera (optical: X=right, Y=down, Z=fwd).
        // R = [[0,0,1],[-1,0,0],[0,-1,0]] → q = (w=0.5, x=-0.5, y=0.5, z=-0.5)
        let camera_tf = serde_json::to_vec(&json!({
            "timestamp": { "sec": sec, "nsec": nsec },
            "parent_frame_id": "gimbal_link",
            "child_frame_id":  "camera",
            "translation": { "x": 0.0, "y": 0.0, "z": 0.0 },
            "rotation":    { "x": -0.5, "y": 0.5, "z": -0.5, "w": 0.5 }
        }))?;

        Ok(vec![
            TransformedMessage {
                topic: "/joint_states".to_string(),
                schema_name: "sensor_msgs/JointState".to_string(),
                schema_encoding: "jsonschema".to_string(),
                schema_data: JOINT_STATE_SCHEMA.as_bytes().to_vec(),
                payload: joint_state,
            },
            TransformedMessage {
                topic: "/foxglove/gimbal/tf".to_string(),
                schema_name: "foxglove.FrameTransform".to_string(),
                schema_encoding: "jsonschema".to_string(),
                schema_data: FRAME_TRANSFORM_SCHEMA.as_bytes().to_vec(),
                payload: gimbal_tf,
            },
            TransformedMessage {
                topic: "/foxglove/camera/tf".to_string(),
                schema_name: "foxglove.FrameTransform".to_string(),
                schema_encoding: "jsonschema".to_string(),
                schema_data: FRAME_TRANSFORM_SCHEMA.as_bytes().to_vec(),
                payload: camera_tf,
            },
        ])
    }
}
