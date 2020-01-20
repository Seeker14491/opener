#![warn(
    rust_2018_idioms,
    deprecated_in_future,
    macro_use_extern_crate,
    missing_debug_implementations,
    unused_qualifications,
    clippy::cast_possible_truncation
)]

use std::{path::PathBuf, process};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(parse(from_os_str))]
    path: PathBuf,
}

fn main() {
    let args = Cli::from_args();

    match opener::open(&args.path) {
        Ok(()) => {
            println!("Opened path successfully.");
        }
        Err(e) => {
            println!("Failed to open path.\n\nerror:\n\n{:#?}", e);
            process::exit(1);
        }
    }
}
