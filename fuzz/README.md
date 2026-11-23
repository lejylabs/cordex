# Fuzzing

`cargo-fuzz` targets live outside the main workspace so regular builds on
stable Rust stay unaffected. Fuzzing requires Linux (or WSL) and a nightly
toolchain with `cargo-fuzz` installed:

```bash
cargo install cargo-fuzz
cargo +nightly fuzz run envelope -- -max_total_time=600
cargo +nightly fuzz run event -- -max_total_time=600
cargo +nightly fuzz run zlib_inflate -- -max_total_time=600
cargo +nightly fuzz run verify_signature -- -max_total_time=600
```
