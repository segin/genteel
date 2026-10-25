//! GDB Remote Serial Protocol Server
//!
//! Implements a GDB stub for debugging M68k code running in the emulator.
//! Connect with: `m68k-elf-gdb -ex "target remote :1234"`

use std::collections::{HashSet, VecDeque};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};

/// Default GDB server port
pub const DEFAULT_PORT: u16 = 1234;

/// Maximum GDB packet size to prevent unbounded memory consumption.
/// Advertised to the client (in hex, as the protocol requires) via
/// `qSupported`; it bounds both received payloads and reply payloads.
pub const MAX_PACKET_SIZE: usize = 4096;

/// Largest `m` request that fits in one reply (two hex digits per byte).
pub const MAX_MEMORY_READ: usize = MAX_PACKET_SIZE / 2;

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Maximum number of breakpoints
pub const MAX_BREAKPOINTS: usize = 1024;

/// Maximum number of failed authentication attempts
pub const MAX_AUTH_ATTEMPTS: u32 = 10;

/// Duration for authentication lockout
pub const AUTH_LOCKOUT_DURATION: Duration = Duration::from_secs(10);

/// Maximum password length for constant-time comparison to prevent timing attacks
const MAX_PASSWORD_CHECK_LEN: usize = 128;

/// Constant-time string comparison to prevent timing attacks
///
/// This function executes in constant time to prevent timing attacks when checking passwords.
/// It iterates up to MAX_PASSWORD_CHECK_LEN to ensure timing is independent of string length.
#[cfg(test)]
fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();

    let mut result = Choice::from(1u8);

    // Check lengths in constant time
    result &= (a_bytes.len() as u64).ct_eq(&(b_bytes.len() as u64));

    for i in 0..MAX_PASSWORD_CHECK_LEN {
        // Use Choice::conditional_select to avoid branching on the length of a/b
        let in_a = Choice::from((i < a_bytes.len()) as u8);
        let in_b = Choice::from((i < b_bytes.len()) as u8);

        let a_byte = u8::conditional_select(&0, &a_bytes.get(i).copied().unwrap_or(0), in_a);
        let b_byte = u8::conditional_select(&0, &b_bytes.get(i).copied().unwrap_or(0), in_b);

        result &= a_byte.ct_eq(&b_byte);
    }

    // If either string is longer than MAX_PASSWORD_CHECK_LEN, it's not a match
    // (and we don't want to expose the actual length if it's large)
    result &= Choice::from((a_bytes.len() <= MAX_PASSWORD_CHECK_LEN) as u8);
    result &= Choice::from((b_bytes.len() <= MAX_PASSWORD_CHECK_LEN) as u8);

    result.unwrap_u8() == 1
}

/// GDB stop reasons
#[derive(Debug, Clone, Copy, PartialEq)]
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

    /// Pre-formatted GDB signal response string. This avoids allocation and formatting during the emulation loop.
    pub fn signal_string(&self) -> &'static str {
        match self {
            StopReason::Halted => "S05",
            StopReason::Breakpoint => "S05",
            StopReason::Step => "S05",
            StopReason::Interrupt => "S02",
        }
    }
}

/// Wrapper around TcpListener to allow mocking in tests
pub enum GdbListener {
    Real(TcpListener),
    #[cfg(test)]
    MockError,
}

impl GdbListener {
    pub fn accept(&self) -> std::io::Result<(TcpStream, std::net::SocketAddr)> {
        match self {
            Self::Real(l) => l.accept(),
            #[cfg(test)]
            Self::MockError => Err(std::io::Error::other("mock error")),
        }
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> std::io::Result<()> {
        match self {
            Self::Real(l) => l.set_nonblocking(nonblocking),
            #[cfg(test)]
            Self::MockError => Ok(()),
        }
    }

    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        match self {
            Self::Real(l) => l.local_addr(),
            #[cfg(test)]
            Self::MockError => Err(std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                "Mock local_addr error",
            )),
        }
    }
}

/// Events produced by the incremental packet parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseEvent {
    /// A complete packet with a valid checksum. The payload is raw bytes.
    Packet(Vec<u8>),
    /// A packet whose checksum did not match or was not two hex digits.
    BadChecksum,
    /// A packet whose payload exceeded `MAX_PACKET_SIZE`.
    Oversized,
    /// An out-of-band interrupt request (0x03).
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ParseState {
    #[default]
    Idle,
    Data,
    ChecksumHigh,
    ChecksumLow,
}

/// Incremental GDB Remote Serial Protocol framing parser.
///
/// Bytes may be fed in arbitrary chunks (split packets, several packets in
/// one segment); parser state is retained between calls so nothing is lost.
#[derive(Debug, Default)]
pub struct PacketParser {
    state: ParseState,
    data: Vec<u8>,
    checksum: u8,
    received_high: u8,
    oversized: bool,
}

fn hex_digit_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

impl PacketParser {
    /// Create an idle parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Discard any partially received packet.
    pub fn reset(&mut self) {
        self.state = ParseState::Idle;
        self.data.clear();
        self.checksum = 0;
        self.received_high = 0;
        self.oversized = false;
    }

    /// Feed one byte; returns an event when a frame completes.
    pub fn push(&mut self, byte: u8) -> Option<ParseEvent> {
        match self.state {
            ParseState::Idle => match byte {
                b'$' => {
                    self.reset();
                    self.state = ParseState::Data;
                    None
                }
                0x03 => Some(ParseEvent::Interrupt),
                /* ACK/NAK and stray bytes outside a packet are ignored. */
                _ => None,
            },
            ParseState::Data => {
                match byte {
                    b'#' => self.state = ParseState::ChecksumHigh,
                    b'$' => {
                        /* A new start marker resynchronises on the new packet. */
                        self.reset();
                        self.state = ParseState::Data;
                    }
                    _ => {
                        self.checksum = self.checksum.wrapping_add(byte);
                        if self.data.len() < MAX_PACKET_SIZE {
                            self.data.push(byte);
                        } else {
                            self.oversized = true;
                        }
                    }
                }
                None
            }
            ParseState::ChecksumHigh => match hex_digit_value(byte) {
                Some(v) => {
                    self.received_high = v;
                    self.state = ParseState::ChecksumLow;
                    None
                }
                None => {
                    self.reset();
                    Some(ParseEvent::BadChecksum)
                }
            },
            ParseState::ChecksumLow => {
                let event = match hex_digit_value(byte) {
                    Some(v) if ((self.received_high << 4) | v) == self.checksum => {
                        if self.oversized {
                            ParseEvent::Oversized
                        } else {
                            ParseEvent::Packet(std::mem::take(&mut self.data))
                        }
                    }
                    _ => ParseEvent::BadChecksum,
                };
                self.reset();
                Some(event)
            }
        }
    }
}

