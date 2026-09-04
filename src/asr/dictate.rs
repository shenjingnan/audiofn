/// 离线免提连续听写（SenseVoice/Whisper + Silero VAD 分段，每段整句转写）。
///
/// 平行于流式 `run_realtime_with`：麦克风 → 重采样 16k → Silero VAD 分段 →
/// 每段 `OfflineAsrEngine::transcribe_samples` 整句转写 → `AsrReaction`。
use crate::asr::config::ResolvedAsrConfig;
use crate::asr::offline::OfflineAsrEngine;
use crate::asr::reaction::{AsrReaction, AsrResult};
use crate::kws::model::{ProgressFn, asr_vad_asset, asr_vad_user_model_path, install_raw_file_to};
use sherpa_onnx::{SileroVadModelConfig, VadModelConfig, VoiceActivityDetector};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// 听写模型/VAD 采样率（Silero VAD 与 SenseVoice/Whisper 均为 16k）。
pub const DICTATE_MODEL_SAMPLE_RATE: i32 = 16_000;
/// VAD 语音概率阈值（sherpa `VadModelConfig` 的 Rust `Default` 全 0，必须显式填）。
pub const DICTATE_VAD_THRESHOLD: f32 = 0.5;
/// VAD 尾随静音秒数（说完一句即断段）。
pub const DICTATE_VAD_MIN_SILENCE_DURATION: f32 = 0.5;
/// VAD 最小语音时长（低于此的噪声不构成段）。
pub const DICTATE_VAD_MIN_SPEECH_DURATION: f32 = 0.25;
/// VAD 窗口（silero v4/v5 固定 512 @16k，勿改）。
pub const DICTATE_VAD_WINDOW_SIZE: i32 = 512;
/// VAD 最长语音秒数（超长段强制断段，防无限累积）。
pub const DICTATE_VAD_MAX_SPEECH_DURATION: f32 = 20.0;
/// VAD 内部结果缓冲秒数（长段转写期间段在内部排队的内存上限）。
pub const DICTATE_VAD_BUFFER_SECONDS: f32 = 30.0;

/// 听写引擎配置（VAD 参数 + 推理线程/provider）。
#[derive(Debug, Clone)]
pub struct DictateConfig {
    pub vad_model: PathBuf,
    pub vad_threshold: f32,
    pub vad_min_silence_duration: f32,
    pub vad_min_speech_duration: f32,
    pub vad_max_speech_duration: f32,
    pub vad_window_size: i32,
    pub buffer_size_in_seconds: f32,
    pub num_threads: i32,
    pub provider: String,
    pub debug: bool,
}

impl DictateConfig {
    /// 默认听写参数（VAD 常用值），推理线程/provider 走 `with_runtime` 继承。
    pub fn new(vad_model: PathBuf) -> Self {
        Self {
            vad_model,
            vad_threshold: DICTATE_VAD_THRESHOLD,
            vad_min_silence_duration: DICTATE_VAD_MIN_SILENCE_DURATION,
            vad_min_speech_duration: DICTATE_VAD_MIN_SPEECH_DURATION,
            vad_max_speech_duration: DICTATE_VAD_MAX_SPEECH_DURATION,
            vad_window_size: DICTATE_VAD_WINDOW_SIZE,
            buffer_size_in_seconds: DICTATE_VAD_BUFFER_SECONDS,
            num_threads: 2,
            provider: "cpu".to_string(),
            debug: false,
        }
    }

    /// 从 ASR 配置继承推理线程/provider/debug。
    pub fn with_runtime(mut self, cfg: &ResolvedAsrConfig) -> Self {
        self.num_threads = cfg.num_threads;
        self.provider = cfg.provider.clone();
        self.debug = cfg.debug;
        self
    }
}

/// Silero VAD 模型文件路径（`~/.zapmomo/models/silero-vad/silero_vad.onnx`）。
pub fn vad_model_path() -> PathBuf {
    asr_vad_user_model_path()
}

/// VAD 模型是否已就绪。
pub fn vad_model_present() -> bool {
    vad_model_path().is_file()
}

