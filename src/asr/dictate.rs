/// 免提听写：麦克风整段录制（一期不做 VAD 分段），停止后整段送 audiocpp qwen3_asr 转写。
///
/// 流程：[`crate::audio::start_capture`] 采集 → [`DictateRecorder`] 边采边重采样到
/// 16k mono → 停止（回车 / Ctrl-C / `--duration` / 外部停止标志）→ 临时 wav →
/// [`crate::asr::transcribe_wav`] 一次整段转写 → [`AsrReaction`] 输出全文。
use crate::asr::config::ResolvedAsrConfig;
use crate::asr::{AsrReaction, AsrResult};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// 听写录音采样率（qwen3_asr 输入为 16k mono）。
pub const DICTATE_SAMPLE_RATE: i32 = 16_000;

/// 采集轮询间隔：停止信号生效的上限延迟，同时避免忙等。
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

/// 免提听写：麦克风整段录制，停止后一次性转写并经 reaction 输出全文。
///
/// `should_stop` 语义：`Some(running)` 且 `running=false` 时停止录音并转写
/// （桌面端由 `stop_asr_dictate` 置位；CLI 由回车 / Ctrl-C / `--duration` 触发）。
pub fn run_dictate(
    cfg: &ResolvedAsrConfig,
    device: Option<&str>,
    duration: Option<u64>,
    reaction: &mut dyn AsrReaction,
    should_stop: Option<&AtomicBool>,
) -> Result<(), String> {
    let stop_requested = || should_stop.is_some_and(|f| !f.load(Ordering::Relaxed));

    let mut mic = crate::audio::start_capture(device)?;
    let mut recorder = DictateRecorder::new(mic.device_sample_rate())?;

    let start = std::time::Instant::now();
    let deadline = duration.map(|secs| start + std::time::Duration::from_secs(secs));
    loop {
        if stop_requested() {
            tracing::info!("听写退出：收到停止请求");
            break;
        }
        let now = std::time::Instant::now();
        if deadline.is_some_and(|dl| now >= dl) {
            tracing::info!("听写退出：到达时长上限");
            break;
        }
        // 统一超时收块：无 deadline 时也按轮询间隔收，保证停止信号及时生效
        let timeout = deadline
            .map(|dl| dl.saturating_duration_since(now))
            .unwrap_or(POLL_INTERVAL)
            .min(POLL_INTERVAL);
        match mic.recv_chunk_timeout(timeout) {
            Ok(chunk) => recorder.push(&chunk),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                tracing::warn!("听写退出：麦克风通道断开");
                break;
            }
        }
    }

    // 先释放麦克风（转写期间不占设备，桌面端也能立刻重启新设备的录音），
    // 再冲刷重采样尾部得到完整 16k 样本
    drop(mic);
    let samples = recorder.finish();
    if samples.is_empty() {
        return Err("未采集到有效音频".to_string());
    }
    let seconds = samples.len() as f32 / DICTATE_SAMPLE_RATE as f32;
    tracing::info!(seconds, "听写录音结束，整段转写开始");

    let text = transcribe_recorded(cfg, &samples)?;
    let result = AsrResult {
        text,
        start_time: Some(0.0),
        is_final: true,
    };
    // 全文即最终结果，reaction 的 Stop 语义在单次输出后无后续可终止
    let _ = reaction.on_result(&result);
    Ok(())
}

/// 整段录音累积器：设备采样率 f32 块 → 16k mono 样本。
///
/// 内部 [`crate::audio::Resampler`] 跨块相位连续：分块喂入 + 末尾冲刷与一次性
/// 喂入的产出逐样本一致（无时长漂移，见 `audio` 模块的相位连续性测试）。
struct DictateRecorder {
    resampler: crate::audio::Resampler,
    /// 累积的 16k mono 样本
    samples: Vec<f32>,
}

impl DictateRecorder {
    fn new(device_sample_rate: u32) -> Result<Self, String> {
        Ok(Self {
            resampler: crate::audio::Resampler::new(
                device_sample_rate as i32,
                DICTATE_SAMPLE_RATE,
            )?,
            samples: Vec::new(),
        })
    }

    /// 追加一帧设备采样率的原始 mono 块（重采样到 16k 后累积）。
    fn push(&mut self, chunk: &[f32]) {
        self.samples.extend(self.resampler.process(chunk, false));
    }

    /// 结束录音：冲刷重采样尾部，返回完整的 16k mono 样本。
    fn finish(&mut self) -> Vec<f32> {
        let mut samples = std::mem::take(&mut self.samples);
        samples.extend(self.resampler.process(&[], true));
        samples
    }
}

