use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use super::protocol::LobbyMessage;

#[derive(Debug, thiserror::Error)]
pub enum LobbyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Connection closed")]
    Closed,
    #[error("Login failed: {0}")]
    LoginFailed(String),
}

/// TCP connection to the ZK lobby server.
pub struct LobbyConnection {
    writer: tokio::io::WriteHalf<TcpStream>,
    reader: BufReader<tokio::io::ReadHalf<TcpStream>>,
}

impl LobbyConnection {
    /// Connect to a lobby server.
    pub async fn connect(host: &str, port: u16) -> Result<Self, LobbyError> {
        let addr = format!("{}:{}", host, port);
        tracing::info!("Connecting to lobby server at {}", addr);
        let stream = TcpStream::connect(&addr).await?;
        let (reader, writer) = tokio::io::split(stream);
        Ok(Self {
            writer,
            reader: BufReader::new(reader),
        })
    }

    /// Send a lobby message.
    pub async fn send(&mut self, msg: &LobbyMessage) -> Result<(), LobbyError> {
        let wire = msg.to_wire();
        tracing::debug!("→ {}", wire.trim());
        self.writer.write_all(wire.as_bytes()).await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Send a typed command with JSON data.
    pub async fn send_command(
        &mut self,
        command: &str,
        data: &impl serde::Serialize,
    ) -> Result<(), LobbyError> {
        let msg = LobbyMessage::new(command, serde_json::to_value(data).unwrap());
        self.send(&msg).await
    }

    /// Read the next message from the lobby server.
    /// Returns None on clean disconnect.
    pub async fn recv(&mut self) -> Result<LobbyMessage, LobbyError> {
        loop {
            let mut line = String::new();
            let bytes = self.reader.read_line(&mut line).await?;
            if bytes == 0 {
                return Err(LobbyError::Closed);
            }
            if let Some(msg) = LobbyMessage::from_line(&line) {
                tracing::debug!("← {} {}", msg.command, &msg.data.to_string()[..msg.data.to_string().len().min(200)]);
                return Ok(msg);
            }
        }
    }
}
