//! GVFS Virtualization — ProjFS provider that projects git index onto the filesystem.
//!
//! This crate implements the ProjFS callbacks that serve directory enumerations
//! and file content from the git object store, downloading objects on demand
//! from the GVFS remote when they're not available locally.

#![allow(non_snake_case)]

pub mod callbacks;
pub mod projection;
pub mod virtualizer;
