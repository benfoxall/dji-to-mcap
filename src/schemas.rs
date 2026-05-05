// ── Foxglove standard schemas ────────────────────────────────────────────────

pub const ROBOT_DESCRIPTION_SCHEMA: &str =
    r#"{"type":"object","properties":{"data":{"type":"string"}}}"#;

pub const LOCATION_FIX_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "timestamp":               { "type": "object", "properties": { "sec": { "type": "integer" }, "nsec": { "type": "integer" } } },
    "frame_id":                { "type": "string" },
    "latitude":                { "type": "number" },
    "longitude":               { "type": "number" },
    "altitude":                { "type": "number" },
    "position_covariance_type":{ "type": "integer" },
    "position_covariance":     { "type": "array", "items": { "type": "number" } }
  }
}"#;

pub const FRAME_TRANSFORM_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "timestamp":       { "type": "object", "properties": { "sec": { "type": "integer" }, "nsec": { "type": "integer" } } },
    "parent_frame_id": { "type": "string" },
    "child_frame_id":  { "type": "string" },
    "translation": {
      "type": "object",
      "properties": { "x": { "type": "number" }, "y": { "type": "number" }, "z": { "type": "number" } }
    },
    "rotation": {
      "type": "object",
      "properties": { "x": { "type": "number" }, "y": { "type": "number" }, "z": { "type": "number" }, "w": { "type": "number" } }
    }
  }
}"#;

pub const JOINT_STATE_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "header": {
      "type": "object",
      "properties": {
        "stamp":    { "type": "object", "properties": { "sec": { "type": "integer" }, "nsec": { "type": "integer" } } },
        "frame_id": { "type": "string" }
      }
    },
    "name":     { "type": "array", "items": { "type": "string" } },
    "position": { "type": "array", "items": { "type": "number" } },
    "velocity": { "type": "array", "items": { "type": "number" } },
    "effort":   { "type": "array", "items": { "type": "number" } }
  }
}"#;

pub const CAMERA_CALIBRATION_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "timestamp":         { "type": "object", "properties": { "sec": { "type": "integer" }, "nsec": { "type": "integer" } } },
    "frame_id":          { "type": "string" },
    "width":             { "type": "integer" },
    "height":            { "type": "integer" },
    "distortion_model":  { "type": "string" },
    "D": { "type": "array", "items": { "type": "number" } },
    "K": { "type": "array", "items": { "type": "number" } },
    "P": { "type": "array", "items": { "type": "number" } },
    "R": { "type": "array", "items": { "type": "number" } }
  }
}"#;

// ── DJI raw telemetry schemas ─────────────────────────────────────────────────

pub const OSD_SCHEMA: &str = r#"{
  "type": "object",
  "title": "dji.OSD",
  "properties": {
    "fly_time":                    { "type": "number",  "description": "Flight time (s)" },
    "latitude":                    { "type": "number",  "description": "Latitude (°)" },
    "longitude":                   { "type": "number",  "description": "Longitude (°)" },
    "altitude":                    { "type": "number",  "description": "Altitude above sea level (m)" },
    "height":                      { "type": "number",  "description": "Height above ground (m)" },
    "height_max":                  { "type": "number",  "description": "Maximum height reached (m)" },
    "vps_height":                  { "type": "number",  "description": "Visual Positioning System height (m)" },
    "x_speed":                     { "type": "number",  "description": "Speed along X axis (m/s)" },
    "x_speed_max":                 { "type": "number" },
    "y_speed":                     { "type": "number",  "description": "Speed along Y axis (m/s)" },
    "y_speed_max":                 { "type": "number" },
    "z_speed":                     { "type": "number",  "description": "Vertical speed (m/s)" },
    "z_speed_max":                 { "type": "number" },
    "pitch":                       { "type": "number",  "description": "Body pitch (°)" },
    "roll":                        { "type": "number",  "description": "Body roll (°)" },
    "yaw":                         { "type": "number",  "description": "Body yaw / compass heading (°)" },
    "gps_num":                     { "type": "integer", "description": "GPS satellites in view" },
    "gps_level":                   { "type": "integer", "description": "GPS signal level" },
    "voltage_warning":             { "type": "integer", "description": "Battery voltage warning level" },
    "is_gpd_used":                 { "type": "boolean", "description": "GPS in use" },
    "is_on_ground":                { "type": "boolean" },
    "is_motor_on":                 { "type": "boolean" },
    "is_motor_blocked":            { "type": "boolean" },
    "is_imu_preheated":            { "type": "boolean" },
    "is_acceletor_over_range":     { "type": "boolean" },
    "is_barometer_dead_in_air":    { "type": "boolean" },
    "is_compass_error":            { "type": "boolean" },
    "is_go_home_height_modified":  { "type": "boolean" },
    "is_not_enough_force":         { "type": "boolean", "description": "Low battery warning" },
    "is_out_of_limit":             { "type": "boolean" },
    "is_propeller_catapult":       { "type": "boolean" },
    "is_vibrating":                { "type": "boolean" },
    "is_vision_used":              { "type": "boolean", "description": "Vision positioning active" },
    "can_ioc_work":                { "type": "boolean" },
    "wave_error":                  { "type": "boolean" },
    "is_swave_work":               { "type": "boolean", "description": "Obstacle avoidance active" },
    "flyc_state":                  { "type": "string",  "description": "Flight controller state / mode" },
    "flyc_command":                { "type": "string" },
    "flight_action":               { "type": "string" },
    "go_home_status":              { "type": "string" },
    "non_gps_cause":               { "type": "string" },
    "drone_type":                  { "type": "string" },
    "battery_type":                { "type": "string" },
    "motor_start_failed_cause":    { "type": "string" },
    "imu_init_fail_reason":        { "type": "string" }
  }
}"#;

