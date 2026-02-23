//! HTTP client for the GVFS protocol.
//!
//! Implements the Azure DevOps GVFS protocol endpoints:
//! - GET  /gvfs/config
//! - POST /gvfs/objects (batch download)
//! - GET  /gvfs/objects/<sha> (single object)
//! - GET  /gvfs/prefetch?lastPackTimestamp=<ts>
//! - POST /gvfs/sizes

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::config::{CacheServerInfo, ServerConfig};
use crate::constants;

/// GVFS protocol error types.
#[derive(Debug, thiserror::Error)]
pub enum GvfsHttpError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Authentication required")]
    AuthRequired,
    #[error("Server returned {status}: {body}")]
    ServerError { status: u16, body: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

/// Git credential manager authentication.
/// Mirrors the C# GitAuthentication class: calls `git credential fill` with
/// `GIT_TERMINAL_PROMPT=0` and `GCM_VALIDATE=0`, caches the credential in
/// memory, and calls `git credential approve` after first successful use.
#[derive(Debug, Clone)]
pub struct GitAuth {
    pub username: String,
    pub token: String,
}

impl GitAuth {
    /// Acquire credentials interactively during clone.
    /// Does NOT set GIT_TERMINAL_PROMPT=0 — allows GCM to open browser/device code.
    /// The C# CloneVerb calls this, then uses the token for HTTP, then calls approve().
    pub fn acquire_interactive(working_dir: &std::path::Path, repo_url: &str) -> anyhow::Result<Self> {
        let mut child = std::process::Command::new("git")
            .args([
                "-c", "credential.\"https://dev.azure.com\".useHttpPath=true",
                "credential", "fill",
            ])
            .current_dir(working_dir)
            // GCM_VALIDATE=0 skips the extra HTTP roundtrip but still allows interactive UI
            .env("GCM_VALIDATE", "0")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit()) // let GCM print to stderr if needed
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            write!(stdin, "url={}\n\n", repo_url)?;
        }

        // No timeout — user may need to authenticate in browser
        let output = child.wait_with_output()?;
        Self::parse_credential_output(&output.stdout)
    }

    /// Acquire cached credentials silently during mount.
    /// Sets GIT_TERMINAL_PROMPT=0 to suppress git's stdin prompt, and
    /// GCM_INTERACTIVE=never to prevent GCM from opening any UI (browser,
    /// device code dialog, etc). GCM returns the token that git fetch
    /// persisted automatically during clone.
    pub fn acquire_cached(working_dir: &std::path::Path, repo_url: &str) -> anyhow::Result<Self> {
        let mut child = std::process::Command::new("git")
            .args([
                "-c", "credential.\"https://dev.azure.com\".useHttpPath=true",
                "credential", "fill",
            ])
            .current_dir(working_dir)
            .env("GIT_TERMINAL_PROMPT", "0")  // suppress git stdin prompt
            .env("GCM_VALIDATE", "0")         // skip validation roundtrip
            .env("GCM_INTERACTIVE", "never")  // block ALL GCM UI (browser, dialog)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            write!(stdin, "url={}\n\n", repo_url)?;
        }

        // Timeout: cached creds return in <1s. If it blocks, creds aren't stored.
        let timeout = std::time::Duration::from_secs(10);
        let start = std::time::Instant::now();
        loop {
            match child.try_wait()? {
                Some(_) => break,
                None => {
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        anyhow::bail!("git credential fill timed out — credentials not cached from clone");
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }

        let output = child.wait_with_output()?;
        Self::parse_credential_output(&output.stdout)
    }

    fn parse_credential_output(stdout: &[u8]) -> anyhow::Result<Self> {
        let stdout_str = String::from_utf8_lossy(stdout);
        let mut password = String::new();
        let mut username = String::new();
        for line in stdout_str.lines() {
            if let Some(p) = line.strip_prefix("password=") {
                password = p.to_string();
            }
            if let Some(u) = line.strip_prefix("username=") {
                username = u.to_string();
            }
        }
        if password.is_empty() {
            anyhow::bail!("No credentials returned from git credential fill");
        }
        debug!("Got credentials for {} (token length: {})", username, password.len());
        Ok(Self { username, token: password })
    }

    /// Persist the credential in GCM's cache by calling `git credential approve`.
    /// The C# version calls this after the first successful HTTP request.
    pub fn approve(&self, working_dir: &std::path::Path, repo_url: &str) {
        let _ = std::process::Command::new("git")
            .args([
                "-c", "credential.\"https://dev.azure.com\".useHttpPath=true",
                "credential", "approve",
            ])
            .current_dir(working_dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = write!(
                        stdin,
                        "url={}\nusername={}\npassword={}\n\n",
                        repo_url, self.username, self.token
                    );
                }
                child.wait()
            });
    }

    /// Reject a bad credential so GCM erases it.
    pub fn reject(&self, working_dir: &std::path::Path, repo_url: &str) {
        let _ = std::process::Command::new("git")
            .args([
                "-c", "credential.\"https://dev.azure.com\".useHttpPath=true",
                "credential", "reject",
            ])
            .current_dir(working_dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = write!(stdin, "url={}\n\n", repo_url);
                }
                child.wait()
            });
    }

    /// Create auth from a PAT directly.
    pub fn from_pat(pat: &str) -> Self {
        Self {
            username: String::new(),
            token: pat.to_string(),
        }
    }

    /// Format as a Basic auth header value (Base64 of "username:password").
    pub fn as_basic_auth(&self) -> String {
        base64_encode(&format!("{}:{}", self.username, self.token))
    }
}

