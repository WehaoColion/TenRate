//! Bounded image blocks for Android AI conversations.
//! Only explicitly selected image bytes enter this module. No URLs, local
//! workspace media, telemetry or persistent caches are consulted.
use base64::Engine;
use serde_json::{json, Value};

pub const MAX_IMAGES: usize = 5;
pub const MAX_IMAGE_BYTES: usize = 6 * 1024 * 1024;
pub const MAX_TOTAL_IMAGE_BYTES: usize = 20 * 1024 * 1024;

fn mime_from_signature(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 13, 10, 26, 10]) {
        Some("image/png")
    } else if bytes.len() >= 3 && bytes[..3] == [0xff, 0xd8, 0xff] {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Shared validation for Android and Windows. Calling this never encodes
/// images, allocates a request body, or transmits private local bytes.
pub fn validate_image_inputs(images: &[Vec<u8>]) -> Result<(), String> {
    if images.len() > MAX_IMAGES {
        return Err(format!("每次最多发送 {MAX_IMAGES} 张图片，请减少选择。"));
    }
    let mut total = 0usize;
    for (i, bytes) in images.iter().enumerate() {
        let index = i + 1;
        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
            return Err(format!("第 {index} 张图片为空或超过 6 MiB，请选择较小的图片。"));
        }
        total = total.saturating_add(bytes.len());
        if total > MAX_TOTAL_IMAGE_BYTES {
            return Err("所选图片总大小超过 20 MiB，请减少图片数量。".into());
        }
        if mime_from_signature(bytes).is_none() {
            return Err(format!("第 {index} 张不是受支持的 JPEG、PNG、WebP 或 GIF 图片。"));
        }
    }
    Ok(())
}

/// Construct image parts only after validating the actual file signatures and
/// overall request budget. No unsupported input is silently dropped.
pub fn content_with_images(text: &str, images: &[Vec<u8>]) -> Result<Value, String> {
    validate_image_inputs(images)?;
    if images.is_empty() {
        return Ok(json!(text));
    }
    let mut blocks = vec![json!({"type":"input_text","text":text})];
    for bytes in images {
        let mime = mime_from_signature(bytes).expect("validated image signature");
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        blocks.push(json!({
            "type":"input_image",
            "image_url":format!("data:{mime};base64,{encoded}"),
            "detail":"original"
        }));
    }
    Ok(Value::Array(blocks))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_images_preserve_the_original_string_message() {
        assert_eq!(content_with_images("普通问答", &[]).unwrap(), json!("普通问答"));
    }

    #[test]
    fn image_blocks_use_real_mime_types_and_original_detail() {
        let png = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0];
        let jpeg = vec![0xff, 0xd8, 0xff, 0xe0, 0, 0];
        let webp = b"RIFF1234WEBPVP8 ".to_vec();
        let gif = b"GIF89a123".to_vec();
        let value = content_with_images("识别图片中的文字", &[png, jpeg, webp, gif]).unwrap();
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 5);
        assert_eq!(arr[0]["type"], "input_text");
        assert_eq!(arr[0]["text"], "识别图片中的文字");
        for (i, mime) in ["png", "jpeg", "webp", "gif"].iter().enumerate() {
            assert!(arr[i + 1]["image_url"].as_str().unwrap().starts_with(&format!("data:image/{mime};base64,")));
            assert_eq!(arr[i + 1]["detail"], "original");
        }
    }

    #[test]
    fn rejects_unsupported_and_oversized_inputs_without_partial_payload() {
        assert!(content_with_images("x", &[b"not an image".to_vec()]).is_err());
        assert!(content_with_images("x", &[vec![]]).is_err());
        assert!(content_with_images("x", &vec![vec![0xff, 0xd8, 0xff]; 6]).is_err());
        assert!(content_with_images("x", &[vec![0u8; MAX_IMAGE_BYTES + 1]]).is_err());
        let mut large = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10];
        large.resize(5 * 1024 * 1024, 0);
        let many_large = (0..5).map(|_| large.clone()).collect::<Vec<_>>();
        assert!(content_with_images("x", &many_large).is_err());
    }
}
