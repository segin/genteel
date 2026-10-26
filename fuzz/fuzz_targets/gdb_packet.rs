#![no_main]

//! GDB remote-protocol framing fuzzer.
//!
//! Feeds arbitrary bytes (including non-ASCII) to `PacketParser` and checks
//! that it never panics and never yields a payload over the size limit.
//! Then frames the printable part of the input as a packet and checks that
//! parsing it returns the same payload.

use genteel::debugger::gdb::MAX_PACKET_SIZE;
use genteel::debugger::{GdbServer, PacketParser, ParseEvent};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut parser = PacketParser::new();
    for &b in data {
        if let Some(ParseEvent::Packet(payload)) = parser.push(b) {
            assert!(
                payload.len() <= MAX_PACKET_SIZE,
                "oversized payload accepted"
            );
        }
    }

    /* Round trip: printable ASCII without the framing characters. */
    let payload: String = data
        .iter()
        .map(|&b| b as char)
        .filter(|c| c.is_ascii_graphic() && *c != '$' && *c != '#')
        .take(MAX_PACKET_SIZE)
        .collect();
    let mut parser = PacketParser::new();
    let events: Vec<ParseEvent> = GdbServer::frame_packet(&payload)
        .into_iter()
        .filter_map(|b| parser.push(b))
        .collect();
    assert_eq!(events, vec![ParseEvent::Packet(payload.into_bytes())]);
});
