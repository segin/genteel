//! Benchmark of the GDB stub's packet framing and stop-reply strings.

use genteel::debugger::{GdbServer, PacketParser, ParseEvent, StopReason};
use std::time::Instant;

fn main() {
    // Basic verification
    assert_eq!(StopReason::Breakpoint.signal_string(), "S05");
    assert_eq!(StopReason::Step.signal_string(), "S05");
    assert_eq!(StopReason::Interrupt.signal_string(), "S02");

    /* A realistic stream: register reads, memory reads, a continue. */
    let mut stream = Vec::new();
    for cmd in ["g", "m200,40", "?", "Z0,20a,2", "c"] {
        stream.extend_from_slice(b"+");
        stream.extend_from_slice(&GdbServer::frame_packet(cmd));
    }
    let iterations = 200_000;

    let start = Instant::now();
    let mut parser = PacketParser::new();
    let mut packets = 0usize;
    for _ in 0..iterations {
        for &b in &stream {
            if let Some(ParseEvent::Packet(p)) = parser.push(b) {
                packets += p.len();
            }
        }
    }
    let parse = start.elapsed();

    let start = Instant::now();
    let mut framed = 0usize;
    for _ in 0..iterations {
        framed += GdbServer::frame_packet(StopReason::Breakpoint.signal_string()).len();
    }
    let frame = start.elapsed();

    println!("Iterations: {}", iterations);
    println!(
        "Parse {} bytes/iteration: {:?} ({} payload bytes)",
        stream.len(),
        parse,
        packets
    );
    println!("Frame stop reply: {:?} ({} bytes)", frame, framed);
}
