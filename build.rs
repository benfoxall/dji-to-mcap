use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(env::var("OUT_DIR")?);

    prost_build::Config::new()
        // Map google.protobuf.Timestamp to prost_types so we don't generate our own.
        .extern_path(".google.protobuf.Timestamp", "::prost_types::Timestamp")
        .file_descriptor_set_path(out.join("foxglove_descriptor.bin"))
        .compile_protos(
            &[
                "proto/foxglove/CompressedVideo.proto",
                "proto/foxglove/CompressedImage.proto",
            ],
            &["proto/"],
        )?;

    Ok(())
}
