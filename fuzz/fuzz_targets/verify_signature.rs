#![no_main]

use cordex_interactions::verify::{PublicKey, verify_signature};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 32 {
        return;
    }
    let key_hex: String = data[..32].iter().map(|b| format!("{b:02x}")).collect();
    let Ok(pk) = PublicKey::from_hex(&key_hex) else {
        return;
    };
    let rest = &data[32..];
    let split = rest.len() / 2;
    let sig_hex: String = rest[..split].iter().map(|b| format!("{b:02x}")).collect();
    let _ = verify_signature(&pk, "1700000000", &sig_hex, &rest[split..]);
});