/// 整段转写：16k mono 样本 → 临时 wav → [`crate::asr::transcribe_wav`] → 删除临时文件。
///
/// 与文件转写（`asr transcribe` / GUI「测试识别」）共用同一条 audiocpp 路径；wav 仅为
/// 转写输入的中间产物，转写结束（含失败）即清理，不在用户目录留存录音。
fn transcribe_recorded(cfg: &ResolvedAsrConfig, samples: &[f32]) -> Result<String, String> {
    let wav = temp_wav_path();
    write_dictate_wav(&wav, samples)?;
    let result = crate::asr::transcribe_wav(cfg, &wav);
    let _ = std::fs::remove_file(&wav);
    result
}

/// 把整段 16k mono 样本写成 16-bit PCM wav（[`crate::audio::write_wav_f32`] 的定参封装）。
fn write_dictate_wav(path: &std::path::Path, samples: &[f32]) -> Result<(), String> {
    crate::audio::write_wav_f32(path, DICTATE_SAMPLE_RATE as u32, samples)
}

/// 听写录音的临时落盘路径（系统临时目录，进程 id + 纳秒时间戳避免并发撞名）。
fn temp_wav_path() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "audiofn-dictate-{}-{nanos}.wav",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 分块喂入 + 冲刷应与一次性喂入产出完全一致（48k 麦克风 → 16k）。
    #[test]
    fn recorder_chunked_matches_one_shot() {
        let input: Vec<f32> = (0..48_000).map(|i| ((i as f32) * 0.013).sin()).collect();
        let mut one_shot = crate::audio::Resampler::new(48_000, DICTATE_SAMPLE_RATE).unwrap();
        let expected = one_shot.process(&input, true);

        let mut recorder = DictateRecorder::new(48_000).unwrap();
        for chunk in input.chunks(480) {
            recorder.push(chunk);
        }
        let got = recorder.finish();

        assert_eq!(got.len(), expected.len(), "1 秒 @48k 应产出 16000 个样本");
        assert_eq!(got.len(), DICTATE_SAMPLE_RATE as usize);
        for (i, (a, b)) in got.iter().zip(expected.iter()).enumerate() {
            assert!((a - b).abs() < 1e-6, "样本 {i} 不一致: {a} vs {b}");
        }
    }

    /// 同采样率直通：16k 设备录音不重采样，样本逐个保留。
    #[test]
    fn recorder_passthrough_at_16k() {
        let mut recorder = DictateRecorder::new(16_000).unwrap();
        recorder.push(&[0.25, -0.5, 0.75]);
        recorder.push(&[1.0]);
        assert_eq!(recorder.finish(), vec![0.25, -0.5, 0.75, 1.0]);
    }

    /// 空录音（未收到任何块）→ 空样本 → 上层报「未采集到有效音频」。
    #[test]
    fn recorder_empty_recording_yields_empty() {
        let mut recorder = DictateRecorder::new(44_100).unwrap();
        assert!(recorder.finish().is_empty());
    }

    /// 录音样本 → wav 落盘：路径可写、采样率 16k、mono、样本数不漂移。
    #[test]
    fn written_wav_is_16k_mono_with_same_length() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dictate.wav");
        let samples: Vec<f32> = (0..DICTATE_SAMPLE_RATE)
            .map(|i| ((i as f32) / 100.0).sin())
            .collect();
        write_dictate_wav(&path, &samples).unwrap();
        assert!(path.is_file(), "wav 应落在指定路径");

        let (loaded, rate) = crate::audio::read_wav_mono(&path).unwrap();
        assert_eq!(rate, DICTATE_SAMPLE_RATE as u32);
        assert_eq!(loaded.len(), samples.len(), "16k → 16k 不应有时长漂移");
        for (a, b) in loaded.iter().zip(samples.iter()) {
            assert!((a - b).abs() < 1e-4, "量化误差内应一致");
        }
    }

    /// 临时 wav 路径：落在系统临时目录、`.wav` 结尾且两次生成不撞名。
    #[test]
    fn temp_wav_path_is_unique_and_wav() {
        let a = temp_wav_path();
        let b = temp_wav_path();
        assert_eq!(a.parent(), Some(std::env::temp_dir().as_path()));
        assert!(a.file_name().unwrap().to_str().unwrap().ends_with(".wav"));
        assert_ne!(a, b, "并发听写会话的临时文件不应撞名");
    }
}
