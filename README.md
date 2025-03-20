# fustc: Faster Rust Compiler

fustc is a faster Rust compiler that utilize per-function caching of checking results.

## Installation

Run the following commands to install both the `fustc` compiler and the `cargo-fustc` utility:

```bash
cargo install --path fustc --locked
cargo install --path cargo-fustc --locked
```

## Usage

Build your project with fustc by running:

```bash
cargo fustc build
```
