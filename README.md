# `cargo-n64`

[![Build Status](https://travis-ci.org/rust-console/cargo-n64.svg?branch=master)](https://travis-ci.org/rust-console/cargo-n64)
[![Crates.io](https://img.shields.io/crates/v/cargo-n64)](https://crates.io/crates/cargo-n64)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)

A `cargo` subcommand to build Nintendo 64 ROMs in Rust! 🦀

## Installation

Requires Rust nightly.

When built from source, `cargo` will automatically install the correct version of the nightly compiler, based on the `rust-toolchain` file. However you will still need to install the `rust-src` component separately. It is recommended that you explicitly install the dependencies with the same versions used in CI, as described below.

Install dependencies:

```bash
rustup toolchain install $(cat rust-toolchain)
rustup run $(cat rust-toolchain) -- rustup component add rust-src
```

Install `cargo-n64` from source:

```bash
cargo install --path cargo-n64
```

Install `cargo-n64` from [crates.io](https://crates.io/):

```bash
cargo install cargo-n64
```

## What does it do?

Nintendo 64 ROMs are flat binaries, and each one is unique. There is no standard format for the binary beyond a simple 64-byte header and a \~4KB bootcode (aka Initial Program Loader 3/IPL3). Everything beyond the first 4KB boundary is MIPS code and whatever data it requires. This is unlike modern application or game development where an operating system has a standard binary format (like ELF, PE, or WASM). In fact, the N64 doesn't even have an operating system! The flat binary in the ROM *is* the operating system, for all intents and purposes.

This makes it challenging to get started with N64 development, in general. You first have to build an OS from scratch, or use a library like [`libdragon`](https://github.com/DragonMinded/libdragon) or [`libn64`](https://github.com/tj90241/n64chain/tree/master/libn64). Then you need a tool (or two, or three!) to convert the object files from the compiler toolchain into a flat binary, add the header and IPL3, and finally fix the IPL3 checksum. `cargo-n64` takes the place of the latter set of tools and plugs in nicely to the Rust/cargo ecosystem.

For copyright purposes, the IPL3 binary is not included in this package. Collecting a working IPL3 binary is left as an exercise for the reader. You will be required to provide the path to your IPL3 with the `--ipl3` command line argument, or extract it from an existing ROM with `--ipl3-from-rom`.