pub const GIMBAL_SCHEMA: &str = r#"{
  "type": "object",
  "title": "dji.Gimbal",
  "properties": {
    "pitch":             { "type": "number",  "description": "Gimbal pitch (°, absolute world frame)" },
    "roll":              { "type": "number",  "description": "Gimbal roll (°, absolute world frame)" },
    "yaw":               { "type": "number",  "description": "Gimbal yaw (°, absolute world frame)" },
    "mode":              { "type": "string" },
    "is_pitch_at_limit": { "type": "boolean" },
    "is_roll_at_limit":  { "type": "boolean" },
    "is_yaw_at_limit":   { "type": "boolean" },
    "is_stuck":          { "type": "boolean" }
  }
}"#;

pub const BATTERY_SCHEMA: &str = r#"{
  "type": "object",
  "title": "dji.Battery",
  "properties": {
    "charge_level":               { "type": "integer", "description": "State of charge (%)" },
    "voltage":                    { "type": "number",  "description": "Pack voltage (V)" },
    "current":                    { "type": "number",  "description": "Current (A)" },
    "current_capacity":           { "type": "integer" },
    "full_capacity":              { "type": "integer" },
    "cell_num":                   { "type": "integer" },
    "cell_voltages":              { "type": "array", "items": { "type": "number" }, "description": "Per-cell voltages (V)" },
    "cell_voltage_deviation":     { "type": "number" },
    "max_cell_voltage_deviation": { "type": "number" },
    "temperature":                { "type": "number",  "description": "Battery temperature (°C)" },
    "min_temperature":            { "type": "number" },
    "max_temperature":            { "type": "number" },
    "is_cell_voltage_estimated":  { "type": "boolean" }
  }
}"#;

pub const RC_SCHEMA: &str = r#"{
  "type": "object",
  "title": "dji.RC",
  "properties": {
    "aileron":         { "type": "integer", "description": "Right stick horizontal (roll)" },
    "elevator":        { "type": "integer", "description": "Right stick vertical (pitch)" },
    "throttle":        { "type": "integer", "description": "Left stick vertical" },
    "rudder":          { "type": "integer", "description": "Left stick horizontal (yaw)" },
    "downlink_signal": { "type": "integer", "description": "RC downlink signal strength" },
    "uplink_signal":   { "type": "integer", "description": "RC uplink signal strength" }
  }
}"#;

pub const HOME_SCHEMA: &str = r#"{
  "type": "object",
  "title": "dji.Home",
  "properties": {
    "latitude":                        { "type": "number" },
    "longitude":                       { "type": "number" },
    "altitude":                        { "type": "number",  "description": "Home altitude (m)" },
    "height_limit":                    { "type": "number",  "description": "Max allowed height (m)" },
    "max_allowed_height":              { "type": "number" },
    "go_home_height":                  { "type": "integer", "description": "Return-to-home height (m)" },
    "is_home_record":                  { "type": "boolean" },
    "is_dynamic_home_point_enabled":   { "type": "boolean" },
    "is_near_distance_limit":          { "type": "boolean" },
    "is_near_height_limit":            { "type": "boolean" },
    "is_compass_calibrating":          { "type": "boolean" },
    "is_multiple_mode_enabled":        { "type": "boolean" },
    "is_beginner_mode":                { "type": "boolean" },
    "is_ioc_enabled":                  { "type": "boolean" },
    "go_home_mode":                    { "type": "string" },
    "compass_calibration_state":       { "type": "string" },
    "ioc_mode":                        { "type": "string" },
    "ioc_course_lock_angle":           { "type": "integer" },
    "current_flight_record_index":     { "type": "integer" }
  }
}"#;
