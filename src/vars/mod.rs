//! Boot variables exported to the OS before the kernel is started.

mod disk;
mod firmware;

use uefi::CStr16;
use uefi::Guid;
use uefi::Handle;
use uefi::runtime::{VariableAttributes, VariableVendor, set_variable, variable_exists};

use crate::{info, warn};

/// Vendor GUID of Systemd Boot Loader Interface.
pub const LOADER_VENDOR: VariableVendor =
    VariableVendor(Guid::parse_or_panic("4a67b082-0a4c-41cf-b6c7-440b29bb8c4f"));

/// Exports all Boot Loader Interface variables for the current boot.
pub fn export(device_handle: Option<Handle>) {
    firmware::export();
    disk::export(device_handle);
}

fn export_variable(name: &CStr16, value: &str) {
    if matches!(variable_exists(name, &LOADER_VENDOR), Ok(true)) {
        info!("{name} already set, keeping existing value");
        return;
    }

    let data = utf16_bytes(value);

    match set_variable(name, &LOADER_VENDOR, VariableAttributes::empty(), &data) {
        Ok(()) => info!("{name} exported: {value}"),
        Err(e) => warn!("{name}: export failed: {e}"),
    }
}

fn utf16_bytes(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "6b1f2d44";

    #[test]
    fn utf16_bytes_encode_little_endian_without_terminator() {
        // ARRANGE
        let mut expected = Vec::new();
        for unit in SAMPLE.encode_utf16() {
            expected.extend_from_slice(&unit.to_le_bytes());
        }

        // ACT
        let data = utf16_bytes(SAMPLE);

        // ASSERT
        assert_eq!(data.len(), 16, "8 characters must encode to 16 bytes");
        assert_eq!(data.first(), Some(&0x36), "'6' encodes as 0x0036 low byte");
        assert_eq!(data.get(1), Some(&0x00), "'6' encodes as 0x0036 high byte");
        assert_eq!(
            data.last(),
            Some(&0x00),
            "final UTF-16 unit encodes its high byte last"
        );
        assert_eq!(data, expected, "encoding must be little-endian UTF-16");
    }
}
