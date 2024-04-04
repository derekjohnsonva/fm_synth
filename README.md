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

## TODO:

- Add a GUI
- Polyphonic Modulation (Change params per-voice)
- Preset Management
