
use genteel::debugger::{GdbServer, GdbRegisters, GdbMemory};
use genteel::debugger::gdb::MAX_BREAKPOINTS;

// Minimal mock memory implementation for GdbServer
struct MockMemory;

impl GdbMemory for MockMemory {
    fn read_byte(&mut self, _addr: u32) -> u8 {
        0
    }
    fn write_byte(&mut self, _addr: u32, _val: u8) {
        // no-op
    }
}

#[test]
fn test_verify_gdb_breakpoint_limit() {
    // Create a new GDB server with a known password
    let password = "secret";
    let mut server = GdbServer::new(0, Some(password.to_string())).expect("Failed to create GDB server");

    let mut regs = GdbRegisters::default();
    let mut mem = MockMemory;

    // Authenticate first
    // "auth secret" in hex: 6175746820736563726574
    let auth_cmd_hex = "6175746820736563726574";
    let auth_packet = format!("qRcmd,{}", auth_cmd_hex);
    let auth_resp = server.process_command(&auth_packet, &mut regs, &mut mem);
    assert_eq!(auth_resp, "OK", "Authentication failed");

    println!("Attempting to add {} breakpoints...", MAX_BREAKPOINTS);

    // Fill up breakpoints to the limit
    for i in 0..MAX_BREAKPOINTS {
        let cmd = format!("Z0,{:x},4", i);
        let response = server.process_command(&cmd, &mut regs, &mut mem);

        if response != "OK" {
            panic!("Failed to add breakpoint at index {}: Response was '{}'", i, response);
        }
    }

    // Verify limit is reached
    assert_eq!(server.breakpoints.len(), MAX_BREAKPOINTS, "Breakpoint count mismatch");

    // Try adding one more (new) breakpoint
    println!("Attempting to add one more breakpoint (should fail)...");
    let cmd_overflow = format!("Z0,{:x},4", MAX_BREAKPOINTS);
    let response = server.process_command(&cmd_overflow, &mut regs, &mut mem);

    // Assert that it fails with "E01" (Error)
    assert_eq!(
        response,
        "E01",
        "SECURITY VULNERABILITY: Successfully added breakpoint beyond MAX_BREAKPOINTS limit!"
    );

    // Verify count did not increase
    assert_eq!(server.breakpoints.len(), MAX_BREAKPOINTS, "Breakpoint count increased beyond limit");

    println!("Security verification PASSED: Breakpoint limit enforced.");
}
