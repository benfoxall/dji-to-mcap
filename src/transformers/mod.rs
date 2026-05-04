pub mod foxglove;
pub mod joints;
pub mod raw;

use anyhow::Result;
use dji_log_parser::frame::Frame;

pub struct TransformedMessage {
    pub topic: String,
    pub schema_name: String,
    pub schema_encoding: String,
    pub schema_data: Vec<u8>,
    pub payload: Vec<u8>,
}

pub trait Transformer {
    fn transform(&mut self, frame: &Frame, timestamp_ns: u64) -> Result<Vec<TransformedMessage>>;
}
