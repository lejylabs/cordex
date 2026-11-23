#![no_main]

use cordex_model::gateway::Envelope;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = Envelope::parse(text);
    }
});
