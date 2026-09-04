/// 语音识别（ASR）。
///
/// 一期后端收敛为 audiocpp sidecar 的 qwen3_asr（Qwen3-ASR-0.6B）：
/// - [`offline`]：整段文件转写（CLI `asr test` / Tauri `transcribe_audio`）；
/// - [`dictate`]：麦克风免提听写（整段录制，停止后一次转写）；
/// - [`config`]：配置解析与模型 preflight。
///
/// 识别结果经可插拔的 [`AsrReaction`] 回调（CLI 打印 / GUI 发事件给前端）。
pub mod config;
pub mod dictate;
pub mod offline;

use serde::Serialize;
use std::path::{Path, PathBuf};

pub use crate::model_library::asset::{DownloadProgress, DownloadStage, ModelError, ProgressFn};

/// 一次识别结果（owned 结构，不把后端类型泄漏到公开 API）。
///
/// `Serialize` 供桌面 GUI 通过 Tauri 事件把结果发给前端。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsrResult {
    /// 转写文本
    pub text: String,
    /// 起始时间（秒）
    pub start_time: Option<f32>,
    /// 是否为最终结果
    pub is_final: bool,
}

/// 反应控制信号：`Continue` = 继续识别，`Stop` = 停止识别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionOutcome {
    Continue,
    Stop,
}

/// 可插拔的识别反应接口。`Send` 允许反应被移动到其他线程（如 GUI 主线程）。
pub trait AsrReaction: Send {
    /// 产出一段转写文本时回调。返回 `Stop` 可终止识别循环。
    fn on_result(&mut self, result: &AsrResult) -> ReactionOutcome;
}

/// 默认反应：控制台打印 + tracing 日志。
pub struct ConsoleAsrReaction;

impl AsrReaction for ConsoleAsrReaction {
    fn on_result(&mut self, result: &AsrResult) -> ReactionOutcome {
        println!("[识别] {}", result.text);
        tracing::info!(text = %result.text, "ASR result: {}", result.text);
        ReactionOutcome::Continue
    }
}

/// ASR 模型安装目录：`~/.zapmomo/models/<name>`。
pub fn user_model_dir() -> PathBuf {
    crate::model_library::asset::asr_user_model_dir()
}

/// 目标目录是否已装好 ASR 模型（探测式：按目录内容探测，模型无关）。
pub fn is_installed(dir: &Path) -> bool {
    config::asr_files_present(dir)
}

/// 安装 ASR 模型到 `dest_dir`（默认 `~/.zapmomo/models/<name>`）。
///
/// 幂等：已安装且 `force` 为假时直接返回。下载过程中回调进度。
pub fn install_model_to(
    dest_dir: &Path,
    force: bool,
    on_progress: &mut ProgressFn,
) -> Result<(), ModelError> {
    crate::model_library::asset::install_asset_to(
        crate::model_library::asset::asr_asset(),
        dest_dir,
        force,
        on_progress,
        &config::REQUIRED_FILES,
    )
}

/// 标点模型安装目录：`~/.zapmomo/models/<标点模型名>`。
pub fn punctuation_user_model_dir() -> PathBuf {
    crate::model_library::asset::punctuation_user_model_dir()
}

/// 安装标点模型到 `dest_dir`（默认 `~/.zapmomo/models/<标点模型名>`）。
///
/// 幂等：已安装且 `force` 为假时直接返回。下载过程中回调进度。
pub fn install_punctuation_model_to(
    dest_dir: &Path,
    force: bool,
    on_progress: &mut ProgressFn,
) -> Result<(), ModelError> {
    crate::model_library::asset::install_punctuation_model_to(
        dest_dir,
        force,
        on_progress,
        &config::PUNCT_REQUIRED_FILES,
    )
}

/// 离线转写 wav 文件，返回转写文本（不依赖麦克风，走 audiocpp qwen3_asr）。
///
/// 供 CLI 离线验证与「参考音频自动转写」复用。
pub fn transcribe_wav(cfg: &config::ResolvedAsrConfig, wav: &Path) -> Result<String, String> {
    offline::transcribe_wav_offline(cfg, wav)
}

/// 离线转写 wav 文件并打印结果（不依赖麦克风）。
///
/// 用于验证模型与整条链路：对模型自带 `test_wavs/*.wav` 应输出对应文本。
pub fn run_offline(cfg: &config::ResolvedAsrConfig, wav: &Path) -> Result<(), String> {
    let text = transcribe_wav(cfg, wav)?;
    println!("[识别] {text}");
    Ok(())
}

/// 模型目录内默认测试音频：`test_wavs/0.wav` → `1.wav` → `zh.wav` → 字母序第一个 wav。
///
/// 供 CLI `asr test` 与 GUI「测试识别」在未指定 wav 时自动挑一条示例音频。
pub fn default_test_wav(model_dir: &Path) -> Option<PathBuf> {
    let test_dir = model_dir.join("test_wavs");
    let Ok(entries) = std::fs::read_dir(&test_dir) else {
        return None;
    };
    let mut wavs: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| n.ends_with(".wav"))
        .collect();
    wavs.sort();
    for preferred in ["0.wav", "1.wav", "zh.wav"] {
        if wavs.iter().any(|n| n == preferred) {
            return Some(test_dir.join(preferred));
        }
    }
    wavs.into_iter().next().map(|n| test_dir.join(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_test_wav_prefers_preferred() {
        let dir = tempfile::tempdir().unwrap();
        let test_dir = dir.path().join("test_wavs");
        std::fs::create_dir_all(&test_dir).unwrap();
        // 无示例音频 → None
        assert_eq!(default_test_wav(dir.path()), None);
        // 只有 en/ja → 字母序第一个
        std::fs::write(test_dir.join("ja.wav"), b"x").unwrap();
        std::fs::write(test_dir.join("en.wav"), b"x").unwrap();
        assert_eq!(default_test_wav(dir.path()), Some(test_dir.join("en.wav")));
        // 有 zh → 优先 zh（SenseVoice 中文示例）
        std::fs::write(test_dir.join("zh.wav"), b"x").unwrap();
        assert_eq!(default_test_wav(dir.path()), Some(test_dir.join("zh.wav")));
        // 有 0.wav → 优先 0.wav（zipformer/whisper 包）
        std::fs::write(test_dir.join("0.wav"), b"x").unwrap();
        assert_eq!(default_test_wav(dir.path()), Some(test_dir.join("0.wav")));
    }
}