/// 惰性确保 VAD 模型就绪：缺失时下载 + sha256 校验 + 原子落位（幂等）。
pub fn ensure_vad_model(on_progress: &mut ProgressFn) -> Result<PathBuf, String> {
    let dest = asr_vad_user_model_path();
    install_raw_file_to(asr_vad_asset(), &dest, false, on_progress)
        .map_err(|e| format!("VAD 模型下载失败: {e}"))?;
    Ok(dest)
}

/// 构造 sherpa `VadModelConfig`（显式填全部字段，防 Rust `Default` 全 0 的坑）。
pub(crate) fn build_vad_config(cfg: &DictateConfig) -> VadModelConfig {
    VadModelConfig {
        sample_rate: DICTATE_MODEL_SAMPLE_RATE,
        num_threads: cfg.num_threads,
        provider: Some(cfg.provider.clone()),
        debug: cfg.debug,
        silero_vad: SileroVadModelConfig {
            model: Some(cfg.vad_model.to_string_lossy().to_string()),
            threshold: cfg.vad_threshold,
            min_silence_duration: cfg.vad_min_silence_duration,
            min_speech_duration: cfg.vad_min_speech_duration,
            window_size: cfg.vad_window_size,
            max_speech_duration: cfg.vad_max_speech_duration,
        },
        ten_vad: Default::default(),
    }
}

/// `SpeechSegment.start()`（采样点索引 @16k）→ 秒。
pub(crate) fn segment_start_time_seconds(start_sample: i32) -> f32 {
    start_sample as f32 / DICTATE_MODEL_SAMPLE_RATE as f32
}

