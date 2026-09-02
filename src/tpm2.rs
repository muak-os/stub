//! EFI TCG2 protocol interface for TPM2 PCR measurements.

use anyhow::{Context as _, Result};
use uefi::Identify as _;
use uefi::boot::{self, ScopedProtocol, SearchType};
use uefi::proto::tcg::v2::{HashLogExtendEventFlags, PcrEventInputs, Tcg};
use uefi::proto::tcg::{EventType, PcrIndex};

const PCR_INDEX: u32 = 11;

/// Prefix of an `EFI_TCG2_EVENT`: `u32` size field + 14-byte `EFI_TCG2_EVENT_HEADER`.
const PCR_EVENT_PREFIX_LEN: usize = 18;

/// Returns whether the EFI TCG2 protocol is available (TPM2 present).
#[must_use]
pub fn is_available() -> bool {
    boot::locate_handle_buffer(SearchType::ByProtocol(&Tcg::GUID))
        .is_ok_and(|handles| !handles.is_empty())
}

/// Measures a UKI section into PCR#11 via the EFI TCG2 protocol.
///
/// # Errors
///
/// Returns an error if the TCG2 protocol is unavailable or measurement fails.
pub fn measure_section(name: &str, data: &[u8]) -> Result<()> {
    let mut tcg = open_tcg()?;

    let mut name_bytes = name.as_bytes().to_vec();
    name_bytes.push(0_u8);
    hash_log_extend(&mut tcg, name, &name_bytes)?;
    hash_log_extend(&mut tcg, name, data)?;

    Ok(())
}

fn open_tcg() -> Result<ScopedProtocol<Tcg>> {
    let handles = boot::locate_handle_buffer(SearchType::ByProtocol(&Tcg::GUID))
        .context("Failed to search for EFI_TCG2_PROTOCOL handles")?;
    let handle = handles.first().context("EFI_TCG2_PROTOCOL not available")?;

    boot::open_protocol_exclusive::<Tcg>(*handle).context("Failed to open EFI_TCG2_PROTOCOL")
}

fn hash_log_extend(tcg: &mut Tcg, event_name: &str, data: &[u8]) -> Result<()> {
    let event_desc = event_name.as_bytes();
    let event_size = PCR_EVENT_PREFIX_LEN
        .checked_add(event_desc.len())
        .context("TCG2 event size overflow")?;
    let mut event_buf = vec![0_u8; event_size];
    let event = PcrEventInputs::new_in_buffer(
        &mut event_buf,
        PcrIndex(PCR_INDEX),
        EventType::IPL,
        event_desc,
    )
    .context("Failed to build TCG2 event")?;

    tcg.hash_log_extend_event(HashLogExtendEventFlags::empty(), data, event)
        .with_context(|| format!("HashLogExtendEvent failed for section {event_name}"))
}
