#![warn(rust_2018_idioms)]

use cargo_n64::{build, handle_errors};

fn main() {
    handle_errors(build);
}
