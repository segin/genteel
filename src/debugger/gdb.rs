//! GDB Remote Serial Protocol Server
//!
//! Implements a GDB stub for debugging M68k code running in the emulator.
//! Connect with: `m68k-elf-gdb -ex "target remote :1234"`

use std::collections::HashSet;
use std::io::{BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

/// Default GDB server port
pub const DEFAULT_PORT: u16 = 1234;

/// Maximum GDB packet size to prevent unbounded memory consumption
pub const MAX_PACKET_SIZE: usize = 4096;

/// GDB stop reasons
#[derive(Debug, Clone, Copy)]
pub enum StopReason {
    /// Target halted (initial state)
    Halted,
    /// Hit a breakpoint
    Breakpoint,
    /// Single step completed
    Step,
    /// SIGINT (user break)
    Interrupt,
}

impl StopReason {
    /// Convert to GDB signal number
    pub fn signal(&self) -> u8 {
        match self {
            StopReason::Halted => 5,     // SIGTRAP
            StopReason::Breakpoint => 5, // SIGTRAP
            StopReason::Step => 5,       // SIGTRAP
            StopReason::Interrupt => 2,  // SIGINT
        }
    }

}

/// Constant-time string comparison to prevent timing attacks
///
/// Note: This function leaks the length of the string via early return,
/// but performs constant-time comparison for strings of equal length
/// to prevent prefix matching attacks.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }

    let mut result = 0;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// GDB Server state
pub struct GdbServer {
    /// TCP listener
    listener: TcpListener,
    /// Connected client stream
    client: Option<TcpStream>,
    /// Breakpoints (set of addresses)
    pub breakpoints: HashSet<u32>,
    /// Last stop reason
    pub stop_reason: StopReason,
    /// No-ack mode enabled
    no_ack_mode: bool,
    /// Optional password for authentication
    password: Option<String>,
    /// Whether the client is authenticated
    authenticated: bool,
}

impl GdbServer {
    /// Create a new GDB server
    pub fn new(port: u16, password: Option<String>) -> std::io::Result<Self> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
        listener.set_nonblocking(true)?;

        let final_password = if let Some(pwd) = password {
            eprintln!(
                "🔒 GDB Server listening on 127.0.0.1:{}. Protected with password.",
                port
            );
            Some(pwd)
        } else {
            let token = format!("{:032x}", rand::random::<u128>());
            eprintln!(
                "🔒 GDB Server listening on 127.0.0.1:{}. Protected with auto-generated token.",
                port
            );
            eprintln!(
                "👉 Run this command in GDB to authenticate: monitor auth {}",
                token
            );
            Some(token)
        };

        // Always start unauthenticated to enforce token check
        let authenticated = false;

        Ok(Self {
            listener,
            client: None,
            breakpoints: HashSet::new(),
            stop_reason: StopReason::Halted,
            no_ack_mode: false,
            password: final_password,
            authenticated,
        })
    }

    /// Check for new connections (non-blocking)
    pub fn accept(&mut self) -> bool {
        if self.client.is_some() {
            return true;
        }

        match self.listener.accept() {
            Ok((stream, addr)) => {
                // Security check: Only allow loopback connections
                if !addr.ip().is_loopback() {
                    eprintln!(
                        "⚠️  SECURITY ALERT: Rejected GDB connection from non-loopback address: {}",
                        addr
                    );
                    return false;
                }

                eprintln!("ℹ️  Accepted GDB connection from {}", addr);
                stream.set_nonblocking(true).ok();
                self.client = Some(stream);
                true
            }
            Err(_) => false,
        }
    }

    /// Check if client is connected
    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    /// Send a packet to the client
    pub fn send_packet(&mut self, data: &str) -> std::io::Result<()> {
        if let Some(ref mut client) = self.client {
            let checksum = data.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));
            let packet = format!("${}#{:02x}", data, checksum);
            client.write_all(packet.as_bytes())?;
            client.flush()?;
        }
        Ok(())
    }

    /// Receive a packet from the client (non-blocking)
    pub fn receive_packet(&mut self) -> Option<String> {
        let client = self.client.as_mut()?;
        let mut reader = BufReader::new(client.try_clone().ok()?);

        let mut buf = [0u8; 1];

        // Look for packet start
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    // Connection closed
                    self.client = None;
                    return None;
                }
                Ok(1) => {
                    if buf[0] == b'$' {
                        break;
                    } else if buf[0] == b'+' || buf[0] == b'-' {
                        // ACK/NAK, ignore
                        continue;
                    } else if buf[0] == 0x03 {
                        // Ctrl+C interrupt
                        self.stop_reason = StopReason::Interrupt;
                        return Some("INTERRUPT".to_string());
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return None;
                }
                Err(_) => {
                    self.client = None;
                    return None;
                }
                _ => {}
            }
        }

        // Read until #
        let mut data = String::new();
        loop {
            match reader.read(&mut buf) {
                Ok(1) => {
                    if buf[0] == b'#' {
                        break;
                    }
                    // Security: Prevent unbounded memory consumption by disconnecting clients that send oversized packets
                    if data.len() >= MAX_PACKET_SIZE {
                        eprintln!("⚠️  SECURITY ALERT: GDB packet exceeded maximum size of {}. Disconnecting.", MAX_PACKET_SIZE);
                        self.client = None;
                        return None;
                    }
                    data.push(buf[0] as char);
                }
                Ok(0) => {
                    self.client = None;
                    return None;
                }
                _ => return None,
            }
        }

        // Read checksum (2 chars)
        let mut checksum_buf = [0u8; 2];
        if reader.read_exact(&mut checksum_buf).is_err() {
            return None;
        }

        // Validate checksum
        let received_checksum =
            u8::from_str_radix(std::str::from_utf8(&checksum_buf).unwrap_or("00"), 16).unwrap_or(0);

        let calculated_checksum = data.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));

        // Send ACK/NAK
        if !self.no_ack_mode {
            if let Some(ref mut c) = self.client {
                let ack = if received_checksum == calculated_checksum {
                    b'+'
                } else {
                    b'-'
                };
                c.write_all(&[ack]).ok();
                c.flush().ok();
            }
        }

        if received_checksum == calculated_checksum {
            Some(data)
        } else {
            None
        }
    }

    fn check_authentication(&self, cmd: &str) -> bool {
        if !self.authenticated {
            let allowed = cmd.starts_with("qSupported")
                || cmd == "?"
                || cmd.starts_with("qRcmd,")
                || cmd.starts_with('H')
                || cmd == "QStartNoAckMode";

            if !allowed {
                return false;
            }
        }
        true
    }

    fn handle_control_command(&mut self, cmd: &str, first_char: char) -> String {
        match first_char {
            '?' => {
                // Stop reason
                format!("S{:02x}", self.stop_reason.signal())
            }
            'c' => {
                // Continue
                "CONTINUE".to_string()
            }
            's' => {
                // Single step
                "STEP".to_string()
            }
            'H' => {
                // Set thread (we only have one, just acknowledge)
                "OK".to_string()
            }
            'D' => {
                // Detach
                self.client = None;
                "OK".to_string()
            }
            'k' => {
                // Kill
                self.client = None;
                "".to_string()
            }
            _ => {
                if cmd == "INTERRUPT" {
                    format!("S{:02x}", StopReason::Interrupt.signal())
                } else {
                    "".to_string()
                }
            }
        }
    }

    fn handle_register_command(
        &self,
        cmd: &str,
        first_char: char,
        registers: &mut GdbRegisters,
    ) -> String {
        match first_char {
            'g' => {
                // Read all registers
                self.read_registers(registers)
            }
            'G' => {
                // Write all registers
                self.write_registers(&cmd[1..], registers)
            }
            'p' => {
                // Read single register
                if let Ok(reg_num) = u32::from_str_radix(&cmd[1..], 16) {
                    self.read_register(reg_num, registers)
                } else {
                    "E01".to_string()
                }
            }
            'P' => {
                // Write single register
                self.write_register(&cmd[1..], registers)
            }
            _ => "".to_string(),
        }
    }

    fn handle_memory_command(
        &self,
        cmd: &str,
        first_char: char,
        memory: &mut dyn GdbMemory,
    ) -> String {
        match first_char {
            'm' => {
                // Read memory
                self.read_memory(&cmd[1..], memory)
            }
            'M' => {
                // Write memory
                self.write_memory(&cmd[1..], memory)
            }
            _ => "".to_string(),
        }
    }

    fn handle_breakpoint_command(&mut self, cmd: &str, first_char: char) -> String {
        match first_char {
            'Z' => {
                // Set breakpoint
                self.set_breakpoint(&cmd[1..])
            }
            'z' => {
                // Remove breakpoint
                self.remove_breakpoint(&cmd[1..])
            }
            _ => "".to_string(),
        }
    }

    /// Process a GDB command and return the response
    pub fn process_command(
        &mut self,
        cmd: &str,
        registers: &mut GdbRegisters,
        memory: &mut dyn GdbMemory,
    ) -> String {
        let first_char = cmd.chars().next().unwrap_or('?');

        // Always allow disconnect/kill/interrupt
        if cmd == "INTERRUPT" || cmd == "D" || cmd == "k" {
            return self.handle_control_command(cmd, first_char);
        }

        // Authentication check
        if !self.check_authentication(cmd) {
            return "E01".to_string();
        }

        match first_char {
            '?' | 'c' | 's' | 'H' | 'D' | 'k' => self.handle_control_command(cmd, first_char),
            'g' | 'G' | 'p' | 'P' => self.handle_register_command(cmd, first_char, registers),
            'm' | 'M' => self.handle_memory_command(cmd, first_char, memory),
            'Z' | 'z' => self.handle_breakpoint_command(cmd, first_char),
            'q' => self.handle_query(cmd),
            'Q' => self.handle_set(cmd),
            _ => "".to_string(),
        }
    }

    fn read_registers(&self, registers: &GdbRegisters) -> String {
        use std::fmt::Write;
        let mut result = String::with_capacity(18 * 8);

        // D0-D7
        for &d in &registers.d {
            write!(result, "{:08x}", d).unwrap();
        }

        // A0-A7
        for &a in &registers.a {
            write!(result, "{:08x}", a).unwrap();
        }

        // SR
        write!(result, "{:08x}", registers.sr as u32).unwrap();

        // PC
        write!(result, "{:08x}", registers.pc).unwrap();

        result
    }

    fn write_registers(&self, data: &str, registers: &mut GdbRegisters) -> String {
        if data.len() < 144 {
            // 18 registers * 8 hex chars = 144
            return "E01".to_string();
        }

        let mut pos = 0;

        // D0-D7
        for i in 0..8 {
            if let Ok(v) = u32::from_str_radix(&data[pos..pos + 8], 16) {
                registers.d[i] = v;
            }
            pos += 8;
        }

        // A0-A7
        for i in 0..8 {
            if let Ok(v) = u32::from_str_radix(&data[pos..pos + 8], 16) {
                registers.a[i] = v;
            }
            pos += 8;
        }

        // SR
        if let Ok(v) = u32::from_str_radix(&data[pos..pos + 8], 16) {
            registers.sr = v as u16;
        }
        pos += 8;

        // PC
        if pos + 8 <= data.len() {
            if let Ok(v) = u32::from_str_radix(&data[pos..pos + 8], 16) {
                registers.pc = v;
            }
        }

        "OK".to_string()
    }

    fn read_register(&self, reg_num: u32, registers: &GdbRegisters) -> String {
        match reg_num {
            0..=7 => format!("{:08x}", registers.d[reg_num as usize]),
            8..=15 => format!("{:08x}", registers.a[(reg_num - 8) as usize]),
            16 => format!("{:08x}", registers.sr as u32),
            17 => format!("{:08x}", registers.pc),
            _ => "E01".to_string(),
        }
    }

    fn write_register(&self, cmd: &str, registers: &mut GdbRegisters) -> String {
        let parts: Vec<&str> = cmd.split('=').collect();
        if parts.len() != 2 {
            return "E01".to_string();
        }

        let reg_num = match u32::from_str_radix(parts[0], 16) {
            Ok(n) => n,
            Err(_) => return "E01".to_string(),
        };

        let value = match u32::from_str_radix(parts[1], 16) {
            Ok(v) => v,
            Err(_) => return "E01".to_string(),
        };

        match reg_num {
            0..=7 => registers.d[reg_num as usize] = value,
            8..=15 => registers.a[(reg_num - 8) as usize] = value,
            16 => registers.sr = value as u16,
            17 => registers.pc = value,
            _ => return "E01".to_string(),
        }

        "OK".to_string()
    }

    fn read_memory(&self, cmd: &str, memory: &mut dyn GdbMemory) -> String {
        use std::fmt::Write;
        let parts: Vec<&str> = cmd.split(',').collect();
        if parts.len() != 2 {
            return "E01".to_string();
        }

        let addr = match u32::from_str_radix(parts[0], 16) {
            Ok(a) => a,
            Err(_) => return "E01".to_string(),
        };

        let len = match usize::from_str_radix(parts[1], 16) {
            Ok(l) => l,
            Err(_) => return "E01".to_string(),
        };

        let mut result = String::with_capacity(len * 2);
        for i in 0..len {
            let byte = memory.read_byte(addr.wrapping_add(i as u32));
            write!(result, "{:02x}", byte).unwrap();
        }

        result
    }

    fn write_memory(&self, cmd: &str, memory: &mut dyn GdbMemory) -> String {
        let parts: Vec<&str> = cmd.split(':').collect();
        if parts.len() != 2 {
            return "E01".to_string();
        }

        let addr_len: Vec<&str> = parts[0].split(',').collect();
        if addr_len.len() != 2 {
            return "E01".to_string();
        }

        let addr = match u32::from_str_radix(addr_len[0], 16) {
            Ok(a) => a,
            Err(_) => return "E01".to_string(),
        };

        let data = parts[1];
        if data.len() % 2 != 0 {
            return "E01".to_string();
        }

        let mut i = 0;
        while i + 2 <= data.len() {
            match u8::from_str_radix(&data[i..i + 2], 16) {
                Ok(byte) => {
                    memory.write_byte(addr.wrapping_add((i / 2) as u32), byte);
                }
                Err(_) => return "E01".to_string(),
            }
            i += 2;
        }

        "OK".to_string()
    }

    fn set_breakpoint(&mut self, cmd: &str) -> String {
        let parts: Vec<&str> = cmd.split(',').collect();
        if parts.len() < 2 {
            return "E01".to_string();
        }

        // Type 0 = software breakpoint
        if parts[0] != "0" {
            return "".to_string(); // Not supported
        }

        let addr = match u32::from_str_radix(parts[1], 16) {
            Ok(a) => a,
            Err(_) => return "E01".to_string(),
        };

        self.breakpoints.insert(addr);
        "OK".to_string()
    }

    fn remove_breakpoint(&mut self, cmd: &str) -> String {
        let parts: Vec<&str> = cmd.split(',').collect();
        if parts.len() < 2 {
            return "E01".to_string();
        }

        if parts[0] != "0" {
            return "".to_string();
        }

        let addr = match u32::from_str_radix(parts[1], 16) {
            Ok(a) => a,
            Err(_) => return "E01".to_string(),
        };

        self.breakpoints.remove(&addr);
        "OK".to_string()
    }

    fn handle_query(&mut self, cmd: &str) -> String {
        if cmd.starts_with("qSupported") {
            // Report supported features
            format!("PacketSize={};swbreak+;QStartNoAckMode+", MAX_PACKET_SIZE)
        } else if cmd == "qC" {
            // Current thread
            "QC1".to_string()
        } else if cmd == "qfThreadInfo" {
            // Thread list
            "m1".to_string()
        } else if cmd == "qsThreadInfo" {
            // End of thread list
            "l".to_string()
        } else if cmd == "qAttached" {
            // Attached to existing process
            "1".to_string()
        } else if let Some(stripped) = cmd.strip_prefix("qRcmd,") {
            // Monitor command
            self.handle_monitor_command(stripped)
        } else {
            // Unknown query
            "".to_string()
        }
    }

    fn handle_monitor_command(&mut self, cmd_hex: &str) -> String {
        if cmd_hex.len() % 2 != 0 {
            return "E01".to_string();
        }

        let mut bytes = Vec::with_capacity(cmd_hex.len() / 2);
        let mut i = 0;
        while i + 2 <= cmd_hex.len() {
            match u8::from_str_radix(&cmd_hex[i..i + 2], 16) {
                Ok(byte) => bytes.push(byte),
                Err(_) => return "E01".to_string(),
            }
            i += 2;
        }

        let cmd = String::from_utf8_lossy(&bytes);
        if let Some(stripped) = cmd.strip_prefix("auth ") {
            let provided_pass = stripped.trim();
            if let Some(ref correct_pass) = self.password {
                if constant_time_eq(provided_pass, correct_pass) {
                    self.authenticated = true;
                    return "OK".to_string();
                } else {
                    return "E01".to_string(); // Invalid password
                }
            } else {
                // No password set, already authenticated
                self.authenticated = true;
                return "OK".to_string();
            }
        }

        // Other monitor commands?
        if !self.authenticated {
            return "E01".to_string();
        }

        // Add other monitor commands here if needed
        "OK".to_string()
    }

    fn handle_set(&mut self, cmd: &str) -> String {
        if cmd == "QStartNoAckMode" {
            self.no_ack_mode = true;
            "OK".to_string()
        } else {
            "".to_string()
        }
    }

    /// Check if address is a breakpoint
    pub fn is_breakpoint(&self, addr: u32) -> bool {
        self.breakpoints.contains(&addr)
    }
}

