#![warn(rust_2018_idioms)]

use cargo_n64::{handle_errors, run};

fn main() {
    handle_errors(run);
}
