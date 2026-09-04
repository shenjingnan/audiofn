/// 离线（文件）语音识别：整段 wav 经 audiocpp sidecar 的 qwen3_asr 转写。
///
/// 一期后端收敛：文件转写（CLI `asr transcribe` / Tauri `transcribe_audio` /
/// `transcribe_reference_audio`）与听写（[`crate::asr::dictate`]）统一走
/// [`AudiocppAsr`]（`/v1/audio/transcriptions`，Qwen3-ASR-0.6B GGUF）。
/// 语种自动识别、标点由模型侧负责，本层只做「解码 → 上传 → 取文本」。
use crate::asr::config::ResolvedAsrConfig;
use crate::audiocpp::client::AudiocppAsr;
use std::path::Path;

/// 整段转写 wav 文件，返回文本（trim 后，空结果报错）。
///
/// 流程：hound 解码为 mono f32（多声道按帧平均，采样率保留文件原值）→
/// [`AudiocppAsr::new`]（preflight + sidecar 租约）→ multipart 上传（服务端按
/// 上传采样率自行处理，无需客户端重采样）。wav 读取/校验先于引擎构造，坏文件
/// 不触发 sidecar 启动。
pub fn transcribe_wav_offline(cfg: &ResolvedAsrConfig, wav: &Path) -> Result<String, String> {
    let (samples, sample_rate) = crate::audio::read_wav_mono(wav)?;
    let asr = AudiocppAsr::new(cfg.clone())?;
    let text = asr
        .transcribe(&samples, sample_rate as i32)
        .map_err(|e| e.to_user_message())?;
    if text.is_empty() {
        return Err("未能识别出有效文本，请换一段更清晰的音频".to_string());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audiocpp::client::transcription_request_parts;

    #[test]
    fn transcription_request_uses_qwen3_model_id() {
        let (model, language) = transcription_request_parts(&None);
        assert_eq!(model, "qwen3-asr-0.6b");
        assert_eq!(language, None, "语言自动识别，不显式传");
    }

    /// 显式语言透传（量化下自动语种识别不可靠时的兜底，上游文档明示）。
    #[test]
    fn transcription_request_passes_explicit_language() {
        let (model, language) = transcription_request_parts(&Some("zh".to_string()));
        assert_eq!(model, "qwen3-asr-0.6b");
        assert_eq!(language.as_deref(), Some("zh"), "显式语言应原样透传");
    }

    /// 纯空白语言等价未配置（不携带空字段）。
    #[test]
    fn transcription_request_blank_language_is_none() {
        let (_, language) = transcription_request_parts(&Some("   ".to_string()));
        assert_eq!(language, None);
    }

    /// 文件校验先于引擎构造：坏 wav 直接报解码错误，不触发 sidecar 启动。
    #[test]
    fn transcribe_rejects_missing_wav_before_engine() {
        let cfg = ResolvedAsrConfig::default();
        let err = transcribe_wav_offline(&cfg, Path::new("/nonexistent/asr-in.wav")).unwrap_err();
        assert!(
            err.contains("无法解码音频"),
            "应报 wav 解码失败，实际: {err}"
        );
    }

    /// 非 wav 内容 → 解码错误（含路径上下文）。
    #[test]
    fn transcribe_rejects_garbage_wav() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.wav");
        std::fs::write(&path, b"not a wav at all").unwrap();
        let err = transcribe_wav_offline(&ResolvedAsrConfig::default(), &path).unwrap_err();
        assert!(
            err.contains("无法解码音频") && err.contains("bad.wav"),
            "应报 wav 解码失败并带路径，实际: {err}"
        );
    }

    /// 有 RIFF 头但零样本 → 「音频为空」，同样不到引擎构造。
    #[test]
    fn transcribe_rejects_empty_wav() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let writer = hound::WavWriter::create(&path, spec).unwrap();
        writer.finalize().unwrap();
        let err = transcribe_wav_offline(&ResolvedAsrConfig::default(), &path).unwrap_err();
        assert!(err.contains("音频为空"), "应报空音频，实际: {err}");
    }
}
