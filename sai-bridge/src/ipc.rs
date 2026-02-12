//! Unix socket IPC client to GameManager.
//!
//! No async runtime — this runs inside the engine's thread.
//! Uses blocking writes and non-blocking reads (polled each frame).

use crate::commands::GameCommand;
use crate::events::GameEvent;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

/// IPC connection to GameManager via Unix socket.
pub struct IpcClient {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    read_buf: String,
}

impl IpcClient {
    /// Connect to the GameManager's Unix socket.
    pub fn connect(path: &str) -> io::Result<Self> {
        let stream = UnixStream::connect(path)?;

        // Set non-blocking for reads (we poll each frame in EVENT_UPDATE)
        let reader_stream = stream.try_clone()?;
        reader_stream.set_nonblocking(true)?;

        // Writer stays blocking — sends are immediate and small
        stream.set_nonblocking(false)?;

        Ok(Self {
            writer: stream,
            reader: BufReader::new(reader_stream),
            read_buf: String::new(),
        })
    }

    /// Send a game event to GameManager.
    /// Format: one JSON object per line.
    pub fn send_event(&mut self, event: &GameEvent) -> io::Result<()> {
        let json = serde_json::to_string(event).map_err(|e| io::Error::other(e.to_string()))?;
        self.writer.write_all(json.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()
    }

    /// Poll for commands from GameManager (non-blocking).
    /// Returns any complete commands received since last poll.
    pub fn poll_commands(&mut self) -> Vec<GameCommand> {
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
        // Try a zero-byte write to check connection
        self.writer.try_clone().is_ok()
    }
}
