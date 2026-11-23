#![no_main]

use cordex_model::gateway::Event;
use libfuzzer_sys::fuzz_target;
use serde_json::value::RawValue;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Some((name, json)) = text.split_once('\n') else {
        return;
    };
    if let Ok(raw) = serde_json::from_str::<&RawValue>(json) {
        let _ = Event::from_dispatch(name, Some(raw));
    }
});
