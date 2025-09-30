use serde_json::Value;

pub fn detect_hdr(v: &Value) -> bool {
    // 1. Pixel: CompositeImage == 3
    if v.get("CompositeImage")
        .and_then(|x| x.as_i64())
        .map(|x| x == 3)
        .unwrap_or(false)
    {
        return true;
    }

    // 2. SceneCaptureType == 3 (some DSLRs / iPhones)
    if v.get("SceneCaptureType")
        .and_then(|x| x.as_i64())
        .map(|x| x == 3)
        .unwrap_or(false)
    {
        return true;
    }

    // 3. Explicit HDR tag
    if v.get("HDRImageType").is_some() {
        return true;
    }

    // 4. Software string contains "hdr"
    if v.get("Software")
        .and_then(|x| x.as_str())
        .map(|s| s.to_lowercase().contains("hdr"))
        .unwrap_or(false)
    {
        return true;
    }

    // 5. XMP / gain map detection
    if v.get("GainMapImage").is_some()
        || v.get("DirectoryItemSemantic")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter().any(|s| {
                    s.as_str()
                        .map(|s| s.eq_ignore_ascii_case("GainMap"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    {
        return true;
    }

    false
}