/// Run-control request decoded from the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunRequest {
    /// Resume execution (`c`, `vCont;c`).
    Continue,
    /// Execute one instruction (`s`, `vCont;s`).
    Step,
    /// Stop execution (0x03 break).
    Interrupt,
}

/// How long a reply may block on a slow reader before the client is dropped.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// GDB Server state
pub struct GdbServer {
    /// TCP listener
    listener: GdbListener,
    /// Connected client stream
    client: Option<TcpStream>,
    /// Incremental receive-side framing state (persists across polls)
    parser: PacketParser,
    /// Parsed events not yet consumed by `receive_packet`
    pending_events: VecDeque<ParseEvent>,
    /// Close the connection once the current reply has been sent
    close_after_reply: bool,
    /// Breakpoints (set of addresses)
    pub breakpoints: HashSet<u32>,
    /// Last stop reason
    pub stop_reason: StopReason,
    /// No-ack mode enabled
    no_ack_mode: bool,
    /// Optional password for authentication (plain text for monitor display)
    password: Option<String>,
    /// Pre-padded password bytes for constant-time comparison
    password_bytes: [u8; MAX_PASSWORD_CHECK_LEN],
    /// Actual length of the password
    password_len: usize,
    /// Whether the client is authenticated
    authenticated: bool,
    /// Number of failed authentication attempts
    auth_failed_attempts: u32,
    /// Timestamp until which authentication is locked out
    auth_lockout_until: Option<Instant>,
}

impl GdbServer {
    /// Create a new GDB server
    pub fn new(port: u16, password: Option<String>) -> std::io::Result<Self> {
        let listener = GdbListener::Real(TcpListener::bind(format!("127.0.0.1:{}", port))?);
        listener.set_nonblocking(true)?;

        let final_password = if let Some(pwd) = password {
            eprintln!(
                "🔒 GDB Server listening on 127.0.0.1:{}. Protected with password.",
                port
            );
            Some(pwd)
        } else {
            use rand::RngExt;
            let token = format!("{:032x}", rand::rng().random::<u128>());
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

        Ok(Self::with_listener(listener, final_password))
    }

    fn with_listener(listener: GdbListener, password: Option<String>) -> Self {
        let mut password_bytes = [0u8; MAX_PASSWORD_CHECK_LEN];
        let mut password_len = 0;
        if let Some(ref pwd) = password {
            let bytes = pwd.as_bytes();
            password_len = bytes.len();
            let copy_len = password_len.min(MAX_PASSWORD_CHECK_LEN);
            password_bytes[..copy_len].copy_from_slice(&bytes[..copy_len]);
        }

        Self {
            listener,
            client: None,
            parser: PacketParser::new(),
            pending_events: VecDeque::new(),
            close_after_reply: false,
            breakpoints: HashSet::new(),
            stop_reason: StopReason::Halted,
            no_ack_mode: false,
            password,
            password_bytes,
            password_len,
            /* Always start unauthenticated to enforce the token check. */
            authenticated: false,
            auth_failed_attempts: 0,
            auth_lockout_until: None,
        }
    }

    /// Reset all per-connection protocol state. Authentication lockout
    /// counters are deliberately kept so reconnecting cannot bypass them.
    fn reset_session(&mut self) {
        self.parser.reset();
        self.pending_events.clear();
        self.close_after_reply = false;
        self.breakpoints.clear();
        self.stop_reason = StopReason::Halted;
        self.no_ack_mode = false;
        self.authenticated = false;
    }

    /// Drop the current client (if any) and reset session state.
    pub fn disconnect(&mut self) {
        if let Some(client) = self.client.take() {
            let _ = client.shutdown(std::net::Shutdown::Both);
        }
        self.reset_session();
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
                if stream.set_nonblocking(true).is_err() {
                    return false;
                }
                let _ = stream.set_nodelay(true);
                self.reset_session();
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

    /// Write `bytes` completely to the client.
    ///
    /// The socket is non-blocking, so short writes and `WouldBlock` are
    /// retried until the whole buffer has been sent or `WRITE_TIMEOUT`
    /// expires. On failure the client is dropped, because a partially sent
    /// packet would desynchronise the stream.
    fn write_fully(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let Some(client) = self.client.as_mut() else {
            return Ok(());
        };
        let deadline = Instant::now() + WRITE_TIMEOUT;
        let mut written = 0;
        let result = loop {
            if written == bytes.len() {
                break client.flush();
            }
            match client.write(&bytes[written..]) {
                Ok(0) => break Err(std::io::Error::from(std::io::ErrorKind::WriteZero)),
                Ok(n) => written += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        break Err(std::io::Error::from(std::io::ErrorKind::TimedOut));
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(e) => break Err(e),
            }
        };
        if result.is_err() {
            self.disconnect();
        }
        result
    }

    /// Frame `data` as an RSP packet (`$data#cs`).
    pub fn frame_packet(data: &str) -> Vec<u8> {
        let checksum = data.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));
        let mut packet = Vec::with_capacity(data.len() + 4);
        packet.push(b'$');
        packet.extend_from_slice(data.as_bytes());
        packet.push(b'#');
        packet.push(HEX_DIGITS[(checksum >> 4) as usize]);
        packet.push(HEX_DIGITS[(checksum & 0xF) as usize]);
        packet
    }

    /// Send a packet to the client
    pub fn send_packet(&mut self, data: &str) -> std::io::Result<()> {
        let packet = Self::frame_packet(data);
        self.write_fully(&packet)
    }

    /// Send a command reply and then honour a pending detach/kill by
    /// closing the connection. An empty reply to a kill request is not sent.
    pub fn reply(&mut self, response: &str) -> std::io::Result<()> {
        let result = if response.is_empty() && self.close_after_reply {
            Ok(())
        } else {
            self.send_packet(response)
        };
        if self.close_after_reply {
            self.disconnect();
        }
        result
    }

    /// Record why the target stopped and send the stop reply packet.
    pub fn report_stop(&mut self, reason: StopReason) {
        self.stop_reason = reason;
        let _ = self.send_packet(reason.signal_string());
    }

