use anyhow::Result;
use dji_log_parser::frame::Frame;
use serde_json::json;

use super::{TransformedMessage, Transformer};

// Foxglove-standard schemas (same structure as arducap)
const LOCATION_FIX_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "latitude": { "type": "number" },
    "longitude": { "type": "number" },
    "altitude": { "type": "number" },
    "frame_id": { "type": "string" },
    "position_covariance_type": { "type": "integer" },
    "position_covariance": { "type": "array", "items": { "type": "number" } }
  }
}"#;

const FRAME_TRANSFORM_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "timestamp": {
      "type": "object",
      "properties": { "sec": { "type": "integer" }, "nsec": { "type": "integer" } }
    },
    "parent_frame_id": { "type": "string" },
    "child_frame_id": { "type": "string" },
    "translation": {
      "type": "object",
      "properties": { "x": {"type":"number"}, "y": {"type":"number"}, "z": {"type":"number"} }
    },
    "rotation": {
      "type": "object",
      "properties": { "x": {"type":"number"}, "y": {"type":"number"}, "z": {"type":"number"}, "w": {"type":"number"} }
    }
  }
}"#;

pub struct FoxgloveFusedTransformer {
    home: Option<(f64, f64, f64)>,
}

impl FoxgloveFusedTransformer {
    pub fn new() -> Self {
        Self { home: None }
    }
}

/// Convert Euler angles (degrees, NED frame) to a quaternion in ENU frame.
///
/// DJI uses the NED (North-East-Down) convention. Foxglove/ROS uses ENU
/// (East-North-Up). The conversion negates the Y and Z quaternion components.
pub fn euler_to_quat(roll_deg: f64, pitch_deg: f64, yaw_deg: f64) -> (f64, f64, f64, f64) {
    let r = roll_deg.to_radians();
    let p = pitch_deg.to_radians();
    let y = yaw_deg.to_radians();

    // NED quaternion using the aerospace Z-Y-X (yaw-pitch-roll) sequence
    let cy = (y * 0.5).cos();
    let sy = (y * 0.5).sin();
    let cp = (p * 0.5).cos();
    let sp = (p * 0.5).sin();
    let cr = (r * 0.5).cos();
    let sr = (r * 0.5).sin();

    let qw = cr * cp * cy + sr * sp * sy;
    let qx = sr * cp * cy - cr * sp * sy;
    let qy = cr * sp * cy + sr * cp * sy;
    let qz = cr * cp * sy - sr * sp * cy;

    // NED → ENU: rotate 180° around X → negate Y and Z components
    (qx, -qy, -qz, qw)
}

// WGS-84 ellipsoid constants
const WGS84_A: f64 = 6378137.0;
const WGS84_F: f64 = 1.0 / 298.257223563;
const WGS84_E2: f64 = WGS84_F * (2.0 - WGS84_F);

/// Convert a WGS-84 geodetic position to ENU (East-North-Up) metres relative
/// to a home origin. Accounts for Earth curvature via an ECEF intermediate step.
pub fn wgs84_to_enu(
    lat: f64,
    lon: f64,
    alt: f64,
    home_lat: f64,
    home_lon: f64,
    home_alt: f64,
) -> (f64, f64, f64) {
    let to_ecef = |lat_d: f64, lon_d: f64, alt_m: f64| -> (f64, f64, f64) {
        let lat_r = lat_d.to_radians();
        let lon_r = lon_d.to_radians();
        let n = WGS84_A / (1.0 - WGS84_E2 * lat_r.sin().powi(2)).sqrt();
        (
            (n + alt_m) * lat_r.cos() * lon_r.cos(),
            (n + alt_m) * lat_r.cos() * lon_r.sin(),
            (n * (1.0 - WGS84_E2) + alt_m) * lat_r.sin(),
        )
    };

    let (hx, hy, hz) = to_ecef(home_lat, home_lon, home_alt);
    let (px, py, pz) = to_ecef(lat, lon, alt);

    let dx = px - hx;
    let dy = py - hy;
    let dz = pz - hz;

    let h_lat_r = home_lat.to_radians();
    let h_lon_r = home_lon.to_radians();
    let sin_lat = h_lat_r.sin();
    let cos_lat = h_lat_r.cos();
    let sin_lon = h_lon_r.sin();
    let cos_lon = h_lon_r.cos();

    (
        -sin_lon * dx + cos_lon * dy,                                        // East
        -sin_lat * cos_lon * dx - sin_lat * sin_lon * dy + cos_lat * dz,    // North
        cos_lat * cos_lon * dx + cos_lat * sin_lon * dy + sin_lat * dz,     // Up
    )
}

