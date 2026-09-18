use std::{
    fs,
    io::{Seek, Write},
};

use anyhow::{Context, ensure};
use ere_util_tokio::block_on;
use flate2::read::GzDecoder;
use md5::{Digest, Md5};
use parking_lot::Mutex;
use zisk_common::ZiskPaths;

/// URL of the proving key of v1.2.0-alpha.
const PROVING_KEY_URL: &str =
    "https://storage.googleapis.com/zisk-setup/zisk-provingkey-1.2.0-alpha.tar.gz";

/// MD5 of the proving key of v1.2.0-alpha.
const PROVING_KEY_MD5: [u8; 16] = hex_literal::hex!("51e142e7217b807bec25af465b7eeb71");

/// Marker file holding the hex MD5 of the unpacked archive, written last.
const PROVING_KEY_MD5_FILE: &str = ".md5";

/// Serializes callers in this process so at most one downloads.
static LOCK: Mutex<()> = Mutex::new(());

/// Downloads and unpacks the proving key into `ZiskPaths::proving_key` unless
/// it is already there.
///
/// The directory is filled in place so it can be a mount point. A key with a
/// different MD5 is left untouched and reported as an error.
pub(super) fn ensure_proving_key() -> anyhow::Result<()> {
    let _guard = LOCK.lock();

    let path = &ZiskPaths::global().proving_key;
    let md5_file = path.join(PROVING_KEY_MD5_FILE);
    if let Ok(md5) = fs::read(&md5_file) {
        ensure!(
            md5.trim_ascii() == hex::encode(PROVING_KEY_MD5).as_bytes(),
            "proving key at {} does not match the pinned archive, remove it or use another volume",
            path.display()
        );
        return Ok(());
    }

    fs::create_dir_all(path)?;
    let mut archive = tempfile::tempfile_in(path)?;
    block_on(async {
        let mut response = reqwest::get(PROVING_KEY_URL).await?.error_for_status()?;
        let mut digest = Md5::new();
        while let Some(chunk) = response.chunk().await? {
            archive.write_all(&chunk)?;
            digest.update(&chunk);
        }
        ensure!(
            digest.finalize()[..] == PROVING_KEY_MD5,
            "proving key checksum mismatch"
        );
        anyhow::Ok(())
    })?;

    // The archive unpacks to `<parent>/provingKey`, which is `path`.
    let parent = path.parent().context("proving key path has no parent")?;
    archive.rewind()?;
    tar::Archive::new(GzDecoder::new(archive)).unpack(parent)?;

    fs::write(md5_file, hex::encode(PROVING_KEY_MD5))?;

    Ok(())
}
