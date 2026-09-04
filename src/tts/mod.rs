/// 文本转语音（TTS）。
///
/// 引擎统一走 audio.cpp sidecar（`crate::audiocpp`，Qwen3-TTS GGUF 模型经 HTTP）；
/// `TtsEngine` 是 audiocpp 客户端的门面（预检 / 播放参数 / 语速重采样收敛在此）。
///
/// 设计上对齐 ASR：模型库下载、配置解析、引擎「逐文件预检 + install-model 提示」
/// 等模式保持一致；进度通过回调（`FnMut(f32) -> bool`）暴露（请求前后探询——
/// HTTP 在途请求无法中断）。
pub mod config;
pub mod reaction;
pub mod voice;
pub mod voice_store;

use crate::audiocpp::client::AudiocppTts;
use config::ResolvedTtsConfig;
use std::path::{Path, PathBuf};

pub use voice::TtsVoice;

/// 合成时的「说话人/音色」参数：`Sid`（缺省音色/auto voice）、
/// `Reference`（参考音频克隆）或 `Named`（具名音色）。
#[derive(Debug, Clone, PartialEq)]
pub enum TtsVoiceParams {
    /// speaker id（克隆族缺省音色时传 0 = server auto voice）
    Sid(i32),
    /// 参考音频 + 逐字转写（Qwen3-TTS 等克隆模型）
    Reference {
        wav_path: PathBuf,
        reference_text: String,
    },
    /// 具名音色（audio.cpp 后端的 preset/voice_dir 通道）
    Named(String),
}

/// 文本转语音引擎（audio.cpp sidecar 门面）。
///
/// 一期裁剪后仅剩 audiocpp 一条推理路径，门面保留 `TtsEngine` 类型名与既有方法
/// 签名，收敛两件事：构造前预检（[`config::preflight`]）与语速重采样语义。
/// 引擎 `Send`，可按值 move 进合成线程；参考音色（声音克隆的音色来源）在每次
/// 合成时按需传入，引擎可复用、可切换音色。所有方法接收 `&self`。
pub struct TtsEngine {
    inner: AudiocppTts,
}

impl TtsEngine {
    /// 构造引擎：先做就绪预检（文件清单见 [`config::preflight`]），再定位引擎并
    /// lease sidecar 进程（含 spawn + 健康检查）。
    pub fn new(cfg: ResolvedTtsConfig) -> Result<Self, String> {
        config::preflight(&cfg)?;
        Ok(Self {
            inner: AudiocppTts::new(cfg)?,
        })
    }

    pub fn config(&self) -> &ResolvedTtsConfig {
        self.inner.config()
    }

    /// 合成输出的采样率（Hz）。初值为模型族固定值（24k），
    /// 首次合成后按响应 wav 头校准。
    pub fn sample_rate(&self) -> i32 {
        self.inner.sample_rate()
    }

    /// 把文本合成为 PCM 波形（f32，采样率见 [`Self::sample_rate`]）。
    ///
    /// 语速不传给模型：模型按 1.0 合成，目标语速通过对输出重采样实现（见
    /// [`apply_speed_to_samples`]）。
    pub fn synthesize(
        &self,
        text: &str,
        speed: f32,
        voice: &TtsVoiceParams,
    ) -> Result<Vec<f32>, String> {
        self.inner.synthesize(text, speed, voice)
    }

    /// 把文本合成为 PCM，并在合成过程中回调进度（0..1）。
    ///
    /// 请求前探询（返回 `false` 则不发请求）——HTTP 在途请求无法中断，这是
    /// sidecar 语义下的取消边界。语速同 [`Self::synthesize`]。
    pub fn synthesize_with_progress<F>(
        &self,
        text: &str,
        speed: f32,
        voice: &TtsVoiceParams,
        mut progress: F,
    ) -> Result<Vec<f32>, String>
    where
        F: FnMut(f32) -> bool + 'static,
    {
        if !progress(0.05) {
            return Err("已取消".to_string());
        }
        let out = self.inner.synthesize(text, speed, voice)?;
        let _ = progress(1.0);
        Ok(out)
    }

