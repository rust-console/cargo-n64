#![warn(rust_2018_idioms)]
#![forbid(unsafe_code)]

use cargo_n64::{handle_errors, run};
use std::env;

fn main() {
    let args: Vec<_> = env::args().skip(1).collect();

    handle_errors(run, &args);
}
