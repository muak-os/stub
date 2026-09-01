//! Boot policy seam.

use anyhow::Result;

#[cfg(feature = "muak")]
use crate::info;

/// Returns the final kernel command line to boot with, given the UKI's embedded one.
///
/// The command line is copied into policy-owned memory.
///
/// # Errors
///
/// Returns an error if the command line copy cannot be allocated.
#[cfg(not(feature = "muak"))]
pub fn cmdline(base: Option<&[u8]>) -> Result<Option<Vec<u8>>> {
    let Some(base) = base else {
        return Ok(None);
    };

    let mut copy = Vec::new();
    copy.try_reserve_exact(base.len())
        .map_err(|e| anyhow::anyhow!("kernel command line allocation failed: {e}"))?;
    copy.extend_from_slice(base);

    Ok(Some(copy))
}

/// Prefer TPM-sealed keys; when no TPM2 is available, read the LUKS key from the
/// ESP `luks` file and append it to the kernel command line.
///
/// # Errors
///
/// Returns an error if the LUKS key file exists but cannot be read.
#[cfg(feature = "muak")]
pub fn cmdline(base: Option<&[u8]>) -> Result<Option<Vec<u8>>> {
    if !crate::tpm2::is_available()
        && let Some(combined) = luks::try_inject(base)?
    {
        info!("LUKS key read from ESP file");
        return Ok(Some(combined));
    }

    Ok(base.map(<[u8]>::to_vec))
}

#[cfg(feature = "muak")]
mod luks;
