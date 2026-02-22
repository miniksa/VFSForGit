//! ProjFS (Windows Projected File System) FFI bindings and safe wrappers.
//!
//! This crate provides raw FFI bindings to `ProjectedFSLib.dll` with the exact
//! struct layouts verified against Windows SDK 10.0.26100.0, plus safe Rust
//! wrappers for the most common operations.

#![allow(non_snake_case, non_camel_case_types, dead_code)]

pub mod ffi;
pub mod provider;

pub use ffi::*;
pub use provider::*;
