#![allow(clippy::cast_sign_loss)]
use crate::features::error::MetadataError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::ops::Deref;

/// A newtype wrapper around the raw exiftool JSON output.
///
/// Provides ergonomic, typed accessors for common EXIF field patterns.
/// Supports both flat (`-n`) and grouped (`-g2 -n`) JSON structures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExifData(Value);

impl ExifData {
    #[must_use]
    pub const fn new(value: Value) -> Self {
        Self(value)
    }

    /// Access the underlying JSON value.
    #[must_use]
    pub const fn inner(&self) -> &Value {
        &self.0
    }

    fn find_value(&self, key: &str) -> Option<&Value> {
        if let Some(val) = self.0.get(key) {
            return Some(val);
        }
        let obj = self.0.as_object()?;
        for group_val in obj.values() {
            if let Some(val) = group_val.as_object().and_then(|g| g.get(key)) {
                return Some(val);
            }
        }
        None
    }

    fn key_matches(key: &str, target: &str) -> bool {
        let key_lower = key.to_lowercase();
        key_lower == target || key_lower.ends_with(&format!(":{target}"))
    }

    fn search_object<'a>(obj: &'a Map<String, Value>, target: &str) -> Option<&'a Value> {
        for (key, val) in obj {
            if Self::key_matches(key, target) {
                return Some(val);
            }
        }
        None
    }

    // --- Flat accessors (for -n output) ---

    #[must_use]
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        let val = self.find_value(key)?;
        if let Some(n) = val.as_f64() {
            return Some(n);
        }
        val.as_str().and_then(|s| s.parse().ok())
    }

    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.find_value(key).and_then(Value::as_u64)
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.find_value(key).and_then(Value::as_i64)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.find_value(key).and_then(Value::as_str)
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        self.get_str(key).map(str::to_owned)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.find_value(key).and_then(parse_bool)
    }

    #[must_use]
    pub fn get_value(&self, key: &str) -> Option<&Value> {
        self.find_value(key)
    }

    /// # Errors
    /// * If field is missing or not u64
    pub fn require_u64(&self, key: &str) -> Result<u64, MetadataError> {
        self.get_u64(key)
            .ok_or_else(|| MetadataError::MissingRequiredField(key.to_string()))
    }

    /// # Errors
    /// * If field is or not a string
    pub fn require_string(&self, key: &str) -> Result<String, MetadataError> {
        self.get_string(key)
            .ok_or_else(|| MetadataError::MissingRequiredField(key.to_string()))
    }

    // --- Grouped accessors (for -g2 output) ---

    #[must_use]
    pub fn group_str(&self, group: &str, key: &str) -> Option<&str> {
        self.0.get(group)?.get(key)?.as_str()
    }

    #[must_use]
    pub fn group_u32(&self, group: &str, key: &str) -> Option<u32> {
        self.0
            .get(group)?
            .get(key)?
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
    }

    #[must_use]
    pub fn group_f64(&self, group: &str, key: &str) -> Option<f64> {
        let val = self.0.get(group)?.get(key)?;
        if let Some(n) = val.as_f64() {
            return Some(n);
        }
        val.as_str().and_then(|s| s.parse().ok())
    }

    // --- Convenience ---

    /// Case-insensitive tag lookup with optional namespace prefix matching (e.g. `GPano:UsePanoramaViewer`).
    #[must_use]
    pub fn get_ignoring_case(&self, tag_name: &str) -> Option<&Value> {
        let target = tag_name.to_lowercase();
        let root = self.0.as_object()?;

        if let Some(val) = Self::search_object(root, &target) {
            return Some(val);
        }

        for group_val in root.values() {
            if let Some(group_obj) = group_val.as_object()
                && let Some(val) = Self::search_object(group_obj, &target)
            {
                return Some(val);
            }
        }
        None
    }

    #[must_use]
    pub fn get_f64_ignoring_case(&self, tag_name: &str) -> Option<f64> {
        let val = self.get_ignoring_case(tag_name)?;
        if let Some(n) = val.as_f64() {
            return Some(n);
        }
        val.as_str().and_then(|s| s.parse().ok())
    }

    #[must_use]
    pub fn get_u64_ignoring_case(&self, tag_name: &str) -> Option<u64> {
        let val = self.get_ignoring_case(tag_name)?;
        if let Some(n) = val.as_u64() {
            return Some(n);
        }
        if let Some(n) = val.as_f64() {
            return Some(n as u64);
        }
        val.as_str().and_then(|s| {
            s.parse::<u64>()
                .ok()
                .or_else(|| s.parse::<f64>().ok().map(|f| f as u64))
        })
    }

    pub fn get_bool_ignoring_case(&self, tag_name: &str) -> Option<bool> {
        self.get_ignoring_case(tag_name).and_then(parse_bool)
    }

    #[must_use]
    pub fn is_video(&self) -> bool {
        self.group_str("Other", "MIMEType")
            .or_else(|| self.get_str("MIMEType"))
            .is_some_and(|s| s.starts_with("video/"))
    }
}

impl Deref for ExifData {
    type Target = Value;

    fn deref(&self) -> &Value {
        &self.0
    }
}

fn parse_bool(val: &Value) -> Option<bool> {
    if let Some(b) = val.as_bool() {
        return Some(b);
    }
    if let Some(s) = val.as_str() {
        let s_lower = s.to_lowercase();
        if s_lower == "true" || s_lower == "1" || s_lower == "yes" {
            return Some(true);
        }
        if s_lower == "false" || s_lower == "0" || s_lower == "no" {
            return Some(false);
        }
    }
    if let Some(n) = val.as_f64() {
        return Some(n == 1.0);
    }
    if let Some(n) = val.as_i64() {
        return Some(n == 1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn flat_access_at_root() {
        let exif = ExifData::new(json!({
            "ImageWidth": 4000,
            "FNumber": 2.8
        }));
        assert_eq!(exif.get_u64("ImageWidth"), Some(4000));
        assert_eq!(exif.get_f64("FNumber"), Some(2.8));
    }

    #[test]
    fn flat_access_searches_groups() {
        let exif = ExifData::new(json!({
            "File": { "ImageWidth": 1920, "ImageHeight": 1080 },
            "Composite": { "GPSLatitude": 52.5 }
        }));
        assert_eq!(exif.get_u64("ImageWidth"), Some(1920));
        assert_eq!(exif.get_f64("GPSLatitude"), Some(52.5));
    }

    #[test]
    fn grouped_access() {
        let exif = ExifData::new(json!({
            "Time": { "DateTimeOriginal": "2024:01:01 10:00:00" },
            "Other": { "FileName": "photo.jpg", "MIMEType": "video/mp4" },
            "GPS": { "GPSAltitude": 2401.5 }
        }));
        assert_eq!(
            exif.group_str("Time", "DateTimeOriginal"),
            Some("2024:01:01 10:00:00")
        );
        assert_eq!(exif.group_str("Other", "FileName"), Some("photo.jpg"));
        assert_eq!(exif.group_f64("GPS", "GPSAltitude"), Some(2401.5));
        assert!(exif.is_video());
    }

    #[test]
    fn get_ignoring_case_matches_namespace_prefix() {
        let exif = ExifData::new(json!({
            "XMP": { "GPano:UsePanoramaViewer": true }
        }));
        assert_eq!(exif.get_bool_ignoring_case("UsePanoramaViewer"), Some(true));
    }
}
