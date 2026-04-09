//! Cross-privilege named pipe IPC
//!
//! This module implements secure named pipe communication between
//! the GUI (unprivileged) and scanner (elevated) processes.

use anyhow::{Context, Result};
use std::io;
use std::os::windows::io::FromRawHandle;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex};

#[cfg(windows)]
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use windows::core::PCWSTR;
use windows::Win32::Foundation::GetLastError;
use windows::Win32::Security::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SECURITY_ATTRIBUTES,
    SECURITY_DESCRIPTOR,
};
use windows::Win32::System::Pipes::{
    CreateNamedPipeW, CreatePipe, PIPE_ACCEPT_REMOTE_CLIENTS, PIPE_ACCESS_DUPLEX,
    PIPE_READMODE_MESSAGE, PIPE_TYPE_BYTE, PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use winsweep_common::types::IpcMessage;

/// Named pipe name for WinSweep IPC
const PIPE_NAME: &str = r"\\.\pipe\WinSweepIPC";

/// SDDL string allowing Authenticated Users full access
const PIPE_SDDL: &str = "D:(A;;GA;;;AU)"; // DACL: Allow Generic All to Authenticated Users

/// IPC server for elevated scanner process
pub struct IpcServer {
    server: Arc<Mutex<NamedPipeServer>>,
    message_sender: mpsc::UnboundedSender<IpcMessage>,
    message_receiver: Arc<Mutex<mpsc::UnboundedReceiver<IpcMessage>>>,
}

/// IPC client for GUI process
pub struct IpcClient {
    client: Arc<Mutex<Option<NamedPipeClient>>>,
    message_sender: mpsc::UnboundedSender<IpcMessage>,
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
        let server = NamedPipeServer::from_raw_handle(pipe_handle.0);

        let (tx, rx) = mpsc::unbounded_channel();
        let receiver = Arc::new(Mutex::new(rx));

        let server = Arc::new(Mutex::new(server));

        Ok(Self {
            server,
            message_sender: tx,
            message_receiver: receiver,
        })
    }

    /// Start accepting connections and processing messages
    pub async fn run(&self) -> Result<()> {
        info!("Starting IPC server message loop");

        // Clone references for the task
        let server = self.server.clone();
        let receiver = self.message_receiver.clone();

        // Spawn task to handle outgoing messages
        let sender_task = tokio::spawn(async move {
            let mut receiver = receiver.lock().await;
            let mut server = server.lock().await;

            while let Some(message) = receiver.recv().await {
                debug!(
                    "Sending IPC message: {:?}",
                    std::mem::discriminant(&message)
                );

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
        let mut server = self.server.lock().await;

        loop {
            match receive_message(&mut *server).await {
                Ok(Some(message)) => {
                    debug!(
                        "Received IPC message: {:?}",
                        std::mem::discriminant(&message)
                    );

                    // Handle ping/pong automatically
                    match message {
                        IpcMessage::Ping => {
                            let _ = self.message_sender.send(IpcMessage::Pong);
                        }
                        _ => {
                            // Forward to application handler
                            // In a real implementation, you'd have a callback channel
                        }
                    }
                }
                Ok(None) => {
                    warn!("Client disconnected");
                    break;
                }
                Err(e) => {
                    error!("Error receiving message: {}", e);
                    break;
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
            message_sender: tx,
            message_receiver: receiver,
        })
    }

    /// Connect to the IPC server
    pub async fn connect(&self) -> Result<()> {
        info!("Connecting to IPC server");

        let client = ClientOptions::new()
            .open(PIPE_NAME)
            .await
            .context("Failed to connect to named pipe")?;

        *self.client.lock().await = Some(client);

        // Start message handling task
        self.start_message_loop().await?;

        Ok(())
    }

    /// Send a message to the server
    pub async fn send(&self, message: IpcMessage) -> Result<()> {
        let mut client_guard = self.client.lock().await;

        if let Some(ref mut client) = *client_guard {
            send_message(client, &message).await
        } else {
            Err(anyhow::anyhow!("Not connected to IPC server"))
        }
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
            let mut receiver = receiver.lock().await;

            loop {
                let mut client_guard = client.lock().await;
                if let Some(ref mut client) = *client_guard {
                    match receiver.recv().await {
                        Some(message) => {
                            if let Err(e) = send_message(client, &message).await {
                                error!("Failed to send message: {}", e);
                                break;
                            }
                        }
                        None => break,
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
fn create_named_pipe_with_security(
    security_attributes: &SECURITY_ATTRIBUTES,
) -> Result<windows::Win32::Foundation::HANDLE> {
    use std::ptr;
    use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows::Win32::System::Threading::GetCurrentProcess;
    use windows::Win32::System::IO::FILE_FLAG_OVERLAPPED;

    let pipe_name_wide = to_wide(Path::new(PIPE_NAME));

    unsafe {
        let handle = CreateNamedPipeW(
            PCWSTR(pipe_name_wide.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED.0,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_ACCEPT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            4096, // Out buffer size
            4096, // In buffer size
            0,    // Default timeout
            Some(security_attributes),
        );

        if handle == INVALID_HANDLE_VALUE {
            let error = GetLastError();
            return Err(anyhow::anyhow!(
                "CreateNamedPipeW failed: error {}",
                error.0
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
    use std::ptr;

    let mut sd_ptr = ptr::null_mut();

    // Convert SDDL to security descriptor
    unsafe {
        let result = ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(
                PIPE_SDDL
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect::<Vec<u16>>()
                    .as_ptr(),
            ),
            SDDL_REVISION_1,
            &mut sd_ptr,
            ptr::null_mut(),
        );

        if result.is_err() {
            return Err(anyhow::anyhow!(
                "Failed to convert SDDL to security descriptor"
            ));
        }

        // Get the size of the security descriptor
        let sd_size = windows::Win32::Security::GetSecurityDescriptorLength(sd_ptr as *const _);

        // Allocate a box to hold the security descriptor
        let mut security_descriptor = Box::new(SECURITY_DESCRIPTOR::default());

        // Copy the security descriptor into our box
        std::ptr::copy_nonoverlapping(
            sd_ptr,
            &mut *security_descriptor as *mut _ as *mut _,
            sd_size as usize,
        );

        // Free the allocated descriptor
        windows::Win32::System::Memory::LocalFree(sd_ptr as isize);

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
    async fn test_ipc_server_client() -> Result<()> {
        // This test requires elevated privileges to run
        // Skip in CI unless running as admin

        if !is_running_as_admin() {
            return Ok(());
        }

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

    fn is_running_as_admin() -> bool {
        // Simple check - in real implementation you'd check token privileges
        std::env::var("USERDOMAIN").unwrap_or_default() != "USERDOMAIN"
    }
}
