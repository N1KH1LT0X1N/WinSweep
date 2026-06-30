//! Cross-privilege named pipe IPC
//!
//! This module implements secure named pipe communication between
//! the GUI (unprivileged) and scanner (elevated) processes.

use anyhow::{Context, Result};
use std::io;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex};

use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer};
use tracing::{debug, error, info, warn};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR};
use windows::Win32::Storage::FileSystem::{FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_OVERLAPPED};
use windows::Win32::System::Memory::{GetProcessHeap, HeapFree, HEAP_FLAGS};
use windows::Win32::System::Pipes::{
    CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
const SDDL_REVISION_1: u32 = 1;
const PIPE_ACCESS_DUPLEX: u32 = 0x00000003;
use winsweep_common::types::IpcMessage;

/// Named pipe name for WinSweep IPC
const PIPE_NAME: &str = r"\\.\pipe\WinSweepIPC";

/// SDDL for the elevated IPC pipe DACL.
///
/// Least-privilege: grant GENERIC_ALL only to LocalSystem (`SY`), the local
/// Administrators group (`BA`), and the creating/owner principal (`OW`). The
/// previous value (`D:(A;;GA;;;AU)`) granted full access to *all* Authenticated
/// Users, needlessly widening the attack surface on this cross-privilege channel.
const PIPE_SDDL: &str = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)";

/// IPC server for elevated scanner process
pub struct IpcServer {
    server: Arc<Mutex<NamedPipeServer>>,
    message_sender: mpsc::UnboundedSender<IpcMessage>,
    message_receiver: Arc<Mutex<mpsc::UnboundedReceiver<IpcMessage>>>,
    /// Sender side of the channel for incoming (client → server) messages.
    incoming_tx: mpsc::UnboundedSender<IpcMessage>,
    /// Receiver side exposed to callers via [`IpcServer::incoming_receiver`].
    incoming_rx: Arc<Mutex<mpsc::UnboundedReceiver<IpcMessage>>>,
}

/// IPC client for GUI process
pub struct IpcClient {
    client: Arc<Mutex<Option<NamedPipeClient>>>,
    /// Outgoing message queue sender (used by `start_message_loop`).
    outgoing_tx: mpsc::UnboundedSender<IpcMessage>,
    message_receiver: Arc<Mutex<mpsc::UnboundedReceiver<IpcMessage>>>,
}

impl IpcServer {
    /// Create a new IPC server
    pub async fn new() -> Result<Self> {
        info!("Creating IPC server for elevated process");

        // Create security attributes with proper DACL
        let security_attributes = create_pipe_security_attributes()
            .context("Failed to create pipe security attributes")?;

        // Create the named pipe using Windows API with security attributes
        let pipe_handle = create_named_pipe_with_security(&security_attributes.attributes)
            .context("Failed to create named pipe with security")?;

        // Convert to tokio NamedPipeServer
        let raw_handle = pipe_handle.0 as *mut std::ffi::c_void;
        let server = unsafe { NamedPipeServer::from_raw_handle(raw_handle) }.map_err(|e| {
            anyhow::anyhow!("Failed to create NamedPipeServer from raw handle: {}", e)
        })?;

        let (tx, rx) = mpsc::unbounded_channel();
        let receiver = Arc::new(Mutex::new(rx));

        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
        let incoming_rx = Arc::new(Mutex::new(incoming_rx));

        let server = Arc::new(Mutex::new(server));

        Ok(Self {
            server,
            message_sender: tx,
            message_receiver: receiver,
            incoming_tx,
            incoming_rx,
        })
    }

    /// Borrow the receiver for incoming messages (client → server).
    ///
    /// Callers should poll this channel to react to `StartScan`, `CleanupItems`,
    /// and other application-level messages forwarded by the server loop.
    pub fn incoming_receiver(&self) -> Arc<Mutex<mpsc::UnboundedReceiver<IpcMessage>>> {
        self.incoming_rx.clone()
    }

