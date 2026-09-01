//! Testable components of the UEFI stub.

#![cfg_attr(feature = "uefi", feature(uefi_std))]

pub mod loadfile2;
pub mod log;
pub mod pe;
pub mod policy;
pub mod security;
#[cfg(feature = "tpm")]
pub mod tpm2;
