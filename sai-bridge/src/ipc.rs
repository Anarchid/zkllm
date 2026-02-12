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
        })
    }

    /// Send a game event to GameManager.
    /// Temporarily switches to blocking mode for the write.
    pub fn send_event(&mut self, event: &GameEvent) -> io::Result<()> {
        let json = serde_json::to_string(event).map_err(|e| io::Error::other(e.to_string()))?;

        // Briefly switch to blocking for reliable write
        self.stream.set_nonblocking(false)?;
        self.stream.write_all(json.as_bytes())?;
        self.stream.write_all(b"\n")?;
        self.stream.flush()?;
        self.stream.set_nonblocking(true)?;

        Ok(())
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
        self.stream.try_clone().is_ok()
    }
}
