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
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    let output = cli.output.unwrap_or_else(|| {
        let mut p = cli.input.clone();
        p.set_extension("mcap");
        p
    });

    djicap::pipeline::process(cli.input, output, cli.api_key)
}
