use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "djicap",
    about = "Convert DJI flight logs to Foxglove MCAP files",
    long_about = "Parses DJI flight records (.txt) and writes a .mcap file viewable in Foxglove Studio.\n\nSee https://developer.dji.com/policies/flight_record/ for DJI data usage terms."
)]
struct Cli {
    /// DJI flight log file (.txt)
    input: PathBuf,

    /// Output .mcap file (defaults to <input>.mcap)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// DJI Open API key for decrypting v13+ logs
    #[arg(long, env = "DJI_OPEN_API_KEY")]
    api_key: Option<String>,

    /// Directory containing MP4/JPG files to embed alongside telemetry.
    /// Files are matched to the flight time window automatically.
    #[arg(long)]
    media: Option<PathBuf>,

    /// Scale video to this pixel width (height computed proportionally).
    /// Triggers H.264 transcoding; drastically reduces MCAP size for 4K HEVC.
    /// Example: --scale 1280
    #[arg(long)]
    scale: Option<u32>,
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    let output = cli.output.unwrap_or_else(|| {
        let mut p = cli.input.clone();
        p.set_extension("mcap");
        p
    });

    djicap::pipeline::process(cli.input, output, cli.api_key, cli.media, cli.scale)
}