/// 免提连续听写循环：麦克风 → 16k → VAD 分段 → 每段整句转写 → reaction。
///
/// `should_stop` 语义同 `run_realtime_with`：`Some(running)` 且 `running=false` 时干净退出。
pub fn run_dictate(
    cfg: &ResolvedAsrConfig,
    vad_cfg: &DictateConfig,
    device: Option<&str>,
    duration: Option<u64>,
    reaction: &mut dyn AsrReaction,
    should_stop: Option<&AtomicBool>,
) -> Result<(), String> {
    let engine = OfflineAsrEngine::new(cfg.clone())?;
    let vad =
        VoiceActivityDetector::create(&build_vad_config(vad_cfg), vad_cfg.buffer_size_in_seconds)
            .ok_or_else(|| "无法创建 VAD 检测器，请检查 silero_vad.onnx 是否就绪".to_string())?;
    let mut mic = crate::audio::start_capture(device)?;
    let mut resampler =
        crate::audio::Resampler::new(mic.device_sample_rate() as i32, DICTATE_MODEL_SAMPLE_RATE)?;

    // should_stop 标志语义 = running（true = 正在识别）；「应停止」= 标志为 false。
    let stop_requested = || should_stop.is_some_and(|f| !f.load(Ordering::Relaxed));

    println!("开始免提连续听写... (Ctrl-C 退出; --duration 可限制时长)");

    // 处理 VAD 已排队的所有语音段：拷贝样本 → pop → 整句转写 → reaction。
    let process_segments = |vad: &VoiceActivityDetector,
                            engine: &OfflineAsrEngine,
                            reaction: &mut dyn AsrReaction|
     -> Result<bool, String> {
        while !vad.is_empty() {
            let Some(seg) = vad.front() else { break };
            // SpeechSegment 是 C 指针包装：先拷贝样本再 pop（pop 释放底层内存）
            let start_sample = seg.start();
            let samples = seg.samples().to_vec();
            vad.pop();
            if stop_requested() {
                return Ok(true);
            }
            let text = match engine.transcribe_samples(&samples, DICTATE_MODEL_SAMPLE_RATE) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("听写分段转写失败（跳过该段）: {e}");
                    continue;
                }
            };
            if text.trim().is_empty() {
                continue;
            }
            let result = AsrResult {
                text,
                tokens: Vec::new(),
                timestamps: None,
                start_time: Some(segment_start_time_seconds(start_sample)),
                is_final: true,
            };
            if reaction.on_result(&result) == crate::kws::reaction::ReactionOutcome::Stop {
                return Ok(true);
            }
        }
        Ok(false)
    };

    let start = std::time::Instant::now();
    let deadline = duration.map(|secs| start + std::time::Duration::from_secs(secs));

    loop {
        if stop_requested() {
            tracing::warn!("听写退出：收到停止请求");
            break;
        }
        let raw = if let Some(dl) = deadline {
            if std::time::Instant::now() >= dl {
                break;
            }
            let timeout = dl
                .saturating_duration_since(std::time::Instant::now())
                .min(std::time::Duration::from_millis(200));
            match mic.recv_chunk_timeout(timeout) {
                Ok(raw) => Some(raw),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    tracing::warn!("听写退出：麦克风通道断开");
                    break;
                }
            }
        } else {
            mic.recv_chunk()
        };
        let Some(raw) = raw else {
            tracing::warn!("听写退出：麦克风返回 None");
            break;
        };
        let out = resampler.process(&raw, false);
        vad.accept_waveform(&out);
        if process_segments(&vad, &engine, &mut *reaction)? {
            break;
        }
    }

    // 收尾：冲刷重采样器尾部 + VAD flush 出末段语音
    let tail = resampler.process(&[], true);
    if !tail.is_empty() {
        vad.accept_waveform(&tail);
    }
    vad.flush();
    let _ = process_segments(&vad, &engine, &mut *reaction)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::config::ResolvedAsrConfig;
    use std::path::PathBuf;

    #[test]
    fn test_dictate_config_defaults() {
        let cfg = DictateConfig::new(PathBuf::from("/vad/silero_vad.onnx"));
        assert_eq!(cfg.vad_model, PathBuf::from("/vad/silero_vad.onnx"));
        assert_eq!(cfg.vad_threshold, 0.5);
        assert_eq!(cfg.vad_min_silence_duration, 0.5);
        assert_eq!(cfg.vad_min_speech_duration, 0.25);
        assert_eq!(cfg.vad_window_size, 512);
        assert_eq!(cfg.vad_max_speech_duration, 20.0);
        assert_eq!(cfg.buffer_size_in_seconds, 30.0);
    }

    /// 关键回归护栏：sherpa `VadModelConfig` 的 Rust `Default` 全 0，
    /// 必须显式填，否则 VAD 行为异常或创建失败。
    #[test]
    fn test_build_vad_config_explicit_defaults() {
        let cfg = DictateConfig::new(PathBuf::from("/vad/silero_vad.onnx"));
        let v = build_vad_config(&cfg);
        assert_eq!(v.sample_rate, 16000);
        assert_eq!(v.num_threads, cfg.num_threads);
        assert_eq!(v.provider.as_deref(), Some(cfg.provider.as_str()));
        assert_eq!(v.silero_vad.threshold, 0.5);
        assert_eq!(v.silero_vad.min_silence_duration, 0.5);
        assert_eq!(v.silero_vad.min_speech_duration, 0.25);
        assert_eq!(v.silero_vad.window_size, 512);
        assert_eq!(v.silero_vad.max_speech_duration, 20.0);
        assert_eq!(v.silero_vad.model.as_deref(), Some("/vad/silero_vad.onnx"));
    }

    #[test]
    fn test_dictate_config_with_runtime() {
        let base = ResolvedAsrConfig::default();
        let cfg = DictateConfig::new(PathBuf::from("/vad/v.onnx")).with_runtime(&base);
        assert_eq!(cfg.num_threads, base.num_threads);
        assert_eq!(cfg.provider, base.provider);
        assert_eq!(cfg.debug, base.debug);
    }

    #[test]
    fn test_segment_start_time_seconds() {
        // segment.start() 是采样点索引（@16k）→ 秒
        assert_eq!(segment_start_time_seconds(0), 0.0);
        assert_eq!(segment_start_time_seconds(16_000), 1.0);
        assert_eq!(segment_start_time_seconds(80_000), 5.0);
    }
}
