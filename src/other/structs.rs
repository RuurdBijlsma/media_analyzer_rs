use chrono::{DateTime, Utc};
use meteostat::Hourly;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct GpsInfo {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub location: LocationName,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MediaMetadata {
    pub width: u64,
    pub height: u64,
    pub mime_type: String,
    pub duration: Option<f64>,
    pub size_bytes: Option<u64>,
    pub iso: Option<u64>,
    pub exposure_time: Option<f64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PanoInfo {
    pub use_panorama_viewer: bool,
    pub is_photosphere: bool,
    pub view_info: Option<PanoViewInfo>,
    pub projection_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PanoViewInfo {
    /// The calculated horizontal field of view in degrees.
    pub horizontal_fov_deg: f64,
    /// The calculated vertical field of view in degrees.
    pub vertical_fov_deg: f64,
    /// The horizontal center of the view in degrees (-180 to 180).
    pub center_yaw_deg: f64,
    /// The vertical center of the view in degrees (-90 to 90).
    pub center_pitch_deg: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct LocationName{
    pub latitude: f64,
    pub longitude: f64,
    pub name: String,
    pub admin1: String,
    pub admin2: String,
    pub country_code: String,
    pub country_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WeatherInfo {
    pub hourly: Option<Hourly>,
    pub sun_info: SunInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SunInfo {
    pub sunrise: DateTime<Utc>,
    pub sunset: DateTime<Utc>,
    pub dawn: DateTime<Utc>,
    pub dusk: DateTime<Utc>,
    pub is_daytime: bool,
}