    /// 把文本合成为 wav 文件。
    pub fn synthesize_to_wav(
        &self,
        text: &str,
        speed: f32,
        voice: &TtsVoiceParams,
        out_path: &Path,
    ) -> Result<(), String> {
        self.synthesize_to_wav_with_progress(text, speed, voice, out_path, |_p| true)
            .map(|_| ())
    }

    /// 把文本合成为 wav 文件，并在合成过程中回调进度（0..1）。
    ///
    /// 返回采样点数（已应用语速），便于调用方换算音频时长（`samples / sample_rate`）。
    pub fn synthesize_to_wav_with_progress<F>(
        &self,
        text: &str,
        speed: f32,
        voice: &TtsVoiceParams,
        out_path: &Path,
        mut progress: F,
    ) -> Result<usize, String>
    where
        F: FnMut(f32) -> bool + 'static,
    {
        if !progress(0.05) {
            return Err("已取消".to_string());
        }
        let samples = self.inner.synthesize(text, speed, voice)?;
        let sample_rate = self.inner.sample_rate();
        crate::audio::write_wav_f32(out_path, sample_rate as u32, &samples)?;
        let _ = progress(1.0);
        Ok(samples.len())
    }
}

/// 对合成输出应用语速：模型以 1.0 合成后，把样本重采样到 `sample_rate / speed`，
/// 再以 `sample_rate` 写回，从而改变时长（speed>1 更快、样本更少；speed<1 更慢、样本更多）。
///
/// 语速不传给模型，统一在输出侧重采样：`crate::audiocpp::client` 在合成输出上
/// 调用本函数，两条合成路径（引擎门面 / 客户端直连）的语速行为一致。
pub(crate) fn apply_speed_to_samples(
    samples: &[f32],
    sample_rate: i32,
    speed: f32,
) -> Result<Vec<f32>, String> {
    if speed <= 0.0 {
        return Err(format!("语速必须为正数，当前 {speed}"));
    }
    if (speed - 1.0).abs() < 1e-6 {
        return Ok(samples.to_vec());
    }
    let out_rate = (sample_rate as f32 / speed) as i32;
    let mut resampler = crate::audio::Resampler::new(sample_rate, out_rate)?;
    Ok(resampler.process(samples, true))
}

/// 生成唯一的 TTS 输出 wav 路径：`~/.audiofn/tts/tts-<毫秒时间戳>.wav`
pub fn default_output_path() -> PathBuf {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    crate::config::settings::get_tts_output_dir().join(format!("tts-{millis}.wav"))
}

