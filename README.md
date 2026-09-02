# muak-os/stub

UEFI boot stub for [Muak](https://github.com/muak-os/muak). PE/COFF EFI binary that loads a
Unified Kernel Image, measures its sections into TPM2 PCR #11, installs the initrd via the EFI
LoadFile2 protocol, and starts a kernel.