    /// Start accepting connections and processing messages
    pub async fn run(&self) -> Result<()> {
        info!("Starting IPC server message loop");

        // Clone references for the task
        let server = self.server.clone();
        let receiver = self.message_receiver.clone();

        // Spawn task to handle outgoing messages
        let sender_task = tokio::spawn(async move {
            while let Some(message) = receiver.lock().await.recv().await {
                debug!(
                    "Sending IPC message: {:?}",
                    std::mem::discriminant(&message)
                );

                let mut server = server.lock().await;
                match send_message(&mut *server, &message).await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Failed to send IPC message: {}", e);
                        break;
                    }
                }
            }
        });

        // Handle incoming messages
        loop {
            let message = {
                let mut server = self.server.lock().await;
                match receive_message(&mut *server).await {
                    Ok(Some(msg)) => msg,
                    Ok(None) => {
                        warn!("Client disconnected");
                        break;
                    }
                    Err(e) => {
                        error!("Error receiving message: {}", e);
                        break;
                    }
                }
            };

            debug!(
                "Received IPC message: {:?}",
                std::mem::discriminant(&message)
            );

            // Handle ping/pong automatically; forward everything else to the
            // application-level incoming channel so callers can act on it.
            match message {
                IpcMessage::Ping => {
                    let mut server = self.server.lock().await;
                    if let Err(e) = send_message(&mut *server, &IpcMessage::Pong).await {
                        error!("Failed to send Pong: {}", e);
                    }
                }
                other => {
                    if let Err(e) = self.incoming_tx.send(other) {
                        warn!("Failed to forward incoming IPC message: {}", e);
                    }
                }
            }
        }

        sender_task.abort();
        Ok(())
    }

    /// Send a message to the connected client
    pub async fn send(&self, message: IpcMessage) -> Result<()> {
        self.message_sender
            .send(message)
            .context("Failed to queue message for sending")?;
        Ok(())
    }
}

impl IpcClient {
    /// Create a new IPC client
    pub async fn new() -> Result<Self> {
        debug!("Creating IPC client for GUI process");

        let (tx, rx) = mpsc::unbounded_channel();
        let receiver = Arc::new(Mutex::new(rx));

        Ok(Self {
            client: Arc::new(Mutex::new(None)),
            outgoing_tx: tx,
            message_receiver: receiver,
        })
    }

    /// Connect to the IPC server
    pub async fn connect(&self) -> Result<()> {
        info!("Connecting to IPC server");

        let client = ClientOptions::new()
            .open(PIPE_NAME)
            .context("Failed to connect to named pipe")?;

        *self.client.lock().await = Some(client);

        // Start message handling task
        self.start_message_loop().await?;

        Ok(())
    }

    /// Send a message to the server.
    ///
    /// The message is queued on the outgoing channel and sent by the background
    /// writer task started by [`Self::connect`].
    pub async fn send(&self, message: IpcMessage) -> Result<()> {
        self.outgoing_tx
            .send(message)
            .context("Failed to queue outgoing IPC message")?;
        Ok(())
    }

    /// Receive a message from the server
    pub async fn receive(&self) -> Result<Option<IpcMessage>> {
        let mut client_guard = self.client.lock().await;

        if let Some(ref mut client) = *client_guard {
            receive_message(client).await
        } else {
            Err(anyhow::anyhow!("Not connected to IPC server"))
        }
    }

