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
#[derive(Debug, Clone)]
pub struct GitAuth {
    pub token: String,
}

impl GitAuth {
    /// Get credentials from git credential manager.
    pub fn from_credential_manager(url: &str) -> anyhow::Result<Self> {
        let output = std::process::Command::new("git")
            .args(["credential", "fill"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| {
                if let Some(stdin) = child.stdin.as_mut() {
                    let url_parsed = reqwest::Url::parse(url).unwrap();
                    let host = url_parsed.host_str().unwrap_or("");
                    let protocol = url_parsed.scheme();
                    write!(stdin, "protocol={}\nhost={}\n\n", protocol, host)?;
                }
                child.wait_with_output()
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut password = String::new();
        for line in stdout.lines() {
            if let Some(p) = line.strip_prefix("password=") {
                password = p.to_string();
                break;
            }
        }

        if password.is_empty() {
            anyhow::bail!("No credentials returned from git credential manager");
        }

        Ok(Self { token: password })
    }

    /// Create auth from a PAT directly.
    pub fn from_pat(pat: &str) -> Self {
        Self {
            token: pat.to_string(),
        }
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
            let encoded = base64_encode(&format!(":{}", auth.token));
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Basic {}", encoded)).unwrap(),
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

fn base64_encode(input: &str) -> String {
    
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