/// 目标目录是否已装好任一已收录 TTS 模型族（按族 GGUF 主文件名探测）。
pub fn is_installed(dir: &Path) -> bool {
    crate::audiocpp::families::detect_family_in_dir(dir).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 不存在的模型目录：TtsEngine::new 先预检，报缺 GGUF 并给出可执行的
    /// install-model 提示（不发起进程/网络）。
    #[test]
    fn test_engine_new_missing_model_errors() {
        let cfg = ResolvedTtsConfig {
            model_dir: PathBuf::from("/nonexistent/model"),
            ..ResolvedTtsConfig::default()
        };
        let err = TtsEngine::new(cfg.clone()).err().unwrap();
        assert!(err.contains("install-model"), "err: {err}");
        assert!(
            err.contains("qwen3-tts-12hz-0.6b-base-q8_0.gguf"),
            "err: {err}"
        );
    }

    /// 已装好族 GGUF 的目录 `is_installed` 为真（模型库安装态复用）。
    #[test]
    fn test_is_installed_detects_family_gguf() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_installed(dir.path()));
        std::fs::write(
            dir.path().join("qwen3-tts-12hz-1.7b-base-q8_0_v2.gguf"),
            b"x",
        )
        .unwrap();
        assert!(is_installed(dir.path()));
    }

    #[test]
    #[ignore = "需要 qwen3-tts GGUF 在 QWEN3_TTS_E2E_DIR 目录 + audiocpp 引擎可定位 + 参考音频 QWEN3_TTS_E2E_REF"]
    fn test_qwen3_tts_synthesize_produces_audio() {
        // E2E：QWEN3_TTS_E2E_DIR=/path/to/qwen3-tts QWEN3_TTS_E2E_REF=/path/to/ref.wav \
        //   QWEN3_TTS_E2E_REF_TEXT="转写" cargo test -- --ignored
        let Some(dir) = std::env::var("QWEN3_TTS_E2E_DIR").ok() else {
            eprintln!("跳过：未设置 QWEN3_TTS_E2E_DIR");
            return;
        };
        let Some(ref_wav) = std::env::var("QWEN3_TTS_E2E_REF").ok() else {
            eprintln!("跳过：未设置 QWEN3_TTS_E2E_REF（Base 版必须参考音频）");
            return;
        };
        let kind = match std::env::var("QWEN3_TTS_E2E_SIZE").as_deref() {
            Ok("17") => config::TtsModelKind::Qwen3Tts17,
            _ => config::TtsModelKind::Qwen3Tts06,
        };
        let cfg = config::ResolvedTtsConfig {
            backend: config::TtsBackendKind::Audiocpp,
            model_type: kind,
            model_dir: PathBuf::from(&dir),
            provider: std::env::var("QWEN3_TTS_E2E_PROVIDER")
                .unwrap_or_else(|_| "metal".to_string()),
            ..config::ResolvedTtsConfig::default()
        };
        let engine = TtsEngine::new(cfg).unwrap();
        assert_eq!(engine.sample_rate(), 24_000, "qwen3_tts 固定 24kHz");

        let voice = TtsVoiceParams::Reference {
            wav_path: PathBuf::from(ref_wav),
            reference_text: std::env::var("QWEN3_TTS_E2E_REF_TEXT").unwrap_or_else(|_| {
                "那还是36年前, 1987年. 我呢考上了武汉大学的计算机系.".to_string()
            }),
        };
        let started = std::time::Instant::now();
        let samples = engine
            .synthesize(
                "你好，我是 AudioFn 语音伙伴，正在验证 Qwen3-TTS 中文合成。",
                1.0,
                &voice,
            )
            .unwrap();
        let elapsed = started.elapsed().as_secs_f32();
        assert!(!samples.is_empty(), "合成音频不应为空");
        let duration = samples.len() as f32 / engine.sample_rate() as f32;
        eprintln!(
            "qwen3_tts e2e ({kind:?}): {:.2}s 音频 / {:.2}s 合成 (RTF {:.2})",
            duration,
            elapsed,
            elapsed / duration
        );
    }

    #[test]
    fn test_apply_speed_identity() {
        let samples = vec![0.1f32; 24000];
        let out = apply_speed_to_samples(&samples, 24000, 1.0).unwrap();
        assert_eq!(out.len(), 24000);
    }

    #[test]
    fn test_apply_speed_faster_shortens() {
        // speed 1.3 → 样本数 ≈ 1/1.3（24k / 1.3 ≈ 18461 目标采样率）
        let samples = vec![0.1f32; 24000];
        let out = apply_speed_to_samples(&samples, 24000, 1.3).unwrap();
        assert!(
            (out.len() as i64 - 18461).abs() <= 64,
            "speed 1.3 len={}",
            out.len()
        );
    }

    #[test]
    fn test_apply_speed_slower_lengthens() {
        // speed 0.7 → 样本数 ≈ 1/0.7（24k / 0.7 ≈ 34285 目标采样率）
        let samples = vec![0.1f32; 24000];
        let out = apply_speed_to_samples(&samples, 24000, 0.7).unwrap();
        assert!(
            (out.len() as i64 - 34285).abs() <= 64,
            "speed 0.7 len={}",
            out.len()
        );
    }

    #[test]
    fn test_apply_speed_rejects_non_positive() {
        assert!(apply_speed_to_samples(&[0.0f32], 24000, 0.0).is_err());
        assert!(apply_speed_to_samples(&[0.0f32], 24000, -1.0).is_err());
    }
}
