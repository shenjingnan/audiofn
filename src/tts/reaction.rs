/// 文本转语音的进度载荷。
///
/// TTS 是批量一次性合成，进度来自 sherpa-onnx `generate_with_config` 的进度回调
/// （0..1）。此处只定义推给前端的可序列化载荷；进度回调本身用 `FnMut(f32) -> bool`
/// （返回 `false` 提前终止），避免跨 FFI 的 `'static` 约束下再包一层 trait。
use serde::Serialize;

/// 合成进度（0..1，0 表示刚开始，1 表示完成）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TtsProgress {
    pub percent: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tts_progress_serializes() {
        let p = TtsProgress { percent: 0.5 };
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["percent"], 0.5);
    }
}