    /// Start the message handling loop
    async fn start_message_loop(&self) -> Result<()> {
        let client = self.client.clone();
        let receiver = self.message_receiver.clone();

        tokio::spawn(async move {
            loop {
                let message = {
                    let mut receiver = receiver.lock().await;
                    receiver.recv().await
                };
                if let Some(message) = message {
                    let mut client_guard = client.lock().await;
                    if let Some(ref mut client) = *client_guard {
                        if let Err(e) = send_message(client, &message).await {
                            error!("Failed to send message: {}", e);
                            break;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        });

        Ok(())
    }
}

/// Security attributes holder for named pipe
struct PipeSecurityAttributes {
    _security_descriptor: Box<SECURITY_DESCRIPTOR>,
    attributes: SECURITY_ATTRIBUTES,
}

/// Create a named pipe with security attributes
fn create_named_pipe_with_security(security_attributes: &SECURITY_ATTRIBUTES) -> Result<HANDLE> {
    let pipe_name_wide = to_wide(Path::new(PIPE_NAME));

    unsafe {
        let handle = CreateNamedPipeW(
            PCWSTR(pipe_name_wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED.0),
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            4096, // Out buffer size
            4096, // In buffer size
            0,    // Default timeout
            Some(security_attributes),
        );

        if handle == INVALID_HANDLE_VALUE {
            let error = GetLastError();
            return Err(anyhow::anyhow!(
                "CreateNamedPipeW failed: error {:?}",
                error
            ));
        }

        Ok(handle)
    }
}

/// Convert a Rust path to a wide string for Windows API
fn to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Create security attributes for the named pipe
fn create_pipe_security_attributes() -> Result<PipeSecurityAttributes> {
    let mut sd_ptr = PSECURITY_DESCRIPTOR(std::ptr::null_mut());

    // Convert SDDL to security descriptor
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(
                PIPE_SDDL
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect::<Vec<u16>>()
                    .as_ptr(),
            ),
            SDDL_REVISION_1,
            &mut sd_ptr,
            None,
        )
        .map_err(|_| anyhow::anyhow!("Failed to convert SDDL to security descriptor"))?;

        // Get the size of the security descriptor
        let sd_size = windows::Win32::Security::GetSecurityDescriptorLength(sd_ptr);

        // Allocate a box to hold the security descriptor
        let mut security_descriptor = Box::new(SECURITY_DESCRIPTOR::default());

        // Copy the security descriptor into our box
        std::ptr::copy_nonoverlapping(
            sd_ptr.0 as *const u8,
            &mut *security_descriptor as *mut _ as *mut u8,
            sd_size as usize,
        );

        // Free the allocated descriptor
        let heap = GetProcessHeap().map_err(|_| anyhow::anyhow!("Failed to get process heap"))?;
        let _ = HeapFree(
            heap,
            HEAP_FLAGS(0),
            Some(sd_ptr.0 as *const std::ffi::c_void),
        );

        // Create security attributes pointing to our boxed descriptor
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: &*security_descriptor as *const _ as *mut _,
            bInheritHandle: false.into(),
        };

        Ok(PipeSecurityAttributes {
            _security_descriptor: security_descriptor,
            attributes,
        })
    }
}

/// Send a message over the named pipe
async fn send_message(pipe: &mut (impl AsyncWriteExt + Unpin), message: &IpcMessage) -> Result<()> {
    // Serialize the message
    let serialized = serde_json::to_vec(message).context("Failed to serialize IPC message")?;

    // Send length prefix (4 bytes)
    let length = serialized.len() as u32;
    pipe.write_all(&length.to_le_bytes())
        .await
        .context("Failed to write message length")?;

    // Send the message
    pipe.write_all(&serialized)
        .await
        .context("Failed to write message data")?;

    pipe.flush().await.context("Failed to flush message")?;

    Ok(())
}

/// Receive a message from the named pipe
async fn receive_message(pipe: &mut (impl AsyncReadExt + Unpin)) -> Result<Option<IpcMessage>> {
    // Read length prefix
    let mut length_bytes = [0u8; 4];
    match pipe.read_exact(&mut length_bytes).await {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            return Ok(None); // Client disconnected
        }
        Err(e) => return Err(e.into()),
    }

    let length = u32::from_le_bytes(length_bytes) as usize;

    // Validate length
    if length > 10 * 1024 * 1024 {
        return Err(anyhow::anyhow!("Message too large: {} bytes", length));
    }

    // Read the message
    let mut buffer = vec![0u8; length];
    pipe.read_exact(&mut buffer)
        .await
        .context("Failed to read message data")?;

    // Deserialize
    let message: IpcMessage =
        serde_json::from_slice(&buffer).context("Failed to deserialize IPC message")?;

    Ok(Some(message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_ipc_message_serialization() {
        let message = IpcMessage::Ping;
        let serialized = serde_json::to_vec(&message).unwrap();
        let deserialized: IpcMessage = serde_json::from_slice(&serialized).unwrap();

        match deserialized {
            IpcMessage::Ping => {}
            _ => panic!("Wrong message type"),
        }
    }

    #[tokio::test]
    #[ignore = "requires admin and stable named pipe handle setup"]
    async fn test_ipc_server_client() -> Result<()> {
        let server = IpcServer::new().await?;

        // Start server in background
        let server_handle = tokio::spawn(async move { server.run().await });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Connect client
        let client = IpcClient::new().await?;
        client.connect().await?;

        // Send ping
        client.send(IpcMessage::Ping).await?;

        // Receive pong (with timeout)
        let pong = timeout(Duration::from_secs(1), client.receive()).await??;

        match pong {
            Some(IpcMessage::Pong) => {}
            _ => panic!("Expected Pong message"),
        }

        server_handle.abort();
        Ok(())
    }
}
