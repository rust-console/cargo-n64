# `cargo-n64`

A `cargo` subcommand to build Nintendo 64 ROMs in Rust! 🦀

## Installation

Requires Rust nightly. You can use `rustup` to install the toolchain:

```bash
rustup toolchain install nightly
```

Install dependencies:

```bash
rustup run nightly rustup component add rust-src
cargo +nightly install cargo-xbuild
```

Install `cargo-n64`:

```bash
cargo +nightly install cargo-n64
```

## What does it do?

Nintendo 64 ROMs are flat binaries, and each one is unique. There is no standard format for the binary beyond a simple 64-byte header and a ~4KB bootcode (aka Initial Program Loader 3/IPL3). Everything beyond the first 4KB boundary is MIPS code and whatever data it requires. This is unlike modern application or game development where an operating system has a standard binary format (like ELF, PE, or WASM). In fact, the N64 doesn't even have an operating system! The flat binary in the ROM *is* the operating system, for all intents and purposes.

This makes it challenging to get started with N64 development, in general. You first have to build an OS from scratch, or use a library like [`libdragon`](https://github.com/DragonMinded/libdragon) or [`libn64`](https://github.com/tj90241/n64chain/tree/master/libn64). Then you need a tool (or two, or three!) to convert the object files from the compiler toolchain into a flat binary, add the header and IPL3, and finally fix the IPL3 checksum. `cargo-n64` takes the place of the latter set of tools and plugs in nicely to the Rust/cargo ecosystem.

For copyright purposes, the IPL3 binary is not included in this package. Collecting a working IPL3 binary is left as an exercise for the reader. You will be required to provide the path to your IPL3 with the `--ipl3` command line argument.