    /// Read whatever bytes are available from the socket into the parser.
    fn fill_events(&mut self) {
        let mut chunk = [0u8; 1024];
        loop {
            let Some(client) = self.client.as_mut() else {
                return;
            };
            match client.read(&mut chunk) {
                Ok(0) => {
                    // Connection closed by peer
                    self.disconnect();
                    return;
                }
                Ok(n) => {
                    for &b in &chunk[..n] {
                        if let Some(ev) = self.parser.push(b) {
                            self.pending_events.push_back(ev);
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => return,
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => {
                    self.disconnect();
                    return;
                }
            }
        }
    }

    fn send_ack(&mut self, ok: bool) {
        if !self.no_ack_mode {
            let _ = self.write_fully(if ok { b"+" } else { b"-" });
        }
    }

    /// Receive a packet from the client (non-blocking).
    ///
    /// Returns `Some("INTERRUPT")` for an out-of-band break. Packets with a
    /// bad checksum are NAKed; oversized or non-ASCII packets are answered
    /// with `E01`. None of those are returned to the caller.
    pub fn receive_packet(&mut self) -> Option<String> {
        if self.pending_events.is_empty() {
            self.fill_events();
        }
        while let Some(event) = self.pending_events.pop_front() {
            match event {
                ParseEvent::Interrupt => {
                    self.stop_reason = StopReason::Interrupt;
                    return Some("INTERRUPT".to_string());
                }
                ParseEvent::BadChecksum => self.send_ack(false),
                ParseEvent::Oversized => {
                    eprintln!(
                        "⚠️  SECURITY ALERT: GDB packet exceeded maximum size of {}. Refusing.",
                        MAX_PACKET_SIZE
                    );
                    self.send_ack(true);
                    let _ = self.send_packet("E01");
                }
                ParseEvent::Packet(data) => {
                    self.send_ack(true);
                    if data.is_ascii() {
                        /* ASCII bytes are always valid UTF-8. */
                        return String::from_utf8(data).ok();
                    }
                    let _ = self.send_packet("E01");
                }
            }
        }
        None
    }

    /// Process every packet currently available, sending replies, and
    /// return the most recent run-control request (if any).
    pub fn service(
        &mut self,
        registers: &mut GdbRegisters,
        memory: &mut dyn GdbMemory,
    ) -> Option<RunRequest> {
        let mut request = None;
        while let Some(cmd) = self.receive_packet() {
            if cmd == "INTERRUPT" {
                request = Some(RunRequest::Interrupt);
                continue;
            }
            let response = self.process_command(&cmd, registers, memory);
            match response.as_str() {
                "CONTINUE" => request = Some(RunRequest::Continue),
                "STEP" => request = Some(RunRequest::Step),
                _ => {
                    let _ = self.reply(&response);
                }
            }
            if !self.is_connected() {
                break;
            }
        }
        request
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
                self.stop_reason.signal_string().to_string()
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
                // Detach: the connection is closed after the reply is sent.
                self.close_after_reply = true;
                "OK".to_string()
            }
            'k' => {
                // Kill: no reply; the connection is closed afterwards.
                self.close_after_reply = true;
                "".to_string()
            }
            _ => {
                if cmd == "INTERRUPT" {
                    StopReason::Interrupt.signal_string().to_string()
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
        /* Every handler below slices commands by byte offset; restricting the
         * input to ASCII guarantees those offsets are character boundaries. */
        if !cmd.is_ascii() {
            return "E01".to_string();
        }
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
        let mut parts = cmd.split('=');
        let (reg_part, val_part) = match (parts.next(), parts.next(), parts.next()) {
            (Some(r), Some(v), None) => (r, v),
            _ => return "E01".to_string(),
        };

        let reg_num = match u32::from_str_radix(reg_part, 16) {
            Ok(n) => n,
            Err(_) => return "E01".to_string(),
        };

        let value = match u32::from_str_radix(val_part, 16) {
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
        let mut parts = cmd.split(',');
        let (addr_part, len_part) = match (parts.next(), parts.next(), parts.next()) {
            (Some(a), Some(l), None) => (a, l),
            _ => return "E01".to_string(),
        };

        let addr = match u32::from_str_radix(addr_part, 16) {
            Ok(a) => a,
            Err(_) => return "E01".to_string(),
        };

        let len = match usize::from_str_radix(len_part, 16) {
            Ok(l) if l <= MAX_MEMORY_READ => l,
            _ => return "E01".to_string(),
        };

        let mut result = String::with_capacity(len * 2);
        for i in 0..len {
            let byte = memory.read_byte(addr.wrapping_add(i as u32));
            write!(result, "{:02x}", byte).unwrap();
        }

        result
    }

    /// Decode a string of hex digit pairs into bytes.
    fn decode_hex_bytes(hex: &str) -> Option<Vec<u8>> {
        let bytes = hex.as_bytes();
        if !bytes.len().is_multiple_of(2) {
            return None;
        }
        bytes
            .chunks_exact(2)
            .map(|pair| Some((hex_digit_value(pair[0])? << 4) | hex_digit_value(pair[1])?))
            .collect()
    }

    fn write_memory(&self, cmd: &str, memory: &mut dyn GdbMemory) -> String {
        let mut parts = cmd.split(':');
        let (addr_len_str, data) = match (parts.next(), parts.next(), parts.next()) {
            (Some(al), Some(d), None) => (al, d),
            _ => return "E01".to_string(),
        };

        let mut addr_len_parts = addr_len_str.split(',');
        let (addr_part, len_part) = match (
            addr_len_parts.next(),
            addr_len_parts.next(),
            addr_len_parts.next(),
        ) {
            (Some(a), Some(l), None) => (a, l),
            _ => return "E01".to_string(),
        };

        let addr = match u32::from_str_radix(addr_part, 16) {
            Ok(a) => a,
            Err(_) => return "E01".to_string(),
        };
        let Ok(len) = usize::from_str_radix(len_part, 16) else {
            return "E01".to_string();
        };

        /* Validate the whole payload before touching memory so a malformed
         * request has no partial effect. */
        let Some(bytes) = Self::decode_hex_bytes(data) else {
            return "E01".to_string();
        };
        if bytes.len() != len {
            return "E01".to_string();
        }

        for (i, byte) in bytes.into_iter().enumerate() {
            memory.write_byte(addr.wrapping_add(i as u32), byte);
        }

        "OK".to_string()
    }

    fn set_breakpoint(&mut self, cmd: &str) -> String {
        let mut parts = cmd.split(',');
        let (type_part, addr_part) = match (parts.next(), parts.next()) {
            (Some(t), Some(a)) => (t, a),
            _ => return "E01".to_string(),
        };

        // Type 0 = software breakpoint
        if type_part != "0" {
            return "".to_string(); // Not supported
        }

        let addr = match u32::from_str_radix(addr_part, 16) {
            Ok(a) => a,
            Err(_) => return "E01".to_string(),
        };

        if self.breakpoints.len() >= MAX_BREAKPOINTS && !self.breakpoints.contains(&addr) {
            eprintln!(
                "⚠️  SECURITY ALERT: Maximum number of breakpoints ({}) reached.",
                MAX_BREAKPOINTS
            );
            return "E01".to_string();
        }

        self.breakpoints.insert(addr);
        "OK".to_string()
    }

    fn remove_breakpoint(&mut self, cmd: &str) -> String {
        let mut parts = cmd.split(',');
        let (type_part, addr_part) = match (parts.next(), parts.next()) {
            (Some(t), Some(a)) => (t, a),
            _ => return "E01".to_string(),
        };

        if type_part != "0" {
            return "".to_string();
        }

        let addr = match u32::from_str_radix(addr_part, 16) {
            Ok(a) => a,
            Err(_) => return "E01".to_string(),
        };

        self.breakpoints.remove(&addr);
        "OK".to_string()
    }

    fn handle_query(&mut self, cmd: &str) -> String {
        if cmd.starts_with("qSupported") {
            // Report supported features
            format!("PacketSize={:x};swbreak+;QStartNoAckMode+", MAX_PACKET_SIZE)
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
        } else if cmd == "vCont?" {
            // Report supported vCont actions
            "vCont;c;s".to_string()
        } else if let Some(stripped) = cmd.strip_prefix("vCont;") {
            // Handle vCont command
            self.handle_vcont(stripped)
        } else if let Some(stripped) = cmd.strip_prefix("qRcmd,") {
            // Monitor command
            self.handle_monitor_command(stripped)
        } else {
            // Unknown query
            "".to_string()
        }
    }

    fn handle_vcont(&mut self, actions: &str) -> String {
        // Simple vCont implementation: handle first action
        // Format: action[:thread-id][;action[:thread-id]]...
        let first_action = actions.split(';').next().unwrap_or("");
        match first_action {
            "c" => "CONTINUE".to_string(),
            "s" => "STEP".to_string(),
            _ => {
                if first_action.starts_with('c') {
                    "CONTINUE".to_string()
                } else if first_action.starts_with('s') {
                    "STEP".to_string()
                } else {
                    "".to_string()
                }
            }
        }
    }

    fn decode_hex_command(&self, hex: &str) -> Option<String> {
        let bytes = Self::decode_hex_bytes(hex)?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn handle_auth_command(&mut self, provided_pass: &str) -> String {
        // Check for lockout
        if let Some(lockout_time) = self.auth_lockout_until {
            if Instant::now() < lockout_time {
                return "E01".to_string();
            } else {
                // Lockout expired
                self.auth_lockout_until = None;
                self.auth_failed_attempts = 0;
            }
        }

        if self.password.is_some() {
            let provided_bytes = provided_pass.trim().as_bytes();
            let mut matches = Choice::from(1u8);

            // Compare lengths in constant time
            matches &= (provided_bytes.len() as u64).ct_eq(&(self.password_len as u64));

            for i in 0..MAX_PASSWORD_CHECK_LEN {
                let in_provided = Choice::from((i < provided_bytes.len()) as u8);
                // Selection of the byte to compare is branching-free
                let b_provided = u8::conditional_select(
                    &0,
                    &provided_bytes.get(i).copied().unwrap_or(0),
                    in_provided,
                );
                let b_correct = self.password_bytes[i];

                matches &= b_provided.ct_eq(&b_correct);
            }

            // Also ensure provided password is not longer than our buffer
            matches &= Choice::from((provided_bytes.len() <= MAX_PASSWORD_CHECK_LEN) as u8);

            if matches.unwrap_u8() == 1 {
                self.authenticated = true;
                self.auth_failed_attempts = 0;
                self.auth_lockout_until = None;
                "OK".to_string()
            } else {
                self.auth_failed_attempts += 1;
                if self.auth_failed_attempts >= MAX_AUTH_ATTEMPTS {
                    self.auth_lockout_until = Some(Instant::now() + AUTH_LOCKOUT_DURATION);
                    eprintln!(
                        "⚠️  SECURITY ALERT: GDB authentication lockout active for {}s",
                        AUTH_LOCKOUT_DURATION.as_secs()
                    );
                }
                "E01".to_string() // Invalid password
            }
        } else {
            // No password set, already authenticated
            self.authenticated = true;
            "OK".to_string()
        }
    }

    fn handle_monitor_command(&mut self, cmd_hex: &str) -> String {
        let cmd = match self.decode_hex_command(cmd_hex) {
            Some(c) => c,
            None => return "E01".to_string(),
        };

        if let Some(stripped) = cmd.strip_prefix("auth ") {
            return self.handle_auth_command(stripped);
        }

        // All other commands require authentication
        if !self.authenticated {
            return "E01".to_string();
        }

        // Unknown monitor command.
        // We return "OK" to prevent the client from treating this as an error,
        // effectively ignoring unknown commands.
        eprintln!("ℹ️  Ignored unknown monitor command: {}", cmd);
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

    /// Get the local port the server is bound to
    pub fn port(&self) -> u16 {
        self.listener
            .local_addr()
            .expect("Failed to get local addr")
            .port()
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
        let mut server = GdbServer::with_listener(
            GdbListener::Real(TcpListener::bind("127.0.0.1:0").unwrap()),
            None,
        );
        server.authenticated = true;
        server
    }

    #[test]
    fn test_port_getter_success() {
        let server = create_test_server();
        let port = server.port();
        assert!(port > 0, "Port should be greater than 0");
    }

    #[test]
    #[should_panic(expected = "Failed to get local addr")]
    fn test_port_getter_error() {
        let mut server = create_test_server();
        server.listener = GdbListener::MockError;
        server.port();
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq("secret", "secret"));
        assert!(!constant_time_eq("secret", "secreT"));
        assert!(!constant_time_eq("secret", "secre"));
        assert!(!constant_time_eq("secret", "secret1"));
        assert!(!constant_time_eq("", "secret"));
        assert!(!constant_time_eq("secret", ""));
        assert!(constant_time_eq("", ""));
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
        assert_eq!(sr.signal_string(), "S05");
        assert_eq!(StopReason::Interrupt.signal_string(), "S02");
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
    fn test_gdb_server_new_bind_error() {
        // Bind a listener to a random port to keep it occupied
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind listener");
        let port = listener
            .local_addr()
            .expect("Failed to get local addr")
            .port();

        // Attempt to create a GdbServer on the same port, which should fail
        let result = GdbServer::new(port, None);
        assert!(
            result.is_err(),
            "Expected GdbServer::new to fail when port is already in use"
        );

        let err = result.err().unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
    }

    #[test]
    fn test_security_loopback_accepted() {
        // Bind to random port
        let mut server = GdbServer::new(0, None).expect("Failed to create GDB server");
        let port = server.port();

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
            format!("PacketSize={:x};swbreak+;QStartNoAckMode+", MAX_PACKET_SIZE);
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
    fn test_send_packet_write_failure() {
        let mut server = GdbServer::new(0, None).expect("Failed to create GDB server");
        let port = server
            .listener
            .local_addr()
            .expect("Failed to get local addr")
            .port();

        // Connect via loopback
        let client_stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Failed to connect");
        assert!(server.accept(), "Server should accept connection");
        assert!(server.is_connected(), "Server should be connected");

        // Forcefully close the client connection to simulate a dropped connection
        client_stream
            .shutdown(std::net::Shutdown::Both)
            .expect("Failed to shutdown client stream");

        // Give the OS a moment to propagate the shutdown
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Attempting to send a packet to a closed connection might buffer initially.
        // We write enough data to ensure the TCP buffer fills and an error is returned.
        let large_data = "A".repeat(100_000);
        let mut result = Ok(());

        // Retry a few times if the first write gets buffered
        for _ in 0..10 {
            result = server.send_packet(&large_data);
            if result.is_err() {
                break;
            }
        }

        assert!(
            result.is_err(),
            "Expected send_packet to return an error when the client is disconnected"
        );
    }

    #[test]
    fn test_receive_packet_robustness() {
        let mut server = GdbServer::new(0, None).expect("Failed to create GDB server");
        server.no_ack_mode = false;
        let port = server.port();

        // Connect via loopback
        let mut client_stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Failed to connect");
        assert!(server.accept(), "Server should accept connection");

        // 1. Test ignoring ACK/NAK (+/-) characters
        let packet_with_prefix = b"+-$OK#9a";
        client_stream.write_all(packet_with_prefix).unwrap();
        client_stream.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(server.receive_packet(), Some("OK".to_string()));

        // Read out the ACK (+) from the first message
        client_stream.set_nonblocking(true).unwrap();
        let mut response = [0u8; 10];
        let _ = client_stream.read(&mut response); // Clear the buffer
        client_stream.set_nonblocking(false).unwrap();

        // 2. Test invalid checksum
        // Send a valid packet body but an invalid checksum "00" instead of "9a"
        // Also ensure the packet starts cleanly with $
        let invalid_checksum_packet = b"$OK#00";
        client_stream.write_all(invalid_checksum_packet).unwrap();
        client_stream.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        // This should read the packet, see the invalid checksum, send '-', and return None
        assert_eq!(server.receive_packet(), None);

        // Read the NAK from the server
        client_stream.set_nonblocking(true).unwrap();
        let mut naks_found = false;
        let mut response = [0u8; 10];
        for _ in 0..50 {
            if let Ok(bytes_read) = client_stream.read(&mut response) {
                if bytes_read > 0 && response[..bytes_read].contains(&b'-') {
                    naks_found = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(naks_found, "Expected NAK (-) from server");

        // 3. Test receiving interrupt character (0x03)
        let interrupt_packet = [0x03];
        client_stream.write_all(&interrupt_packet).unwrap();
        client_stream.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(server.receive_packet(), Some("INTERRUPT".to_string()));
        assert_eq!(server.stop_reason, StopReason::Interrupt);

        // Read out anything left from interrupt
        client_stream.set_nonblocking(true).unwrap();
        let _ = client_stream.read(&mut response); // Clear the buffer
        client_stream.set_nonblocking(false).unwrap();

        // 4. Test improperly framed packet
        // Send a packet but cut the connection before the checksum
        let bad_frame_packet = b"$OK#";
        client_stream.write_all(bad_frame_packet).unwrap();
        client_stream.flush().unwrap();

        // Disconnect immediately
        client_stream.shutdown(std::net::Shutdown::Both).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        assert_eq!(server.receive_packet(), None);
    }

    #[test]
    fn test_receive_packet_write_ack_failure() {
        let mut server = GdbServer::new(0, None).expect("Failed to create GDB server");
        // Disable no_ack_mode so the server attempts to write an ACK (+)
        server.no_ack_mode = false;

        let port = server
            .listener
            .local_addr()
            .expect("Failed to get local addr")
            .port();

        // Connect via loopback
        let mut client_stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Failed to connect");
        assert!(server.accept(), "Server should accept connection");

        // We write a valid packet to the server, but then immediately shutdown
        // the client connection BEFORE the server has a chance to read it and
        // write the ACK back.
        // Format of valid packet: $OK#9a
        let packet = b"$OK#9a";
        client_stream
            .write_all(packet)
            .expect("Failed to write packet");
        client_stream.flush().expect("Failed to flush");

        // Forcefully close the client connection so the server's ACK write fails
        client_stream
            .shutdown(std::net::Shutdown::Both)
            .expect("Failed to shutdown client stream");

        // Give the OS a moment to propagate the shutdown
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Attempt to receive the packet.
        // The server will read the packet, see it's valid, and try to write an ACK (+).
        // Since the client is gone, writing the ACK fails, but the server just uses .ok()
        // and ignores the error, returning the parsed packet.
        // This test ensures that the .ok() correctly handles the failure without panicking.
        let result = server.receive_packet();

        // We might get None if the read fails due to the connection reset,
        // or Some("OK") if the read succeeded before the reset was processed.
        // Both are acceptable outcomes, but the key is that it shouldn't panic
        // when attempting to write the ACK to a closed stream.
        let _ = result;
    }

    #[test]
    fn test_oversized_packet_prevention() {
        let mut server = GdbServer::new(0, None).expect("Failed to create GDB server");
        let port = server.port();

        // Connect via loopback
        let mut client_stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Failed to connect");
        assert!(server.accept(), "Server should accept connection");

        // Send a very large but validly framed packet: $AAA...A#XX
        let large_size = 10000;
        let mut data = String::with_capacity(large_size);
        for _ in 0..large_size {
            data.push('A');
        }
        let checksum = data.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));
        let packet = format!("${}#{:02x}", data, checksum);

        client_stream
            .write_all(packet.as_bytes())
            .expect("Failed to write to server");
        client_stream.flush().expect("Failed to flush");

        // Try to receive the packet.
        let result = server.receive_packet();
        assert!(
            result.is_none(),
            "Should not return a valid packet for oversized input"
        );
        assert!(
            server.is_connected(),
            "Server should STAY connected after oversized packet"
        );

        // Verify that the server sent "E01"
        client_stream.set_nonblocking(false).unwrap();
        let mut response = [0u8; 8]; // +$E01#XX
        client_stream
            .read_exact(&mut response)
            .expect("Failed to read response from server");
        assert_eq!(&response[0..4], b"+$E0");
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
            generated_pwd.chars().all(|c| c.is_ascii_hexdigit()),
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
            format!("PacketSize={:x};swbreak+;QStartNoAckMode+", MAX_PACKET_SIZE);
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
    fn test_breakpoint_limit() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        // Fill up breakpoints
        for i in 0..MAX_BREAKPOINTS {
            let cmd = format!("Z0,{:x},4", i);
            assert_eq!(server.process_command(&cmd, &mut regs, &mut mem), "OK");
        }

        // Verify full
        assert_eq!(server.breakpoints.len(), MAX_BREAKPOINTS);

        // Try adding one more (new)
        let cmd_overflow = format!("Z0,{:x},4", MAX_BREAKPOINTS);
        assert_eq!(
            server.process_command(&cmd_overflow, &mut regs, &mut mem),
            "E01"
        );

        // Verify existing one still works (idempotent)
        let cmd_existing = "Z0,0,4";
        assert_eq!(
            server.process_command(cmd_existing, &mut regs, &mut mem),
            "OK"
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
    fn test_authentication_lockout() {
        let password = "secret".to_string();
        let mut server = GdbServer::new(0, Some(password)).unwrap();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();

        fn to_hex(s: &str) -> String {
            s.bytes().map(|b| format!("{:02x}", b)).collect()
        }

        let wrong_packet = format!("qRcmd,{}", to_hex("auth wrong"));
        let right_packet = format!("qRcmd,{}", to_hex("auth secret"));

        // 1. Fail MAX_AUTH_ATTEMPTS times
        for _ in 0..MAX_AUTH_ATTEMPTS {
            assert_eq!(
                server.process_command(&wrong_packet, &mut regs, &mut mem),
                "E01"
            );
            assert!(!server.authenticated);
        }

        // 2. Lockout should be active now. Even correct password should fail.
        assert!(server.auth_lockout_until.is_some());
        assert_eq!(
            server.process_command(&right_packet, &mut regs, &mut mem),
            "E01"
        );
        assert!(!server.authenticated);
    }

    #[test]
    fn test_constant_time_eq_extended() {
        // Equal strings
        assert!(constant_time_eq("password", "password"));
        assert!(constant_time_eq("", ""));
        assert!(constant_time_eq("123456", "123456"));

        // Unequal strings, same length
        assert!(!constant_time_eq("password", "passworf"));
        assert!(!constant_time_eq("123456", "123457"));
        assert!(!constant_time_eq("a", "b"));

        // Unequal strings, different length
        assert!(!constant_time_eq("password", "pass"));
        assert!(!constant_time_eq("pass", "password"));
        assert!(!constant_time_eq("", "a"));

        // Unicode
        assert!(constant_time_eq("p@sswöd", "p@sswöd"));
        assert!(!constant_time_eq("p@sswöd", "p@sswod"));
    }

    #[test]
    fn test_constant_time_eq_length_agnostic() {
        // Test passwords with same prefix but different lengths
        // "pass" vs "password"
        assert!(!constant_time_eq("pass", "password"));

        // "password" vs "pass"
        assert!(!constant_time_eq("password", "pass"));

        // "abc" vs "abc\0" (zero padding check)
        assert!(!constant_time_eq("abc", "abc\0"));

        // "abc\0" vs "abc"
        assert!(!constant_time_eq("abc\0", "abc"));

        // "abc" vs "abd"
        assert!(!constant_time_eq("abc", "abd"));
    }

    #[test]
    #[should_panic(expected = "Failed to get local addr")]
    fn test_local_addr_failure() {
        let mut server = create_test_server();
        // Replace listener with mock to simulate local_addr failure
        server.listener = GdbListener::MockError;

        // Triggering the failure on local_addr() after a successful bind
        let _port = server.port();
    }

    #[test]
    fn test_is_breakpoint() {
        let mut server = create_test_server();

        // Initially no breakpoints
        assert!(!server.is_breakpoint(0x1000));
        assert!(!server.is_breakpoint(0x2000));

        // Add a breakpoint directly
        server.breakpoints.insert(0x1000);

        // Verify it exists, and others do not
        assert!(server.is_breakpoint(0x1000));
        assert!(!server.is_breakpoint(0x2000));

        // Remove the breakpoint
        server.breakpoints.remove(&0x1000);

        // Verify it is gone
        assert!(!server.is_breakpoint(0x1000));
    }

    #[test]
    fn test_receive_packet_would_block() {
        let mut server = GdbServer::new(0, None).expect("Failed to create GDB server");
        let port = server.port();

        // Connect via loopback
        let _client_stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Failed to connect");
        assert!(server.accept(), "Server should accept connection");
        assert!(server.is_connected(), "Server should be connected");

        // The client hasn't sent any data.
        // The server's stream is set to non-blocking in accept().
        // receive_packet should return None and handle the WouldBlock error cleanly.
        let result = server.receive_packet();
        assert!(
            result.is_none(),
            "Expected None when no data is available to read"
        );
        assert!(
            server.is_connected(),
            "Server should remain connected after a WouldBlock"
        );
    }

    #[test]
    fn test_receive_packet_garbage_prefix() {
        let mut server = GdbServer::new(0, None).expect("Failed to create GDB server");
        // Disable no_ack_mode to test complete behavior
        server.no_ack_mode = false;
        let port = server.port();

        // Connect via loopback
        let mut client_stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Failed to connect");
        assert!(server.accept(), "Server should accept connection");

        // Send a valid packet prefixed with some garbage bytes and wait
        // The garbage should be ignored until '$' is hit.
        let packet_with_garbage = b"garbage data$OK#9a";
        client_stream.write_all(packet_with_garbage).unwrap();
        client_stream.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        assert_eq!(server.receive_packet(), Some("OK".to_string()));
    }

    #[test]
    fn test_receive_packet_broken_checksum() {
        let mut server = GdbServer::new(0, None).expect("Failed to create GDB server");
        // Disable no_ack_mode to test complete behavior
        server.no_ack_mode = false;
        let port = server.port();

        // Connect via loopback
        let mut client_stream =
            TcpStream::connect(format!("127.0.0.1:{}", port)).expect("Failed to connect");
        assert!(server.accept(), "Server should accept connection");

        // Send a valid packet start and data, but disconnect before sending the two-byte checksum
        let packet_incomplete = b"$OK#9";
        client_stream.write_all(packet_incomplete).unwrap();
        client_stream.flush().unwrap();
        client_stream.shutdown(std::net::Shutdown::Both).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        assert_eq!(server.receive_packet(), None);
    }

    #[test]
    fn test_constant_time_eq_robustness() {
        // Test strings of varying lengths
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(!constant_time_eq("abcd", "abc"));

        // Test empty strings
        assert!(constant_time_eq("", ""));
        assert!(!constant_time_eq("", "a"));
        assert!(!constant_time_eq("a", ""));

        // Test strings at MAX_PASSWORD_CHECK_LEN
        let long_str = "A".repeat(MAX_PASSWORD_CHECK_LEN);
        let long_str_copy = "A".repeat(MAX_PASSWORD_CHECK_LEN);
        let long_str_diff = "A".repeat(MAX_PASSWORD_CHECK_LEN - 1) + "B";

        assert!(constant_time_eq(&long_str, &long_str_copy));
        assert!(!constant_time_eq(&long_str, &long_str_diff));

        // Test strings exceeding MAX_PASSWORD_CHECK_LEN
        let too_long = "A".repeat(MAX_PASSWORD_CHECK_LEN + 1);
        let too_long_copy = "A".repeat(MAX_PASSWORD_CHECK_LEN + 1);

        // Should return false even if they are equal, because they exceed the checked length
        assert!(!constant_time_eq(&too_long, &too_long_copy));
    }

    fn feed_all(parser: &mut PacketParser, bytes: &[u8]) -> Vec<ParseEvent> {
        bytes.iter().filter_map(|&b| parser.push(b)).collect()
    }

    fn connected_pair(password: Option<String>) -> (GdbServer, TcpStream) {
        let mut server = GdbServer::new(0, password).expect("server");
        let client = TcpStream::connect(format!("127.0.0.1:{}", server.port())).expect("connect");
        for _ in 0..100 {
            if server.accept() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(server.is_connected());
        (server, client)
    }

    fn receive_with_retry(server: &mut GdbServer) -> Option<String> {
        for _ in 0..100 {
            if let Some(p) = server.receive_packet() {
                return Some(p);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }

    fn service_until<F>(server: &mut GdbServer, mut done: F) -> Option<RunRequest>
    where
        F: FnMut(&GdbServer, Option<RunRequest>) -> bool,
    {
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();
        let mut req = None;
        for _ in 0..200 {
            req = server.service(&mut regs, &mut mem);
            if done(server, req) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        req
    }

    #[test]
    fn test_parser_two_packets_in_one_chunk() {
        let mut parser = PacketParser::new();
        let events = feed_all(&mut parser, b"$g#67$?#3f");
        assert_eq!(
            events,
            vec![
                ParseEvent::Packet(b"g".to_vec()),
                ParseEvent::Packet(b"?".to_vec())
            ]
        );
    }

    #[test]
    fn test_parser_reassembles_split_packet() {
        let mut parser = PacketParser::new();
        assert!(feed_all(&mut parser, b"$O").is_empty());
        assert!(feed_all(&mut parser, b"K#9").is_empty());
        assert_eq!(
            feed_all(&mut parser, b"a"),
            vec![ParseEvent::Packet(b"OK".to_vec())]
        );
    }

    #[test]
    fn test_parser_rejects_non_hex_checksum() {
        let mut parser = PacketParser::new();
        /* "zz" used to parse as checksum 0; a payload summing to 0 must not pass. */
        assert_eq!(
            feed_all(&mut parser, b"$#zz"),
            vec![ParseEvent::BadChecksum]
        );
        assert_eq!(
            feed_all(&mut parser, b"$#0z"),
            vec![ParseEvent::BadChecksum]
        );
        assert_eq!(
            feed_all(&mut parser, b"$#00"),
            vec![ParseEvent::Packet(Vec::new())]
        );
        assert_eq!(
            feed_all(&mut parser, b"$OK#00"),
            vec![ParseEvent::BadChecksum]
        );
    }

    #[test]
    fn test_parser_interrupt_between_packets() {
        let mut parser = PacketParser::new();
        assert_eq!(
            feed_all(&mut parser, b"+\x03$c#63"),
            vec![ParseEvent::Interrupt, ParseEvent::Packet(b"c".to_vec())]
        );
    }

    #[test]
    fn test_parser_oversized_packet_reports_and_resyncs() {
        let mut parser = PacketParser::new();
        let payload = vec![b'A'; MAX_PACKET_SIZE + 10];
        let sum = payload.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        let mut bytes = vec![b'$'];
        bytes.extend_from_slice(&payload);
        bytes.extend_from_slice(format!("#{:02x}$OK#9a", sum).as_bytes());
        assert_eq!(
            feed_all(&mut parser, &bytes),
            vec![ParseEvent::Oversized, ParseEvent::Packet(b"OK".to_vec())]
        );
    }

    #[test]
    fn test_non_ascii_packet_is_rejected_without_panic() {
        let mut server = GdbServer::new(0, Some("pw".to_string())).unwrap();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();
        /* Reachable before authentication. */
        assert_eq!(
            server.process_command("qRcmd,a\u{e9}b", &mut regs, &mut mem),
            "E01"
        );
        server.authenticated = true;
        let g_cmd = format!("G{}\u{e9}", "0".repeat(150));
        assert_eq!(server.process_command(&g_cmd, &mut regs, &mut mem), "E01");
        assert_eq!(
            server.process_command("M0,2:\u{e9}", &mut regs, &mut mem),
            "E01"
        );
    }

    #[test]
    fn test_non_ascii_packet_over_socket_gets_error_reply() {
        let (mut server, mut client) = connected_pair(None);
        let payload: &[u8] = b"qRcmd,a\xC3b";
        let sum = payload.iter().fold(0u8, |a, &b| a.wrapping_add(b));
        let mut bytes = vec![b'$'];
        bytes.extend_from_slice(payload);
        bytes.extend_from_slice(format!("#{:02x}", sum).as_bytes());
        client.write_all(&bytes).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(server.receive_packet(), None);
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buf = [0u8; 8];
        client.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"+$E01#a6");
    }

    #[test]
    fn test_memory_read_length_is_capped() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();
        assert_eq!(
            server.process_command("m0,ffffffffffffffff", &mut regs, &mut mem),
            "E01"
        );
        assert_eq!(
            server.process_command("m0,7fffffff", &mut regs, &mut mem),
            "E01"
        );
        let max = format!("m0,{:x}", MAX_MEMORY_READ);
        assert_eq!(
            server.process_command(&max, &mut regs, &mut mem).len(),
            MAX_PACKET_SIZE
        );
        let over = format!("m0,{:x}", MAX_MEMORY_READ + 1);
        assert_eq!(server.process_command(&over, &mut regs, &mut mem), "E01");
    }

    #[test]
    fn test_packet_size_is_advertised_in_hex() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();
        let reply = server.process_command("qSupported", &mut regs, &mut mem);
        assert!(reply.starts_with("PacketSize=1000;"), "{}", reply);
    }

    #[test]
    fn test_write_memory_length_mismatch_is_rejected() {
        let mut server = create_test_server();
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();
        assert_eq!(
            server.process_command("M100,4:aabb", &mut regs, &mut mem),
            "E01"
        );
        assert!(mem.data.is_empty(), "rejected write must not modify memory");
    }

    #[test]
    fn test_second_connection_requires_authentication() {
        let (mut server, client) = connected_pair(Some("secret".to_string()));
        let mut regs = GdbRegisters::default();
        let mut mem = MockMemory::new();
        let auth: String = "auth secret"
            .bytes()
            .map(|b| format!("{:02x}", b))
            .collect();
        let auth_cmd = format!("qRcmd,{}", auth);
        assert_eq!(server.process_command(&auth_cmd, &mut regs, &mut mem), "OK");
        assert_eq!(
            server.process_command("QStartNoAckMode", &mut regs, &mut mem),
            "OK"
        );
        server.stop_reason = StopReason::Breakpoint;
        server.breakpoints.insert(0x1234);
        drop(client);
        for _ in 0..100 {
            server.receive_packet();
            if !server.is_connected() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!server.is_connected());

        let _client2 = TcpStream::connect(format!("127.0.0.1:{}", server.port())).unwrap();
        for _ in 0..100 {
            if server.accept() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(server.is_connected());
        assert!(!server.authenticated);
        assert!(!server.no_ack_mode);
        assert_eq!(server.stop_reason, StopReason::Halted);
        assert!(server.breakpoints.is_empty());
        assert_eq!(server.process_command("g", &mut regs, &mut mem), "E01");
    }

    #[test]
    fn test_pipelined_packets_are_all_processed() {
        let (mut server, mut client) = connected_pair(None);
        client.write_all(b"$g#67$?#3f\x03").unwrap();
        assert_eq!(receive_with_retry(&mut server), Some("g".to_string()));
        assert_eq!(server.receive_packet(), Some("?".to_string()));
        assert_eq!(server.receive_packet(), Some("INTERRUPT".to_string()));
    }

    #[test]
    fn test_split_packet_over_socket_is_reassembled() {
        let (mut server, mut client) = connected_pair(None);
        client.write_all(b"$O").unwrap();
        client.flush().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(server.receive_packet(), None);
        assert!(server.is_connected());
        client.write_all(b"K#9a").unwrap();
        assert_eq!(receive_with_retry(&mut server), Some("OK".to_string()));
    }

    #[test]
    fn test_reply_is_complete_for_slow_reader() {
        let (mut server, client) = connected_pair(None);
        /* Far larger than the socket buffers, so the non-blocking writer
         * must wait for the reader instead of failing mid-packet. */
        let data: String = (0..8 * 1024 * 1024)
            .map(|i| (b'a' + (i % 26) as u8) as char)
            .collect();
        let expected = GdbServer::frame_packet(&data);
        let expected_len = expected.len();
        let reader = std::thread::spawn(move || {
            let mut client = client;
            std::thread::sleep(Duration::from_millis(100));
            let mut received = Vec::with_capacity(expected_len);
            let mut chunk = [0u8; 65536];
            while received.len() < expected_len {
                std::thread::sleep(Duration::from_micros(200));
                let n = client.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                received.extend_from_slice(&chunk[..n]);
            }
            received
        });
        server.send_packet(&data).expect("complete send");
        let received = reader.join().unwrap();
        assert_eq!(received.len(), expected.len());
        assert!(received == expected, "reply corrupted");
    }

    #[test]
    fn test_detach_reply_is_sent_before_close() {
        let (mut server, mut client) = connected_pair(None);
        client.write_all(b"$D#44").unwrap();
        service_until(&mut server, |s, _| !s.is_connected());
        assert!(!server.is_connected());
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut received = Vec::new();
        client.read_to_end(&mut received).unwrap();
        assert_eq!(received, b"+$OK#9a");
    }

    #[test]
    fn test_kill_closes_connection() {
        let (mut server, mut client) = connected_pair(None);
        client.write_all(b"$k#6b").unwrap();
        service_until(&mut server, |s, _| !s.is_connected());
        assert!(!server.is_connected());
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut received = Vec::new();
        client.read_to_end(&mut received).unwrap();
        assert_eq!(received, b"+");
    }

    #[test]
    fn test_service_reports_run_requests() {
        let (mut server, mut client) = connected_pair(None);
        server.authenticated = true;
        client.write_all(b"$c#63").unwrap();
        let req = service_until(&mut server, |_, r| r.is_some());
        assert_eq!(req, Some(RunRequest::Continue));
        client.write_all(b"\x03").unwrap();
        let req = service_until(&mut server, |_, r| r.is_some());
        assert_eq!(req, Some(RunRequest::Interrupt));
    }

    #[test]
    fn test_server_binds_loopback_only() {
        let server = GdbServer::new(0, None).unwrap();
        let addr = server.listener.local_addr().unwrap();
        assert!(addr.ip().is_loopback());
    }

    mod fuzz {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn parser_never_panics_and_is_chunking_invariant(
                bytes in proptest::collection::vec(any::<u8>(), 0..2048),
                split in any::<usize>(),
            ) {
                let mut whole = PacketParser::new();
                let all = feed_all(&mut whole, &bytes);
                let cut = if bytes.is_empty() { 0 } else { split % bytes.len() };
                let mut chunked = PacketParser::new();
                let mut parts = feed_all(&mut chunked, &bytes[..cut]);
                parts.extend(feed_all(&mut chunked, &bytes[cut..]));
                prop_assert_eq!(all, parts);
            }

            #[test]
            fn process_command_never_panics(
                bytes in proptest::collection::vec(any::<u8>(), 0..512),
                authenticated in any::<bool>(),
            ) {
                let mut server = create_test_server();
                server.authenticated = authenticated;
                let mut regs = GdbRegisters::default();
                let mut mem = MockMemory::new();
                let cmd = String::from_utf8_lossy(&bytes);
                let _ = server.process_command(&cmd, &mut regs, &mut mem);
            }

            #[test]
            fn process_command_structured_inputs_never_panic(
                prefix in prop::sample::select(vec![
                    "m", "M", "g", "G", "p", "P", "Z0,", "z0,", "qRcmd,", "vCont;", "H", "q", "Q"
                ]),
                body in "[0-9a-fA-F,:;=\u{80}-\u{ff}]{0,200}",
            ) {
                let mut server = create_test_server();
                let mut regs = GdbRegisters::default();
                let mut mem = MockMemory::new();
                let cmd = format!("{}{}", prefix, body);
                let _ = server.process_command(&cmd, &mut regs, &mut mem);
            }
        }
    }
}
