//! Module for extracting and consolidating time information from media metadata.
mod extraction;
mod logic;
mod parsing;
pub mod time_types;
pub use logic::get_time_info;
