use anyhow::{anyhow, Result};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use log::{error, info, warn};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{protocol::Message, Error as WsError},
    MaybeTlsStream, WebSocketStream as TungsteniteStream,
};
use url::Url;

const PING_INTERVAL: Duration = Duration::from_secs(30);
const PING_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

pub struct WsStream {
    write: SplitSink<TungsteniteStream<MaybeTlsStream<TcpStream>>, Message>,
    read: SplitStream<TungsteniteStream<MaybeTlsStream<TcpStream>>>,
}

impl WsStream {
    pub async fn connect(url: &str) -> Result<Self> {
        let url = Url::parse(url)?;

        // Add connection timeout
        let connect_fut = connect_async(url);
        let (ws_stream, _) = match timeout(CONNECTION_TIMEOUT, connect_fut).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => return Err(anyhow!("WebSocket connection error: {}", e)),
            Err(_) => return Err(anyhow!("WebSocket connection timeout")),
        };

        let (write, read) = ws_stream.split();
        Ok(Self { write, read })
    }

    pub async fn send_message(&mut self, msg: Message) -> Result<()> {
        self.write
            .send(msg)
            .await
            .map_err(|e| anyhow!("Send error: {}", e))
    }

    pub async fn send_text(&mut self, text: String) -> Result<()> {
        self.send_message(Message::Text(text)).await
    }

    pub async fn read_message(&mut self) -> Result<Option<Message>> {
        match timeout(PING_INTERVAL, self.read.next()).await {
            Ok(Some(Ok(msg))) => {
                match msg {
                    Message::Ping(data) => {
                        // Automatically respond to pings
                        if let Err(e) = self.send_message(Message::Pong(data)).await {
                            warn!("Failed to send pong: {}", e);
                        }
                        Ok(None)
                    }
                    Message::Pong(_) => {
                        // Ignore pongs
                        Ok(None)
                    }
                    Message::Close(frame) => {
                        Err(anyhow!("WebSocket closed by server: {:?}", frame))
                    }
                    _ => Ok(Some(msg)),
                }
            }
            Ok(Some(Err(e))) => match e {
                WsError::Protocol(_) | WsError::Utf8 => {
                    warn!("WebSocket protocol error: {}", e);
                    Ok(None)
                }
                _ => Err(anyhow!("WebSocket error: {}", e)),
            },
            Ok(None) => Err(anyhow!("WebSocket stream ended")),
            Err(_) => {
                // Send ping on timeout
                if let Err(e) = self.send_message(Message::Ping(vec![])).await {
                    error!("Failed to send ping: {}", e);
                }

                // Wait for pong response
                match timeout(PING_TIMEOUT, self.read.next()).await {
                    Ok(Some(Ok(Message::Pong(_)))) => Ok(None),
                    _ => Err(anyhow!("WebSocket ping timeout")),
                }
            }
        }
    }

    pub async fn read_text(&mut self) -> Result<Option<String>> {
        while let Some(msg) = self.read_message().await? {
            if let Message::Text(text) = msg {
                return Ok(Some(text));
            }
        }
        Ok(None)
    }
}