/// The GVFS HTTP client.
pub struct GvfsClient {
    client: reqwest::Client,
    base_url: String,
    auth: Option<GitAuth>,
    cache_server: Option<CacheServerInfo>,
    max_retries: u32,
}

impl GvfsClient {
    pub fn new(base_url: &str, auth: Option<GitAuth>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(constants::DEFAULT_TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
            cache_server: None,
            max_retries: constants::DEFAULT_MAX_RETRIES,
        }
    }

    pub fn with_cache_server(mut self, cs: CacheServerInfo) -> Self {
        self.cache_server = Some(cs);
        self
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(auth) = &self.auth {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Basic {}", auth.as_basic_auth())).unwrap(),
            );
        }
        headers.insert(
            "X-TFS-FedAuthRedirect",
            HeaderValue::from_static("Suppress"),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&format!("GVFS-Rust/{}", constants::GVFS_VERSION)).unwrap(),
        );
        headers
    }

    fn objects_base_url(&self) -> String {
        if let Some(cs) = &self.cache_server {
            cs.objects_url()
        } else {
            format!("{}/gvfs/objects", self.base_url)
        }
    }

    fn prefetch_base_url(&self) -> String {
        if let Some(cs) = &self.cache_server {
            cs.prefetch_url()
        } else {
            format!("{}/gvfs/prefetch", self.base_url)
        }
    }

    /// Fetch server GVFS config.
    pub async fn get_config(&self) -> Result<ServerConfig, GvfsHttpError> {
        let url = format!("{}/gvfs/config", self.base_url);
        debug!("Fetching GVFS config from {}", url);

        let resp = self
            .client
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if resp.status() == 401 {
            return Err(GvfsHttpError::AuthRequired);
        }
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(GvfsHttpError::ServerError { status, body });
        }

        let config: ServerConfig = resp.json().await?;
        Ok(config)
    }

    /// Download a single loose object by SHA.
    pub async fn download_object(&self, sha: &str) -> Result<Vec<u8>, GvfsHttpError> {
        let url = format!("{}/{}", self.objects_base_url(), sha);
        debug!("Downloading object {} from {}", sha, url);

        let resp = self
            .client
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(GvfsHttpError::ServerError { status, body });
        }

        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }

    /// Batch download objects. Request body is JSON with objectIds.
    /// Response is a packfile.
    pub async fn download_objects_batch(
        &self,
        shas: &[String],
        output_dir: &Path,
    ) -> Result<PathBuf, GvfsHttpError> {
        let url = self.objects_base_url();
        debug!("Batch downloading {} objects from {}", shas.len(), url);

        let body = serde_json::json!({
            "objectIds": shas,
            "commitDepth": 1
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.auth_headers())
            .header(ACCEPT, "application/x-git-packfile")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(GvfsHttpError::ServerError { status, body });
        }

        // Write the packfile to disk.
        let pack_name = format!("gvfs-{}.pack", &shas[0][..8]);
        let pack_path = output_dir.join(&pack_name);
        std::fs::create_dir_all(output_dir)?;

        let bytes = resp.bytes().await?;
        std::fs::write(&pack_path, &bytes)?;

        debug!("Wrote {} bytes to {:?}", bytes.len(), pack_path);
        Ok(pack_path)
    }

    /// Prefetch packs since a given timestamp.
    pub async fn prefetch(
        &self,
        last_pack_timestamp: i64,
        output_dir: &Path,
    ) -> Result<Vec<PathBuf>, GvfsHttpError> {
        let url = format!(
            "{}?lastPackTimestamp={}",
            self.prefetch_base_url(),
            last_pack_timestamp
        );
        debug!("Prefetching from {}", url);

        let resp = self
            .client
            .get(&url)
            .headers(self.auth_headers())
            .header(ACCEPT, "application/x-gvfs-timestamped-packfiles-indexes")
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(GvfsHttpError::ServerError { status, body });
        }

        let bytes = resp.bytes().await?;

        // The response is a concatenation of timestamped pack+idx files.
        // Parse the header format: each entry prefixed with 8-byte timestamp + 4-byte type + 8-byte length.
        let packs = parse_prefetch_response(&bytes, output_dir)?;
        info!("Prefetched {} pack files", packs.len());
        Ok(packs)
    }

    /// Query file sizes for a list of object SHAs.
    pub async fn query_sizes(&self, shas: &[String]) -> Result<Vec<ObjectSize>, GvfsHttpError> {
        let url = format!("{}/gvfs/sizes", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.auth_headers())
            .json(&shas)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(GvfsHttpError::ServerError { status, body });
        }

        let sizes: Vec<ObjectSize> = resp.json().await?;
        Ok(sizes)
    }

    /// Git info/refs request.
    pub async fn info_refs(&self) -> Result<String, GvfsHttpError> {
        let url = format!("{}/info/refs?service=git-upload-pack", self.base_url);
        let resp = self
            .client
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(GvfsHttpError::ServerError { status, body });
        }

        Ok(resp.text().await?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSize {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Size")]
    pub size: i64,
}

/// Parse the prefetch response binary format.
/// Format: repeated entries of [timestamp: i64][type: "P" or "I"][length: i64][data: bytes]
fn parse_prefetch_response(data: &[u8], output_dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::io::Cursor;

    let mut paths = Vec::new();
    let mut cursor = Cursor::new(data);

    std::fs::create_dir_all(output_dir)?;

    while (cursor.position() as usize) < data.len() {
        // Read timestamp
        let timestamp = match cursor.read_i64::<LittleEndian>() {
            Ok(t) => t,
            Err(_) => break,
        };

        // Read type byte (P = pack, I = idx)
        let mut type_byte = [0u8; 1];
        if std::io::Read::read_exact(&mut cursor, &mut type_byte).is_err() {
            break;
        }

        // Read length
        let length = match cursor.read_i64::<LittleEndian>() {
            Ok(l) => l,
            Err(_) => break,
        };

        // Read data
        let pos = cursor.position() as usize;
        let end = pos + length as usize;
        if end > data.len() {
            break;
        }

        let ext = if type_byte[0] == b'P' { "pack" } else { "idx" };
        let filename = format!("prefetch-{}.{}", timestamp, ext);
        let file_path = output_dir.join(&filename);
        std::fs::write(&file_path, &data[pos..end])?;

        if type_byte[0] == b'P' {
            paths.push(file_path);
        }

        cursor.set_position(end as u64);
    }

    Ok(paths)
}

/// Simple base64 encoding (public for use by GitAuth).
pub fn base64_encode(input: &str) -> String {
    
    // Simple base64 encoding
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}
