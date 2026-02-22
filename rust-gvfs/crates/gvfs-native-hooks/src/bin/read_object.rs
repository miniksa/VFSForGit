//! read-object — Native git hook for downloading missing objects.
//!
//! This hook is called by git when it needs an object that isn't available
//! locally. It communicates with the GVFS mount daemon via named pipe using
//! the DLO (Download Object) command.
//!
//! Protocol: git pkt-line over stdin/stdout.
//! Handshake → capability negotiation → object download loop.

use std::io::{self, Read, Write};
use std::path::PathBuf;

use gvfs_native_hooks::{get_enlistment_root_from_cwd, get_enlistment_root_from_hook};
use gvfs_common::pipe::{self};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Determine enlistment root.
    let root = args
        .first()
        .and_then(|a| get_enlistment_root_from_hook(a))
        .or_else(get_enlistment_root_from_cwd)
        .unwrap_or_else(|| {
            eprintln!("read-object: cannot determine enlistment root");
            std::process::exit(1);
        });

    if let Err(e) = run_read_object_hook(&root) {
        eprintln!("read-object: error: {}", e);
        std::process::exit(1);
    }
}

fn run_read_object_hook(root: &PathBuf) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    // ─── Handshake ─────────────────────────────────────────────────

    // Read: "git-read-object-client\n"
    let line = read_pkt_line(&mut reader)?;
    if !line.starts_with("git-read-object-client") {
        anyhow::bail!("Expected git-read-object-client, got: {}", line);
    }

    // Read: "version=1\n"
    let _version = read_pkt_line(&mut reader)?;

    // Read flush
    read_pkt_flush(&mut reader)?;

    // Send: "git-read-object-server\n"
    write_pkt_line(&mut writer, "git-read-object-server\n")?;
    write_pkt_line(&mut writer, "version=1\n")?;
    write_pkt_flush(&mut writer)?;

    // Read capabilities
    let _cap = read_pkt_line(&mut reader)?;
    read_pkt_flush(&mut reader)?;

    // Send capabilities
    write_pkt_line(&mut writer, "capability=get\n")?;
    write_pkt_flush(&mut writer)?;
    writer.flush()?;

    // ─── Download loop ─────────────────────────────────────────────

    loop {
        // Read "command=get\n" or end of input
        let line = match read_pkt_line(&mut reader) {
            Ok(l) => l,
            Err(_) => break, // EOF — git closed stdin
        };

        if !line.starts_with("command=get") {
            continue;
        }

        // Read "sha1=<40hex>\n"
        let sha_line = read_pkt_line(&mut reader)?;
        let sha = sha_line
            .strip_prefix("sha1=")
            .unwrap_or(&sha_line)
            .trim()
            .to_string();

        // Read flush
        read_pkt_flush(&mut reader)?;

        // Download via named pipe.
        let success = pipe::download_object(root, &sha).unwrap_or(false);

        if success {
            write_pkt_line(&mut writer, "status=success\n")?;
        } else {
            write_pkt_line(&mut writer, "status=error\n")?;
        }
        write_pkt_flush(&mut writer)?;
        writer.flush()?;
    }

    Ok(())
}

// ─── pkt-line protocol helpers ─────────────────────────────────────

fn read_pkt_line(reader: &mut impl Read) -> anyhow::Result<String> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len_str = std::str::from_utf8(&len_buf)?;
    let len = u16::from_str_radix(len_str, 16)? as usize;

    if len == 0 {
        // Flush packet
        return Ok(String::new());
    }

    if len < 4 {
        anyhow::bail!("Invalid pkt-line length: {}", len);
    }

    let data_len = len - 4;
    let mut buf = vec![0u8; data_len];
    reader.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

fn read_pkt_flush(reader: &mut impl Read) -> anyhow::Result<()> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {
            // Should be "0000"
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn write_pkt_line(writer: &mut impl Write, data: &str) -> anyhow::Result<()> {
    let len = data.len() + 4;
    write!(writer, "{:04x}{}", len, data)?;
    Ok(())
}

fn write_pkt_flush(writer: &mut impl Write) -> anyhow::Result<()> {
    write!(writer, "0000")?;
    Ok(())
}
