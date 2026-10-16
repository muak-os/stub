//! Disk-related UEFI variable exports.

use uefi::CStr16;
use uefi::Guid;
use uefi::Handle;
use uefi::boot::open_protocol_exclusive;
use uefi::cstr16;
use uefi::proto::device_path::DevicePath;
use uefi::proto::device_path::media::{HardDrive, PartitionSignature};

use crate::info;

// Name of the exported boot partition PARTUUID variable.
const LOADER_DEVICE_PART_UUID: &CStr16 = cstr16!("LoaderDevicePartUUID");

/// Exports the GPT PARTUUID of the partition the stub was loaded from.
pub(crate) fn export(device_handle: Option<Handle>) {
    let Some(handle) = device_handle else {
        info!("LoaderDevicePartUUID: loaded image has no device handle, skipping");
        return;
    };

    let device_path = match open_protocol_exclusive::<DevicePath>(handle) {
        Ok(path) => path,
        Err(e) => {
            info!("LoaderDevicePartUUID: cannot open device path: {e}");
            return;
        }
    };

    let Some(guid) = partition_guid(&device_path) else {
        info!("LoaderDevicePartUUID: device path has no GPT partition node, skipping");
        return;
    };

    super::export_variable(LOADER_DEVICE_PART_UUID, &guid.to_string());
}

#[must_use]
fn partition_guid(device_path: &DevicePath) -> Option<Guid> {
    device_path.node_iter().find_map(|node| {
        let hard_drive = <&HardDrive>::try_from(node).ok()?;
        match hard_drive.partition_signature() {
            PartitionSignature::Guid(guid) => Some(guid),
            PartitionSignature::None
            | PartitionSignature::Mbr(_)
            | PartitionSignature::Unknown { .. } => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // PARTUUID text expected for `SIGNATURE_BYTES`.
    const PARTUUID_TEXT: &str = "6b1f2d44-9c85-4c8a-9d1e-0e2f31a4b5c6";

    // GUID signature bytes in UEFI memory layout (little-endian fields).
    const SIGNATURE_BYTES: [u8; 16] = [
        0x44, 0x2d, 0x1f, 0x6b, 0x85, 0x9c, 0x8a, 0x4c, 0x9d, 0x1e, 0x0e, 0x2f, 0x31, 0xa4, 0xb5,
        0xc6,
    ];

    fn hard_drive_node(signature_type: u8, signature: [u8; 16]) -> Vec<u8> {
        let mut node = Vec::new();
        node.extend_from_slice(&[0x04, 0x01]); // MEDIA_DEVICE_PATH / MEDIA_HARDDRIVE_DP
        node.extend_from_slice(&42_u16.to_le_bytes()); // node length
        node.extend_from_slice(&1_u32.to_le_bytes()); // partition number
        node.extend_from_slice(&2048_u64.to_le_bytes()); // partition start LBA
        node.extend_from_slice(&2048_u64.to_le_bytes()); // partition size in blocks
        node.extend_from_slice(&signature);
        node.push(0x02); // PartitionFormat::GPT
        node.push(signature_type);

        node
    }

    fn pci_node() -> Vec<u8> {
        let mut node = vec![0x01, 0x01]; // HARDWARE_DEVICE_PATH / HARDWARE_PCI
        node.extend_from_slice(&6_u16.to_le_bytes()); // node length
        node.extend_from_slice(&[0x05, 0x18]); // function, device

        node
    }

    fn file_path_node() -> Vec<u8> {
        let mut node = vec![0x04, 0x04]; // MEDIA_DEVICE_PATH / MEDIA_FILE_PATH_DP
        node.extend_from_slice(&6_u16.to_le_bytes()); // node length
        node.extend_from_slice(&[0x00, 0x00]); // empty UCS-2 path name

        node
    }

    fn end_node() -> Vec<u8> {
        let mut node = vec![0x7F, 0xFF]; // END_DEVICE_PATH / END_ENTIRE
        node.extend_from_slice(&4_u16.to_le_bytes()); // node length

        node
    }

    fn device_path_bytes(nodes: &[&[u8]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for node in nodes {
            bytes.extend_from_slice(node);
        }
        bytes.extend_from_slice(&end_node());

        bytes
    }

    fn parse(bytes: &[u8]) -> &DevicePath {
        // SAFETY: `bytes` holds a well-formed device path: node lengths are
        // consistent and the path ends with an end-entire node.
        unsafe { DevicePath::from_ffi_ptr(bytes.as_ptr().cast()) }
    }

    #[test]
    fn variable_name_matches_systemd_stub() {
        // ARRANGE
        let name = LOADER_DEVICE_PART_UUID;

        // ACT
        let rendered = name.to_string();

        // ASSERT
        assert_eq!(
            rendered, "LoaderDevicePartUUID",
            "variable name must match systemd-stub exactly"
        );
    }

    #[test]
    fn gpt_hard_drive_node_yields_guid() {
        // ARRANGE
        let bytes = device_path_bytes(&[
            &pci_node(),
            &hard_drive_node(2, SIGNATURE_BYTES),
            &file_path_node(),
        ]);

        // ACT
        let guid = partition_guid(parse(&bytes));

        // ASSERT
        assert_eq!(
            guid.map(|signed| signed.to_string()).as_deref(),
            Some(PARTUUID_TEXT),
            "GPT partition node GUID should be found and formatted canonically"
        );
    }

    #[test]
    fn mbr_signature_node_is_skipped() {
        // ARRANGE
        let mut signature = [0_u8; 16];
        signature[0] = 0xAA;
        signature[1] = 0xBB;
        signature[2] = 0xCC;
        signature[3] = 0xDD;
        let bytes = device_path_bytes(&[&hard_drive_node(1, signature)]);

        // ACT
        let guid = partition_guid(parse(&bytes));

        // ASSERT
        assert!(
            guid.is_none(),
            "MBR-signature partition nodes must not yield a GUID"
        );
    }

    #[test]
    fn path_without_partition_node_yields_none() {
        // ARRANGE
        let bytes = device_path_bytes(&[&pci_node(), &file_path_node()]);

        // ACT
        let guid = partition_guid(parse(&bytes));

        // ASSERT
        assert!(
            guid.is_none(),
            "device paths without a hard-drive node must yield no GUID"
        );
    }

    #[test]
    fn first_guid_signed_node_wins() {
        // ARRANGE.
        let mut mbr_signature = [0_u8; 16];
        mbr_signature[0] = 0x12;
        mbr_signature[1] = 0x34;
        mbr_signature[2] = 0x56;
        mbr_signature[3] = 0x78;
        let bytes = device_path_bytes(&[
            &hard_drive_node(1, mbr_signature),
            &hard_drive_node(2, SIGNATURE_BYTES),
        ]);

        // ACT
        let guid = partition_guid(parse(&bytes));

        // ASSERT
        assert_eq!(
            guid.map(|signed| signed.to_string()).as_deref(),
            Some(PARTUUID_TEXT),
            "the first GUID-signed hard-drive node must win, matching systemd-stub"
        );
    }
}
