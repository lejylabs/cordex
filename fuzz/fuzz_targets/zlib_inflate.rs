#![no_main]

use cordex_gateway::zlib::ZlibStream;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut stream = ZlibStream::with_limit(256 * 1024);
    let _ = stream.push(data);
});
