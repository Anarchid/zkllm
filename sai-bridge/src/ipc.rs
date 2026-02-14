//! Unix socket IPC client to GameManager.
//!
//! No async runtime — this runs inside the engine's thread.
//! Uses non-blocking mode with temporary blocking for writes.
//!
//! Note: `UnixStream::try_clone()` creates a new FD pointing to the same
//! socket description. `set_nonblocking()` operates on the description, not
//! the FD — so setting blocking on one clone affects the other. We use a
//! single stream and toggle between blocking/non-blocking as needed.

use crate::commands::GameCommand;
use crate::events::GameEvent;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

/// IPC connection to GameManager via Unix socket.
pub struct IpcClient {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    read_buf: String,
    /// Outbound buffer for events that couldn't be written immediately.
    write_buf: Vec<u8>,
}

impl IpcClient {
    /// Connect to the GameManager's Unix socket.
    pub fn connect(path: &str) -> io::Result<Self> {
        let stream = UnixStream::connect(path)?;
        let reader_stream = stream.try_clone()?;

        // Start in non-blocking mode (poll_commands is called every frame)
        stream.set_nonblocking(true)?;

        Ok(Self {
            stream,
            reader: BufReader::new(reader_stream),
            read_buf: String::new(),
            write_buf: Vec::new(),
        })
    }

    /// Send a game event to GameManager (non-blocking).
    /// Appends to an internal buffer and drains as much as the socket will accept.
    /// Never blocks the engine thread — drops oldest data if buffer exceeds 256KB.
    pub fn send_event(&mut self, event: &GameEvent) -> io::Result<()> {
        let json = serde_json::to_string(event).map_err(|e| io::Error::other(e.to_string()))?;
        self.write_buf.extend_from_slice(json.as_bytes());
        self.write_buf.push(b'\n');

        // Cap buffer at 256KB — if downstream is that far behind, drop oldest data
        const MAX_BUF: usize = 256 * 1024;
        if self.write_buf.len() > MAX_BUF {
            let drop = self.write_buf.len() - MAX_BUF;
            self.write_buf.drain(..drop);
        }

        self.flush_write_buf();
        Ok(())
    }

    /// Try to drain the write buffer without blocking.
    /// Called from send_event and poll_commands.
    fn flush_write_buf(&mut self) {
        while !self.write_buf.is_empty() {
            match self.stream.write(&self.write_buf) {
                Ok(0) => break, // socket closed
                Ok(n) => {
                    self.write_buf.drain(..n);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
    }

    /// Poll for commands from GameManager (non-blocking).
    /// Returns any complete commands received since last poll.
    /// Also drains the outbound write buffer.
    pub fn poll_commands(&mut self) -> Vec<GameCommand> {
        // Opportunistically flush pending writes
        self.flush_write_buf();

        let mut commands = Vec::new();

        loop {
            self.read_buf.clear();
            match self.reader.read_line(&mut self.read_buf) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let trimmed = self.read_buf.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<GameCommand>(trimmed) {
                        Ok(cmd) => commands.push(cmd),
                        Err(e) => {
                            eprintln!("[SAI] Failed to parse command: {} — {:?}", e, trimmed);
                        }
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    eprintln!("[SAI] IPC read error: {}", e);
                    break;
                }
            }
        }

        commands
    }

    /// Check if the connection is still alive.
    pub fn is_connected(&self) -> bool {
        self.stream.try_clone().is_ok()
    }
}
