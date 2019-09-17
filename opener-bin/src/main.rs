extern crate failure;
extern crate opener;
extern crate structopt;

use failure::{Error, ResultExt};
use std::{path::PathBuf, process};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(parse(from_os_str))]
    path: PathBuf,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {}", e);
        for cause in e.iter_causes() {
            eprintln!("caused by: {}", cause);
        }
        process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let args = Cli::from_args();

    opener::open(&args.path).context("failed to open path")?;

    println!("Opened path successfully.");

    Ok(())
}
