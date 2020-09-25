#![warn(
    rust_2018_idioms,
    deprecated_in_future,
    macro_use_extern_crate,
    missing_debug_implementations,
    unused_qualifications
)]

use std::{path::PathBuf, process};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Args {
    /// The path to open
    #[structopt(parse(from_os_str))]
    path: PathBuf,

    /// Open the path with the `open_browser()` function instead of the `open` function
    #[structopt(long = "browser")]
    browser: bool,
}

fn main() {
    let args = Args::from_args();

    let open_result = if args.browser {
        opener::open_browser(&args.path)
    } else {
        opener::open(&args.path)
    };

    match open_result {
        Ok(()) => {
            println!("Opened path successfully.");
        }
        Err(e) => {
            println!("Failed to open path.\n\nerror:\n\n{:#?}", e);
            process::exit(1);
        }
    }
}
