//! Module for extracting and consolidating time information from media metadata.
pub mod error;
mod extraction;
mod logic;
mod parsing;
pub mod structs;
mod filename_parsing;

pub use logic::get_time_info;