/// M68k register state for GDB
#[derive(Debug, Clone, Default)]
pub struct GdbRegisters {
    pub d: [u32; 8],
    pub a: [u32; 8],
    pub sr: u16,
    pub pc: u32,
}

/// Trait for memory access from GDB
pub trait GdbMemory {
    fn read_byte(&mut self, addr: u32) -> u8;
    fn write_byte(&mut self, addr: u32, value: u8);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockMemory {
        pub data: HashMap<u32, u8>,
    }

    impl MockMemory {
        fn new() -> Self {
            Self {
                data: HashMap::new(),
            }
        }
    }

    impl GdbMemory for MockMemory {
        fn read_byte(&mut self, addr: u32) -> u8 {
            *self.data.get(&addr).unwrap_or(&0)
        }
        fn write_byte(&mut self, addr: u32, value: u8) {
            self.data.insert(addr, value);
        }
    }

    fn create_test_server() -> GdbServer {
        GdbServer {
            listener: TcpListener::bind("127.0.0.1:0").unwrap(),
            client: None,
            breakpoints: HashSet::new(),
            stop_reason: StopReason::Halted,
            no_ack_mode: false,
            password: None,
            authenticated: true,
        }
    }

    #[test]
    fn test_handle_monitor_command_unrecognized() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        // Unrecognized monitor command should return "OK" when authenticated
        // "test" in hex: 74657374
        assert_eq!(
            server.process_command("qRcmd,74657374", &mut regs, &mut mem),
            "OK"
        );
    }

    #[test]
    fn test_checksum() {
        let data = "OK";
        let checksum = data.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));
        assert_eq!(checksum, 0x9a);
    }

    #[test]
    fn test_stop_reason() {
        let sr = StopReason::Breakpoint;
        assert_eq!(sr.signal(), 5);
    }

    #[test]
    fn test_gdb_registers_default() {
        let regs = GdbRegisters::default();
        assert_eq!(regs.d[0], 0);
        assert_eq!(regs.a[7], 0);
        assert_eq!(regs.pc, 0);
    }

    #[test]
    fn test_breakpoint_management() {
        let mut server = create_test_server();

        // Set breakpoint
        let result = server.set_breakpoint("0,1000,4");
        assert_eq!(result, "OK");
        assert!(server.is_breakpoint(0x1000));

        // Remove breakpoint
        let result = server.remove_breakpoint("0,1000,4");
        assert_eq!(result, "OK");
        assert!(!server.is_breakpoint(0x1000));
    }

    #[test]
    fn test_security_loopback_accepted() {
        // Bind to random port
        let mut server = GdbServer::new(0, None).expect("Failed to create GDB server");
        let port = server
            .listener
            .local_addr()
            .expect("Failed to get local addr")
            .port();

        // Connect via loopback
        let _stream = TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Failed to connect");

        // Accept connection
        assert!(server.accept(), "Server should accept loopback connection");
        assert!(server.is_connected(), "Server should be connected");

        // Disconnect
        drop(_stream);
    }

    #[test]
    fn test_process_command_basic() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        // Test ? command
        assert_eq!(server.process_command("?", &mut regs, &mut mem), "S05");

        // Test custom INTERRUPT
        assert_eq!(
            server.process_command("INTERRUPT", &mut regs, &mut mem),
            "S02"
        );

        // Test continue and step
        assert_eq!(server.process_command("c", &mut regs, &mut mem), "CONTINUE");
        assert_eq!(server.process_command("s", &mut regs, &mut mem), "STEP");

        // Test unknown command
        assert_eq!(server.process_command("X", &mut regs, &mut mem), "");
    }

    #[test]
    fn test_process_command_memory() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        // Write memory: M1000,4:deadbeef
        let resp = server.process_command("M1000,4:deadbeef", &mut regs, &mut mem);
        assert_eq!(resp, "OK");
        assert_eq!(mem.read_byte(0x1000), 0xde);
        assert_eq!(mem.read_byte(0x1001), 0xad);
        assert_eq!(mem.read_byte(0x1002), 0xbe);
        assert_eq!(mem.read_byte(0x1003), 0xef);

        // Read memory: m1000,4
        let resp = server.process_command("m1000,4", &mut regs, &mut mem);
        assert_eq!(resp, "deadbeef");

        // Test malformed memory commands
        assert_eq!(server.process_command("m1000", &mut regs, &mut mem), "E01");
        assert_eq!(
            server.process_command("M1000,4", &mut regs, &mut mem),
            "E01"
        );
    }

    #[test]
    fn test_process_command_registers() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        // Set single register: P0b=12345678 (A3)
        let resp = server.process_command("P0b=12345678", &mut regs, &mut mem);
        assert_eq!(resp, "OK");
        assert_eq!(regs.a[3], 0x12345678);

        // Read single register: p0b
        let resp = server.process_command("p0b", &mut regs, &mut mem);
        assert_eq!(resp, "12345678");

        // Test g and G commands
        let g_resp = server.process_command("g", &mut regs, &mut mem);
        assert_eq!(g_resp.len(), 18 * 8);

        // G command (just test it doesn't crash with correct length)
        let g_data = "0".repeat(18 * 8);
        let resp = server.process_command(&format!("G{}", g_data), &mut regs, &mut mem);
        assert_eq!(resp, "OK");
    }

    #[test]
    fn test_process_command_queries() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        // Strict check for qSupported
        let expected_supported =
            format!("PacketSize={};swbreak+;QStartNoAckMode+", MAX_PACKET_SIZE);
        assert_eq!(
            server.process_command("qSupported", &mut regs, &mut mem),
            expected_supported
        );

        // Strict check for qC
        assert_eq!(server.process_command("qC", &mut regs, &mut mem), "QC1");

        // Strict check for qfThreadInfo
        assert_eq!(
            server.process_command("qfThreadInfo", &mut regs, &mut mem),
            "m1"
        );

        // Strict check for qsThreadInfo
        assert_eq!(
            server.process_command("qsThreadInfo", &mut regs, &mut mem),
            "l"
        );

        // Strict check for qAttached
        assert_eq!(
            server.process_command("qAttached", &mut regs, &mut mem),
            "1"
        );

        // Unknown/Unsupported queries should return empty string
        assert_eq!(server.process_command("qUnknown", &mut regs, &mut mem), "");

        // QStartNoAckMode
        assert_eq!(
            server.process_command("QStartNoAckMode", &mut regs, &mut mem),
            "OK"
        );
        assert!(server.no_ack_mode);
    }

    #[test]
    fn test_process_command_connection() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        assert_eq!(server.process_command("H", &mut regs, &mut mem), "OK");
        assert_eq!(server.process_command("D", &mut regs, &mut mem), "OK");
        assert_eq!(server.process_command("k", &mut regs, &mut mem), "");
    }

    #[test]
    fn test_oversized_packet_prevention() {
        let mut server = GdbServer::new(0, None).expect("Failed to create GDB server");
        let port = server
            .listener
            .local_addr()
            .expect("Failed to get local addr")
            .port();

        // Connect via loopback
        let mut client_stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Failed to connect");
        assert!(server.accept(), "Server should accept connection");

        // Send a very large packet without '#'
        let large_size = 10000;
        let mut large_packet = String::with_capacity(large_size + 1);
        large_packet.push('$');
        for _ in 0..large_size {
            large_packet.push('A');
        }

        client_stream
            .write_all(large_packet.as_bytes())
            .expect("Failed to write to server");
        client_stream.flush().expect("Failed to flush");

        // Try to receive the packet.
        let result = server.receive_packet();
        assert!(
            result.is_none(),
            "Should not return a valid packet for oversized input"
        );
        assert!(
            !server.is_connected(),
            "Server should have disconnected the client after oversized packet"
        );
    }

    #[test]
    fn test_authentication_flow() {
        let password = "secret".to_string();
        let mut server = GdbServer::new(0, Some(password)).unwrap();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        assert!(!server.authenticated);

        // Access denied for protected commands
        assert_eq!(server.process_command("g", &mut regs, &mut mem), "E01");

        // Authenticate success
        // "auth secret" in hex: 6175746820736563726574
        assert_eq!(
            server.process_command("qRcmd,6175746820736563726574", &mut regs, &mut mem),
            "OK"
        );
        assert!(server.authenticated);

        // Now commands work
        assert_eq!(
            server.process_command("g", &mut regs, &mut mem).len(),
            18 * 8
        );
    }

    #[test]
    fn test_default_security() {
        let mut server = GdbServer::new(0, None).unwrap();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        assert!(!server.authenticated);

        // Authenticate with auto-generated password
        let token = server.password.clone().unwrap();
        let auth_cmd = format!("auth {}", token);
        let auth_hex: String = auth_cmd.bytes().map(|b| format!("{:02x}", b)).collect();
        let cmd = format!("qRcmd,{}", auth_hex);

        assert_eq!(server.process_command(&cmd, &mut regs, &mut mem), "OK");
        assert!(server.authenticated);
    }

    #[test]
    fn test_write_memory_malformed() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        // Odd length
        assert_eq!(
            server.process_command("M100,2:123", &mut regs, &mut mem),
            "E01"
        );

        // Non-hex
        assert_eq!(
            server.process_command("M100,2:1g", &mut regs, &mut mem),
            "E01"
        );
    }

    #[test]
    fn test_monitor_malformed() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        // Odd length in qRcmd
        assert_eq!(
            server.process_command("qRcmd,123", &mut regs, &mut mem),
            "E01"
        );

        // Non-hex in qRcmd
        assert_eq!(
            server.process_command("qRcmd,1g", &mut regs, &mut mem),
            "E01"
        );
    }

    #[test]
    fn test_auto_generated_password() {
        let mut server = GdbServer::new(0, None).unwrap();
        assert!(!server.authenticated);
        assert!(server.password.is_some());
        let generated_pwd = server.password.as_ref().unwrap().clone();

        // Check password format (32 chars hex)
        assert_eq!(
            generated_pwd.len(),
            32,
            "Generated password should be 32 chars"
        );
        assert!(
            generated_pwd.chars().all(|c| c.is_digit(16)),
            "Generated password should be hex"
        );

        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        fn to_hex(s: &str) -> String {
            s.bytes().map(|b| format!("{:02x}", b)).collect()
        }

        let right_cmd_str = format!("auth {}", generated_pwd);
        let right_packet = format!("qRcmd,{}", to_hex(&right_cmd_str));
        assert_eq!(
            server.process_command(&right_packet, &mut regs, &mut mem),
            "OK"
        );
        assert!(server.authenticated);
    }

    #[test]
    fn test_process_command_breakpoints() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        assert_eq!(
            server.process_command("Z0,1000,4", &mut regs, &mut mem),
            "OK"
        );
        assert!(server.is_breakpoint(0x1000));
        assert_eq!(
            server.process_command("z0,1000,4", &mut regs, &mut mem),
            "OK"
        );
        assert!(!server.is_breakpoint(0x1000));
    }

    #[test]
    fn test_write_registers_length_validation() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        // 18 registers * 8 chars = 144
        let data_short = "0".repeat(143);
        assert_eq!(
            server.process_command(&format!("G{}", data_short), &mut regs, &mut mem),
            "E01"
        );

        let data_exact = "0".repeat(144);
        assert_eq!(
            server.process_command(&format!("G{}", data_exact), &mut regs, &mut mem),
            "OK"
        );
    }

    #[test]
    fn test_query_commands_strict_conformance() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        let expected_supported =
            format!("PacketSize={};swbreak+;QStartNoAckMode+", MAX_PACKET_SIZE);
        assert_eq!(
            server.process_command("qSupported", &mut regs, &mut mem),
            expected_supported
        );
        assert_eq!(server.process_command("qC", &mut regs, &mut mem), "QC1");
        assert_eq!(
            server.process_command("qfThreadInfo", &mut regs, &mut mem),
            "m1"
        );
        assert_eq!(
            server.process_command("qsThreadInfo", &mut regs, &mut mem),
            "l"
        );
        assert_eq!(
            server.process_command("qAttached", &mut regs, &mut mem),
            "1"
        );
    }

    #[test]
    fn test_breakpoint_edge_cases() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        assert_eq!(
            server.process_command("Z0,GG,4", &mut regs, &mut mem),
            "E01"
        );
        assert_eq!(
            server.process_command("Z0,2000,4", &mut regs, &mut mem),
            "OK"
        );
        assert!(server.is_breakpoint(0x2000));
        assert_eq!(
            server.process_command("z0,3000,4", &mut regs, &mut mem),
            "OK"
        );
        assert_eq!(server.process_command("Z1,1000,4", &mut regs, &mut mem), "");
    }

    #[test]
    fn test_auth_timing_attack_mitigation() {
        let password = "secret".to_string();
        // Since we cannot easily create a TcpListener on port 0 in tests without risking failure or mocking it fully,
        // we'll use GdbServer::new which internally binds. Hopefully port 0 works fine.
        let mut server = GdbServer::new(0, Some(password)).unwrap();

        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        assert!(!server.authenticated);

        // Helper to encode auth command
        fn make_auth_cmd(pwd: &str) -> String {
            let cmd_str = format!("auth {}", pwd);
            let hex_cmd: String = cmd_str.bytes().map(|b| format!("{:02x}", b)).collect();
            format!("qRcmd,{}", hex_cmd)
        }

        // Correct password
        let correct_cmd = make_auth_cmd("secret");
        assert_eq!(server.process_command(&correct_cmd, &mut regs, &mut mem), "OK");
        assert!(server.authenticated);

        // Reset auth for negative tests manually
        server.authenticated = false;

        // Incorrect password (same length)
        let incorrect_cmd = make_auth_cmd("secreT");
        assert_eq!(server.process_command(&incorrect_cmd, &mut regs, &mut mem), "E01");
        assert!(!server.authenticated);

        // Incorrect password (different length - shorter)
        let short_cmd = make_auth_cmd("short");
        assert_eq!(server.process_command(&short_cmd, &mut regs, &mut mem), "E01");
        assert!(!server.authenticated);

        // Incorrect password (different length - longer)
        let long_cmd = make_auth_cmd("secretlong");
        assert_eq!(server.process_command(&long_cmd, &mut regs, &mut mem), "E01");
        assert!(!server.authenticated);

        // Empty password
        let empty_cmd = make_auth_cmd("");
        assert_eq!(server.process_command(&empty_cmd, &mut regs, &mut mem), "E01");
        assert!(!server.authenticated);
    }

    #[test]
    fn test_constant_time_eq_logic() {
        assert!(constant_time_eq("secret", "secret"));
        assert!(!constant_time_eq("secret", "secreT"));
        assert!(!constant_time_eq("secret", "short"));
        assert!(!constant_time_eq("secret", "secretlong"));
        assert!(!constant_time_eq("", "secret"));
        assert!(!constant_time_eq("secret", ""));
        assert!(constant_time_eq("", ""));

        // Ensure it doesn't just check prefix
        assert!(!constant_time_eq("secret", "secreta"));
    }
}
