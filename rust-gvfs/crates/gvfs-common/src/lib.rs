//! Shared types and utilities for all GVFS binaries.
//!
//! This crate contains the named pipe protocol, config management, HTTP client
//! for the GVFS protocol, repository metadata, modified paths database,
//! enlistment abstraction, git utilities, and the GVFS lock.

pub mod config;
pub mod constants;
pub mod enlistment;
pub mod git;
pub mod http;
pub mod lock;
pub mod modified_paths;
pub mod pipe;
pub mod repo_metadata;
