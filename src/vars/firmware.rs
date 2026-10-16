//! Firmware identity exported through the Boot Loader Interface.

use uefi::CStr16;
use uefi::cstr16;
use uefi::system::{firmware_revision, firmware_vendor, uefi_revision};

/// Exports `LoaderFirmwareInfo` and `LoaderFirmwareType`.
pub(crate) fn export() {
    super::export_variable(
        cstr16!("LoaderFirmwareInfo"),
        &format_info(firmware_vendor(), firmware_revision()),
    );
    super::export_variable(
        cstr16!("LoaderFirmwareType"),
        &format!("UEFI {}", uefi_revision()),
    );
}

fn format_info(vendor: &CStr16, revision: u32) -> String {
    format!("{} {}.{:02}", vendor, revision >> 16, revision & 0xFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_info_zero_pads_minor_revision() {
        // ARRANGE
        let vendor = cstr16!("ACME");
        let revision = 0x0005_0005;

        // ACT
        let info = format_info(vendor, revision);

        // ASSERT
        assert_eq!(
            info, "ACME 5.05",
            "minor revisions must be zero padded to two digits"
        );
    }
}
