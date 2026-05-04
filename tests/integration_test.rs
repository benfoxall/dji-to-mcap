/// Integration tests that require actual flight log files.
///
/// Run with: cargo test -- --include-ignored
/// or: cargo test -- --ignored
use std::path::Path;

#[test]
#[ignore = "requires FlightRecord files from the DJI app (gitignored)"]
fn pipeline_produces_nonempty_mcap() {
    // Adjust this path to any log file present on your machine.
    // The FlightRecord directory is relative to the repo root.
    let log_path = Path::new("../../FlightRecord/FlightRecord_2026-01-02_[11-04-28].txt");

    if !log_path.exists() {
        eprintln!("Skipping: log file not found at {}", log_path.display());
        return;
    }

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let out_path = tmp.path().to_path_buf();

    let api_key = std::env::var("DJI_OPEN_API_KEY").ok();

    djicap::pipeline::process(log_path.to_path_buf(), out_path.clone(), api_key)
        .expect("pipeline should succeed");

    let metadata = std::fs::metadata(&out_path).unwrap();
    assert!(metadata.len() > 0, "output .mcap should not be empty");

    // Read back with the mcap crate and count messages
    let bytes = std::fs::read(&out_path).unwrap();
    let mut message_count = 0u64;
    for record in mcap::read::LinearReader::new(&bytes).unwrap() {
        if let Ok(mcap::records::Record::Message { .. }) = record {
            message_count += 1;
        }
    }
    assert!(message_count > 0, "MCAP should contain messages");
    eprintln!("Total messages in MCAP: {message_count}");
}