impl Transformer for FoxgloveFusedTransformer {
    fn transform(&mut self, frame: &Frame, timestamp_ns: u64) -> Result<Vec<TransformedMessage>> {
        let mut output = Vec::new();

        let lat = frame.osd.latitude;
        let lon = frame.osd.longitude;
        let alt = frame.osd.altitude as f64;

        // Skip frames without a GPS fix
        if lat.abs() < 1e-6 && lon.abs() < 1e-6 {
            return Ok(output);
        }

        // Emit the map origin anchor once on the first valid GPS fix
        if self.home.is_none() {
            self.home = Some((lat, lon, alt));

            output.push(TransformedMessage {
                topic: "/foxglove/map_origin".to_string(),
                schema_name: "foxglove.LocationFix".to_string(),
                schema_encoding: "jsonschema".to_string(),
                schema_data: LOCATION_FIX_SCHEMA.as_bytes().to_vec(),
                payload: serde_json::to_vec(&json!({
                    "frame_id": "world",
                    "latitude": lat,
                    "longitude": lon,
                    "altitude": alt
                }))?,
            });
        }

        // GPS trace for the 2D map panel
        output.push(TransformedMessage {
            topic: "/foxglove/gps".to_string(),
            schema_name: "foxglove.LocationFix".to_string(),
            schema_encoding: "jsonschema".to_string(),
            schema_data: LOCATION_FIX_SCHEMA.as_bytes().to_vec(),
            payload: serde_json::to_vec(&json!({
                "frame_id": "base_link",
                "latitude": lat,
                "longitude": lon,
                "altitude": alt
            }))?,
        });

        if let Some((home_lat, home_lon, home_alt)) = self.home {
            let (e, n, u) = wgs84_to_enu(lat, lon, alt, home_lat, home_lon, home_alt);
            let (qx, qy, qz, qw) = euler_to_quat(
                frame.osd.roll as f64,
                frame.osd.pitch as f64,
                frame.osd.yaw as f64,
            );

            let sec = timestamp_ns / 1_000_000_000;
            let nsec = timestamp_ns % 1_000_000_000;

            // Drone body transform: world → base_link
            output.push(TransformedMessage {
                topic: "/foxglove/drone/tf".to_string(),
                schema_name: "foxglove.FrameTransform".to_string(),
                schema_encoding: "jsonschema".to_string(),
                schema_data: FRAME_TRANSFORM_SCHEMA.as_bytes().to_vec(),
                payload: serde_json::to_vec(&json!({
                    "timestamp": { "sec": sec, "nsec": nsec },
                    "parent_frame_id": "world",
                    "child_frame_id": "base_link",
                    "translation": { "x": e, "y": n, "z": u },
                    "rotation": { "x": qx, "y": qy, "z": qz, "w": qw }
                }))?,
            });

            // Gimbal links (gimbal_pitch_link, gimbal_roll_link, gimbal_link) are
            // positioned by the URDF kinematic chain driven by /joint_states.
            // Publishing an explicit TF here would override the URDF joints and
            // make the camera appear locked to the airframe.
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_to_quat_identity() {
        let (x, y, z, w) = euler_to_quat(0.0, 0.0, 0.0);
        assert!((w - 1.0).abs() < 1e-10, "w should be 1.0, got {w}");
        assert!(x.abs() < 1e-10);
        assert!(y.abs() < 1e-10);
        assert!(z.abs() < 1e-10);
    }

    #[test]
    fn euler_to_quat_unit_length() {
        let (x, y, z, w) = euler_to_quat(10.0, -5.0, 45.0);
        let norm = (x * x + y * y + z * z + w * w).sqrt();
        assert!((norm - 1.0).abs() < 1e-10, "quaternion not unit length: {norm}");
    }

    #[test]
    fn wgs84_to_enu_at_home_is_zero() {
        let (e, n, u) = wgs84_to_enu(51.5, -0.1, 100.0, 51.5, -0.1, 100.0);
        assert!(e.abs() < 1e-4, "east should be ~0, got {e}");
        assert!(n.abs() < 1e-4, "north should be ~0, got {n}");
        assert!(u.abs() < 1e-4, "up should be ~0, got {u}");
    }

    #[test]
    fn wgs84_to_enu_one_degree_north() {
        // One degree of latitude ≈ 111_195 m north
        let (e, n, _u) = wgs84_to_enu(52.0, -0.1, 0.0, 51.0, -0.1, 0.0);
        assert!(e.abs() < 1.0, "east offset should be small, got {e}");
        assert!((n - 111_195.0).abs() < 200.0, "north offset off, got {n}");
    }
}
