# Fm Synth

## Building

After installing [Rust](https://rustup.rs/), you can compile Fm Synth as follows:

```shell
cargo xtask bundle fm_synth --release
```

## Standalone
If you want to have a standalone app, create a file called `main.rs` and insert the following.
```rust
use fm_synth::FmSynth;
use nih_plug::wrapper::standalone::nih_export_standalone;
fn main() {
    nih_export_standalone::<FmSynth>();
}
```

## Issues
* We are getting pops on overlapping note changes. This is due to the adsr resetting to the 0. We need to fix this by implementing note stealing.
