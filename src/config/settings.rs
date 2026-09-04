/// Settings - TOML 配置管理
///
/// 提供通用的配置读写功能，支持 ${env.VAR} 环境变量引用。
/// 配置文件存储在 `~/.audiofn/settings.toml`。
use crate::config::shortcuts::ShortcutsSettings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::RwLock;
use std::time::SystemTime;

const PROJECT_DIR: &str = ".audiofn";
const SETTINGS_FILE: &str = "settings.toml";

/// 获取用户 home 目录（跨平台：macOS/Linux 用 $HOME，Windows 用 %USERPROFILE%）
pub fn get_home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string())
        .into()
}

/// 获取配置目录路径
pub fn get_settings_dir() -> PathBuf {
    get_home_dir().join(PROJECT_DIR)
}

/// 获取设置文件路径
pub fn get_settings_path() -> PathBuf {
    get_settings_dir().join(SETTINGS_FILE)
}

/// 获取模型目录路径：`<data_dir>/models`（data_dir 未设置时为 `~/.audiofn/models`）。
///
/// 模型统一安装到用户目录，不随仓库/安装包分发。
pub fn get_models_dir() -> PathBuf {
    get_data_dir()
        .unwrap_or_else(get_settings_dir)
        .join("models")
}

/// 旧版默认模型根 `~/.audiofn/models`：`data_dir` 指向别处时返回 `Some`，
/// 供双根扫描/默认目录回退/迁移定位存量安装；未自定义时返回 `None`。
pub fn legacy_models_dir() -> Option<PathBuf> {
    let default = get_settings_dir().join("models");
    (get_models_dir() != default).then_some(default)
}

pub fn strip_prefix_ci<'a>(path: &'a Path, prefix: &Path) -> Option<&'a Path> {
    let p = path.as_os_str().to_str()?;
    let q = prefix.as_os_str().to_str()?;
    if cfg!(windows) {
        // 归一化分隔符 + 大小写（`/`↔`\` 一一对应，长度不变），再比较前缀；
        // 前缀尾部多余分隔符先去掉，边界判断才不受影响
        let pl = p.replace('/', "\\").to_lowercase();
        let ql = q
            .replace('/', "\\")
            .to_lowercase()
            .trim_end_matches('\\')
            .to_string();
        // 前缀后必须是分隔符（归一化后）或路径恰好等于前缀
        let boundary =
            pl.len() == ql.len() || pl.get(ql.len()..).is_some_and(|r| r.starts_with('\\'));
        if ql.is_empty() || !pl.starts_with(&ql) || !boundary {
            return None;
        }
        // ql.len() 是归一化后的前缀长度；原始 p 里前缀部分分隔符未变长，get 切安全
        let rest = p.get(ql.len()..)?;
        Some(Path::new(rest.trim_start_matches(['/', '\\'])))
    } else {
        let q = q.trim_end_matches('/');
        p.strip_prefix(q).and_then(|rest| {
            if rest.is_empty() || rest.starts_with('/') {
                Some(Path::new(rest.trim_start_matches('/')))
            } else {
                None
            }
        })
    }
}

/// `data_dir` 解析缓存：`(settings 路径, mtime, len, 解析结果)`。
///
/// `get_models_dir` 调用高频（系统资源 30s 轮询 / 模型列表 / 每次下载安装），
/// 不能每次读 TOML；以 settings.toml 的 mtime + 文件大小为键，手改文件也会自动失效
/// （mtime 同秒精度不足时，大小变化兜底）。应用内写入经 `save_settings` 主动刷新缓存。
type DataDirCacheValue = Option<(PathBuf, Option<SystemTime>, Option<u64>, Option<PathBuf>)>;
static DATA_DIR_CACHE: LazyLock<RwLock<DataDirCacheValue>> = LazyLock::new(|| RwLock::new(None));

/// 读 settings.toml 的 (mtime, len)（文件不存在/读取失败 → `None`）。
fn settings_mtime_len() -> (Option<SystemTime>, Option<u64>) {
    match std::fs::metadata(get_settings_path()) {
        Ok(m) => (m.modified().ok(), Some(m.len())),
        Err(_) => (None, None),
    }
}

/// 解析 `data_dir` 设置（支持 `${env.VAR}` 引用）。
///
/// 未设置 / 空串 / 相对路径 / env 解析失败 → `None`（回退默认根 `~/.audiofn`，
/// 调用方拿 `PathBuf` 的签名不能 Err，降级并 `warn`）。
pub fn get_data_dir() -> Option<PathBuf> {
    let path = get_settings_path();
    let (mtime, len) = settings_mtime_len();
    // 快路径：路径 + mtime + 大小都未变，直接用缓存
    if let Some((cached_path, cached_mtime, cached_len, cached_value)) =
        &*DATA_DIR_CACHE.read().unwrap_or_else(|e| e.into_inner())
        && *cached_path == path
        && *cached_mtime == mtime
        && *cached_len == len
    {
        return cached_value.clone();
    }
    // 慢路径：读 settings 解析（失败一律回退默认根）
    let resolved = load_settings()
        .ok()
        .flatten()
        .and_then(|cfg| cfg.data_dir)
        .and_then(|raw| match resolve_env_ref(&raw) {
            Ok(dir) if dir.trim().is_empty() => None,
            Ok(dir) => {
                let p = strip_verbatim_prefix(PathBuf::from(&dir));
                if p.is_absolute() {
                    Some(p)
                } else {
                    tracing::warn!("data_dir 需为绝对路径，当前值 {dir:?}，回退默认目录");
                    None
                }
            }
            Err(e) => {
                tracing::warn!("data_dir 解析失败，回退默认目录：{e}");
                None
            }
        });
    *DATA_DIR_CACHE.write().unwrap_or_else(|e| e.into_inner()) =
        Some((path, mtime, len, resolved.clone()));
    resolved
}

/// 清空 `data_dir` 缓存：写入 data_dir 后调用，确保后续读取立即可见（不等 mtime）。
pub fn refresh_data_dir_cache() {
    *DATA_DIR_CACHE.write().unwrap_or_else(|e| e.into_inner()) = None;
}

/// 剥离 Windows verbatim 前缀：`std::fs::canonicalize` 在 Windows 上返回
/// `\\?\C:\...` / `\\?\UNC\server\share` 形式，若原样落盘，后续「挂载点前缀匹配」
/// 会失配（`Path::starts_with` 逐组件比较，`VerbatimDisk(C)` ≠ `Disk(C)`，
/// 导致磁盘空间查询误报 0）。转回普通形式 `C:\...` / `\\server\share`；
/// 非 verbatim 路径与其他平台原样返回。
pub(crate) fn strip_verbatim_prefix(p: PathBuf) -> PathBuf {
    if !cfg!(windows) {
        return p;
    }
    let Some(s) = p.to_str() else {
        return p;
    };
    if let Some(rest) = s.strip_prefix(r#"\\?\UNC\"#) {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r#"\\?\"#) {
        PathBuf::from(rest)
    } else {
        p
    }
}

/// 测试专用：重置 data_dir 缓存，避免跨用例污染。
#[cfg(test)]
pub(crate) fn reset_data_dir_cache_for_test() {
    refresh_data_dir_cache();
}

/// 获取 TTS 合成音频输出目录：`~/.audiofn/tts`（供前端 asset 协议播放）。
pub fn get_tts_output_dir() -> PathBuf {
    get_settings_dir().join("tts")
}

/// 解析 ${env.VAR} 引用
///
/// - "${env.MY_VAR}" → 从环境变量 MY_VAR 读取
/// - "plain-value" → 原样返回
pub fn resolve_env_ref(value: &str) -> Result<String, String> {
    if let Some(captures) = value
        .strip_prefix("${env.")
        .and_then(|s| s.strip_suffix('}'))
    {
        let env_var = captures;
        if env_var.is_empty() {
            return Err("环境变量名称为空".to_string());
        }
        match std::env::var(env_var) {
            Ok(resolved) => Ok(resolved),
            Err(_) => Err(format!(
                "环境变量 {env_var} 未设置。请在 {SETTINGS_FILE} 中配置或设置环境变量 {env_var}。"
            )),
        }
    } else {
        Ok(value.to_string())
    }
}

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    /// 调试模式
    #[serde(default)]
    pub debug: bool,
    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// 自定义配置项（示例）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<std::collections::HashMap<String, String>>,
    /// 全局默认麦克风输入设备名（空 = 系统默认），KWS / ASR 共用；重启后免重选
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub microphone: Option<String>,
    /// 自定义数据目录（绝对路径，支持 ${env.VAR}）：模型存放在 `<data_dir>/models`；
    /// settings/日志等小文件仍留在 `~/.audiofn`。缺省 = `~/.audiofn`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
    /// 「存储位置引导」已确认过（首次下载/导入前的一次性弹窗标记，确认后不再弹）。
    #[serde(default)]
    pub storage_prompt_acknowledged: bool,
    /// 语音识别（ASR）配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asr: Option<AsrSettings>,
    /// 文本转语音（TTS）配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tts: Option<TtsSettings>,
    /// 模型库配置（用户通过「添加本地模型」注册的 external 模型等）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_library: Option<ModelLibrarySettings>,
    /// 全局快捷键配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcuts: Option<ShortcutsSettings>,
}

/// 用户「添加本地模型」注册的模型（external）。
///
/// 只保存注册路径，**不复制/不管理用户文件**；移除时只删除本条目。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LocalModel {
    /// 稳定 id（`local-` + sha256(规范化绝对路径) 前 12 位）
    pub id: String,
    /// 目录/文件基名（展示用）
    pub name: String,
    /// 能力类型：kws | asr | llm | tts
    pub model_type: String,
    /// 绝对路径（LLM 必须是具体 .gguf 文件路径）
    pub path: String,
    /// 注册时间（RFC3339）
    pub added_at: String,
    /// 显式关联的 Registry 模型 id（从 Registry 卡片导入时携带；顶部添加本地模型为 None）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_id: Option<String>,
}

/// 模型库配置段。
///
/// 只保存**用户配置**（本地注册），不保存 installed inventory。
/// "电脑上装了哪些模型" 的唯一事实来源是 `~/.audiofn/models/**/.audiofn-lib.json` 扫描。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelLibrarySettings {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_models: Vec<LocalModel>,
}

/// 语音识别（ASR）配置。
///
/// 全部字段可缺省：未配置的项在解析时回退到 `asr::config` 的内置默认值，
/// 因此这里用 `Option` 以区分「未配置」与「配置了」。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AsrSettings {
    /// 是否启用 ASR（语音会话「能识别」的前提），缺省 false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// 模型类型（sherpa-onnx 分支：zipformer/paraformer/sensevoice/whisper/qwen3_asr；
    /// 缺省按模型目录内容探测）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_type: Option<crate::asr::config::AsrModelKind>,
    /// 转写语言（SenseVoice/Whisper；缺省自动检测）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// SenseVoice 反向文本正则化（数字/标点，缺省 true）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_itn: Option<bool>,
    /// 模型目录（支持 ${env.VAR} 引用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_dir: Option<String>,
    /// encoder onnx 文件名（缺省用 int8 变体）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoder: Option<String>,
    /// decoder onnx 文件名（缺省用 fp32 变体，官方 int8 配方）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoder: Option<String>,
    /// joiner onnx 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joiner: Option<String>,
    /// tokens.txt 文件名（Qwen3-ASR 为 tokenizer 目录名，缺省 "tokenizer"）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<String>,
    /// 推理后端，缺省 "cpu"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// 推理线程数，缺省 2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_threads: Option<i32>,
    /// 每次喂给模型的采样数（@16k），缺省 3200（0.2s）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<usize>,
    /// 模型输入采样率，缺省 16000
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<i32>,
    /// 解码方式：greedy_search | modified_beam_search，缺省 greedy_search
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoding_method: Option<String>,
    /// 端点检测（静音自动断句），缺省 true
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_endpoint: Option<bool>,
    /// 规则 1 最小尾随静音（秒），缺省 2.4
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule1_min_trailing_silence: Option<f32>,
    /// 规则 2 最小尾随静音（秒），缺省 1.2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule2_min_trailing_silence: Option<f32>,
    /// 规则 3 最小句长（秒），缺省 20.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule3_min_utterance_length: Option<f32>,
    /// 空白符惩罚，缺省 0.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blank_penalty: Option<f32>,
    /// 热词（空格分隔，中文直接写），缺省无（zipformer 走 context graph、
    /// Qwen3-ASR 转逗号格式嵌提示词、paraformer 不支持）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotwords: Option<String>,
    /// 是否对最终结果自动加标点，缺省 true
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_punctuation: Option<bool>,
    /// 标点模型 onnx 路径（相对路径锚定标点模型目录）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub punctuation_model: Option<String>,
    /// 调试输出，缺省 false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<bool>,
    /// ASR 引擎后端：audiocpp（audio.cpp sidecar 进程，缺省）| sherpa（已移除，
    /// 仅老配置可解析，预检报「已移除」引导迁移）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// audiocpp 引擎二进制覆盖路径（开发/调试用；缺省由 locator 自动定位）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_path: Option<String>,
}

/// 文本转语音（TTS）配置。
///
/// 全部字段可缺省：未配置的项在解析时回退到 `tts::config` 的内置默认值，
/// 因此这里用 `Option` 以区分「未配置」与「配置了」。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TtsSettings {
    /// 是否启用语音合成，缺省 true
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// 模型类型（qwen3_tts_06/qwen3_tts_17；缺省 qwen3_tts_06，未知值回落默认——
    /// 兼容老版本 settings 里已移除的模型 kind）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_type: Option<crate::tts::config::TtsModelKind>,
    /// 模型目录（支持 ${env.VAR} 引用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_dir: Option<String>,
    /// encoder onnx 文件名（缺省 int8 变体）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoder: Option<String>,
    /// decoder onnx 文件名（缺省 int8 变体）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoder: Option<String>,
    /// 声码器 vocoder onnx 文件名（缺省 vocos_24khz.onnx）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vocoder: Option<String>,
    /// tokens.txt 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<String>,
    /// lexicon.txt 文件名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lexicon: Option<String>,
    /// espeak-ng 数据目录名（缺省 espeak-ng-data）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
    /// 参考音频 wav 路径（相对模型目录；缺省为旧模型包的 test_wavs/leijun-1.wav，
    /// managed 安装为单 GGUF、没有 test_wavs，该缺省不指向真实文件）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_wav: Option<String>,
    /// 参考音频的逐字转写文本
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_text: Option<String>,
    /// 默认音色 id（自定义音色库 id；缺省 None = 未设置，Qwen3-TTS Base 需显式选择克隆音色）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    /// 扩散解码步数（sherpa/ZipVoice 时代遗留字段）。
    ///
    /// 仅保留解析与回写（升级用户的老配置不丢、不被未知字段报错拦截）；
    /// audiocpp Qwen3-TTS 没有扩散解码，该值不再被任何引擎/前端读取。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_steps: Option<i32>,
    /// 语速，缺省 1.0
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    /// 推理后端，缺省 "cpu"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// 推理线程数，缺省 2
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_threads: Option<i32>,
    /// 调试输出，缺省 false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<bool>,
    /// TTS 引擎后端：audiocpp（audio.cpp sidecar 进程，缺省）；
    /// 残留 "sherpa" 的老配置由引擎预检明确报错引导迁移
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// audiocpp 引擎二进制覆盖路径（开发/调试用；缺省由 locator 自动定位）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_path: Option<String>,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            debug: false,
            log_level: default_log_level(),
            custom: None,
            microphone: None,
            data_dir: None,
            storage_prompt_acknowledged: false,
            asr: None,
            tts: None,
            model_library: None,
            shortcuts: None,
        }
    }
}

/// 加载 ~/.audiofn/settings.toml
///
/// 文件不存在时返回 None，不报错。
pub fn load_settings() -> Result<Option<AppConfig>, String> {
    let file_path = get_settings_path();

    let content = match std::fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Ok(None),
    };

    let config: AppConfig = toml::from_str(&content).map_err(|e| format!("TOML 格式错误: {e}"))?;

    Ok(Some(config))
}

/// 保存配置到 `~/.audiofn/settings.toml`（自动创建父目录）。
///
/// 采用「临时文件 + 替换」的安全写：先把完整内容写入带 pid 后缀的临时文件，
/// 再 rename 到正式路径。POSIX 上 rename 同文件系统是原子的（直接覆盖）；Windows
/// 上 rename 无法覆盖已存在目标，先移除旧文件再 rename（存在短暂窗口）。若替换失败
/// 会保留临时文件便于恢复，并返回明确错误——**不做严格 atomic replace 的承诺**。
pub fn save_settings(config: &AppConfig) -> Result<(), String> {
    let file_path = get_settings_path();
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let content = toml::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {e}"))?;
    let tmp = file_path.with_file_name(format!("settings.toml.tmp.{}", std::process::id()));
    std::fs::write(&tmp, &content).map_err(|e| format!("写入临时配置失败: {e}"))?;
    let renamed = match std::fs::rename(&tmp, &file_path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows：目标存在时 rename 可能失败，先移除再重试；失败则保留 tmp 便于恢复。
            if file_path.exists() {
                std::fs::remove_file(&file_path).map_err(|e| format!("移除旧配置失败: {e}"))?;
            }
            std::fs::rename(&tmp, &file_path).map_err(|e| format!("替换配置失败: {e}"))
        }
    };
    if renamed.is_ok() {
        // 应用内写入立即刷新 data_dir 缓存（mtime 同秒精度不足，不能只靠文件时间戳）
        refresh_data_dir_cache();
    }
    renamed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::run_with_temp_home;

    fn write_toml_settings(home: &std::path::Path, content: &str) {
        let settings_dir = home.join(PROJECT_DIR);
        std::fs::create_dir_all(&settings_dir).unwrap();
        std::fs::write(settings_dir.join(SETTINGS_FILE), content).unwrap();
    }

    #[test]
    fn test_get_settings_path() {
        run_with_temp_home(|home| {
            let path = get_settings_path();
            assert_eq!(path, home.join(".audiofn/settings.toml"));
        });
    }

    #[test]
    fn test_get_settings_dir() {
        run_with_temp_home(|home| {
            let dir = get_settings_dir();
            assert_eq!(dir, home.join(".audiofn"));
        });
    }

    #[test]
    fn test_resolve_env_ref_plain_value() {
        assert_eq!(resolve_env_ref("plain-value").unwrap(), "plain-value");
        assert_eq!(
            resolve_env_ref("https://example.com").unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn test_resolve_env_ref_from_env() {
        unsafe {
            std::env::set_var("TEST_MY_VAR", "test-resolved-value");
        }
        assert_eq!(
            resolve_env_ref("${env.TEST_MY_VAR}").unwrap(),
            "test-resolved-value"
        );
        unsafe {
            std::env::remove_var("TEST_MY_VAR");
        }
    }

    #[test]
    fn test_resolve_env_ref_missing_var() {
        let result = resolve_env_ref("${env.NONEXISTENT_VAR_XYZ}");
        assert!(result.is_err());
        assert!(result.err().unwrap().contains("NONEXISTENT_VAR_XYZ"));
    }

    #[test]
    fn test_resolve_env_ref_empty() {
        assert_eq!(resolve_env_ref("").unwrap(), "");
    }

    #[test]
    fn test_resolve_env_ref_empty_env_var_name() {
        let result = resolve_env_ref("${env.}");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_settings_file_not_found() {
        run_with_temp_home(|_| {
            let result = load_settings().unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn test_load_settings_invalid_toml() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "{invalid}");
            let result = load_settings();
            assert!(result.is_err());
            assert!(result.err().unwrap().contains("TOML 格式错误"));
        });
    }

    #[test]
    fn test_load_settings_empty() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "");
            let result = load_settings().unwrap().unwrap();
            assert!(!result.debug);
            assert_eq!(result.log_level, "info");
            assert!(result.custom.is_none());
        });
    }

    #[test]
    fn test_load_settings_full() {
        run_with_temp_home(|home| {
            write_toml_settings(
                home,
                "debug = true\nlog_level = \"debug\"\n\n[custom]\nkey1 = \"value1\"\n",
            );
            let result = load_settings().unwrap().unwrap();
            assert!(result.debug);
            assert_eq!(result.log_level, "debug");
            assert_eq!(result.custom.unwrap().get("key1").unwrap(), "value1");
        });
    }

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert!(!config.debug);
        assert_eq!(config.log_level, "info");
        assert!(config.custom.is_none());
        assert!(config.microphone.is_none());
    }

    #[test]
    fn test_app_config_serde_roundtrip() {
        let config = AppConfig {
            debug: true,
            log_level: "warn".to_string(),
            custom: Some(std::collections::HashMap::new()),
            microphone: Some("内置麦克风".to_string()),
            data_dir: None,
            storage_prompt_acknowledged: false,
            asr: None,
            tts: None,
            model_library: None,
            shortcuts: None,
        };
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, deserialized);
        // microphone 应被序列化
        assert!(toml_str.contains("microphone"));
    }

    #[test]
    fn test_load_settings_ignores_removed_fields() {
        // 旧版本 settings.toml 里已删除的字段（hide_dock_icon / [chatbox]）应被
        // serde 忽略：不报错、也不写回，保证老配置平滑升级。
        run_with_temp_home(|home| {
            write_toml_settings(
                home,
                "debug = true\nhide_dock_icon = true\n\n[chatbox]\nvisible = false\n",
            );
            let result = load_settings().unwrap().unwrap();
            assert!(result.debug);
            let toml_str = toml::to_string(&result).unwrap();
            assert!(!toml_str.contains("hide_dock_icon"));
            assert!(!toml_str.contains("chatbox"));
        });
    }

    #[test]
    fn test_load_settings_without_storage_prompt_ack_defaults_false() {
        // 旧配置文件没有 storage_prompt_acknowledged 时，应回退为 false（引导仍会弹）。
        run_with_temp_home(|home| {
            write_toml_settings(home, "debug = true\n");
            let result = load_settings().unwrap().unwrap();
            assert!(!result.storage_prompt_acknowledged);
        });
    }

    #[test]
    fn test_load_settings_with_storage_prompt_ack() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "storage_prompt_acknowledged = true\n");
            let result = load_settings().unwrap().unwrap();
            assert!(result.storage_prompt_acknowledged);
        });
    }

    #[test]
    fn test_load_settings_with_microphone() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "microphone = \"内置麦克风\"\n");
            let result = load_settings().unwrap().unwrap();
            assert_eq!(result.microphone.as_deref(), Some("内置麦克风"));
        });
    }

    #[test]
    fn test_load_settings_with_asr_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "[asr]\nnum_threads = 4\nenable_endpoint = false\n");
            let result = load_settings().unwrap().unwrap();
            let asr = result.asr.unwrap();
            assert_eq!(asr.num_threads, Some(4));
            assert_eq!(asr.enable_endpoint, Some(false));
            // 未配置的字段保持 None
            assert_eq!(asr.model_dir, None);
            assert_eq!(asr.decoding_method, None);
        });
    }

    #[test]
    fn test_load_settings_without_asr_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "debug = true\n");
            let result = load_settings().unwrap().unwrap();
            assert!(result.asr.is_none());
        });
    }

    #[test]
    fn test_load_settings_with_tts_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "[tts]\nnum_threads = 4\nspeed = 1.2\n");
            let result = load_settings().unwrap().unwrap();
            let tts = result.tts.unwrap();
            assert_eq!(tts.num_threads, Some(4));
            assert_eq!(tts.speed, Some(1.2));
            // 未配置的字段保持 None
            assert_eq!(tts.model_dir, None);
            assert_eq!(tts.num_steps, None);
        });
    }

    #[test]
    fn test_load_settings_without_tts_table() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "debug = true\n");
            let result = load_settings().unwrap().unwrap();
            assert!(result.tts.is_none());
        });
    }

    #[test]
    fn test_load_settings_with_tts_enabled_false() {
        run_with_temp_home(|home| {
            write_toml_settings(home, "[tts]\nenabled = false\n");
            let result = load_settings().unwrap().unwrap();
            let tts = result.tts.unwrap();
            assert_eq!(tts.enabled, Some(false));
        });
    }

    #[test]
    fn test_tts_settings_serde_roundtrip() {
        let tts = TtsSettings {
            enabled: Some(false),
            model_type: Some(crate::tts::config::TtsModelKind::Qwen3Tts06),
            model_dir: Some("${env.TTS_MODEL_DIR}".to_string()),
            encoder: Some("encoder.int8.onnx".to_string()),
            decoder: None,
            vocoder: Some("vocos_24khz.onnx".to_string()),
            tokens: None,
            lexicon: None,
            data_dir: None,
            reference_wav: Some("test_wavs/leijun-1.wav".to_string()),
            reference_text: None,
            voice: Some("custom-voice".to_string()),
            num_steps: Some(4),
            speed: Some(1.0),
            provider: Some("cpu".to_string()),
            num_threads: Some(2),
            debug: Some(false),
            backend: Some("audiocpp".to_string()),
            engine_path: None,
        };
        let toml_str = toml::to_string(&tts).unwrap();
        let deserialized: TtsSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(tts, deserialized);
        // 未配置字段应被 skip_serializing_if 忽略
        assert!(!toml_str.contains("decoder"));
        assert!(!toml_str.contains("engine_path"));
        assert!(toml_str.contains("backend = \"audiocpp\""));
    }

    #[test]
    fn test_get_tts_output_dir() {
        run_with_temp_home(|home| {
            assert_eq!(get_tts_output_dir(), home.join(".audiofn/tts"));
        });
    }

    #[test]
    fn test_save_settings_roundtrip() {
        run_with_temp_home(|home| {
            let config = AppConfig {
                debug: true,
                log_level: "debug".to_string(),
                custom: None,
                microphone: None,
                data_dir: None,
                storage_prompt_acknowledged: false,
                asr: None,
                tts: None,
                model_library: None,
                shortcuts: None,
            };
            save_settings(&config).unwrap();
            let loaded = load_settings().unwrap().unwrap();
            assert_eq!(loaded, config);
            // 文件确实写到了 HOME 下
            assert!(home.join(".audiofn/settings.toml").is_file());
        });
    }

    // ---- data_dir（自定义数据目录）----

    /// 用 AppConfig + save_settings 写 data_dir（TOML 序列化器正确转义 Windows 反斜杠）。
    fn write_data_dir_settings(data_dir: Option<&str>) {
        let config = AppConfig {
            data_dir: data_dir.map(|s| s.to_string()),
            ..AppConfig::default()
        };
        save_settings(&config).unwrap();
    }

    #[test]
    fn test_data_dir_serde_roundtrip() {
        run_with_temp_home(|home| {
            write_data_dir_settings(Some("D:\\zapdata"));
            let loaded = load_settings().unwrap().unwrap();
            assert_eq!(loaded.data_dir.as_deref(), Some("D:\\zapdata"));
            // 未设置时字段不序列化
            let toml_str = toml::to_string(&AppConfig::default()).unwrap();
            assert!(!toml_str.contains("data_dir"));
            assert!(home.join(".audiofn/settings.toml").is_file());
        });
    }

    #[test]
    fn test_get_models_dir_default_unchanged() {
        run_with_temp_home(|home| {
            assert_eq!(get_models_dir(), home.join(".audiofn/models"));
            assert_eq!(legacy_models_dir(), None);
        });
    }

    #[test]
    fn test_get_models_dir_custom_data_dir() {
        run_with_temp_home(|home| {
            let data = home.join("zapdata");
            write_data_dir_settings(Some(&data.display().to_string()));
            assert_eq!(get_data_dir(), Some(data.clone()));
            assert_eq!(get_models_dir(), data.join("models"));
            // 旧根指向默认位置（供双根扫描/迁移定位存量）
            assert_eq!(legacy_models_dir(), Some(home.join(".audiofn/models")));
        });
    }

    #[test]
    fn test_get_data_dir_env_ref_resolution() {
        run_with_temp_home(|home| {
            let env_dir = home.join("envdata");
            unsafe {
                std::env::set_var("TEST_ZM_DATA_DIR", &env_dir);
            }
            write_data_dir_settings(Some("${env.TEST_ZM_DATA_DIR}"));
            assert_eq!(get_data_dir(), Some(env_dir.clone()));
            assert_eq!(get_models_dir(), env_dir.join("models"));
            unsafe {
                std::env::remove_var("TEST_ZM_DATA_DIR");
            }
        });
    }

    #[test]
    fn test_get_data_dir_invalid_env_falls_back() {
        run_with_temp_home(|home| {
            write_data_dir_settings(Some("${env.NONEXISTENT_DATA_DIR_XYZ}"));
            assert_eq!(get_data_dir(), None);
            assert_eq!(get_models_dir(), home.join(".audiofn/models"));
            assert_eq!(legacy_models_dir(), None);
        });
    }

    #[test]
    fn test_strip_prefix_ci() {
        if cfg!(windows) {
            // Windows：大小写不敏感 + 分隔符宽容，剥离前缀后返回相对路径
            let prefix = std::path::Path::new("C:\\Users\\Admin\\zapdata\\models");
            let path = std::path::Path::new("c:\\users\\admin\\zapdata\\models\\llm\\model.gguf");
            let rest = strip_prefix_ci(path, prefix).unwrap();
            assert_eq!(rest, std::path::Path::new("llm\\model.gguf"));
            // 不在前缀下 → None
            let other = std::path::Path::new("D:\\other\\x");
            assert!(strip_prefix_ci(other, prefix).is_none());
            // 前缀自身 → 返回空
            let exact = std::path::Path::new("C:\\Users\\Admin\\zapdata\\models");
            assert_eq!(
                strip_prefix_ci(exact, prefix).unwrap(),
                std::path::Path::new("")
            );
            // 部分段重合（models2）不算前缀，防迁移误改写
            let sibling = std::path::Path::new("C:\\Users\\Admin\\zapdata\\models2\\x.gguf");
            assert!(strip_prefix_ci(sibling, prefix).is_none());
        } else {
            // Unix：大小写敏感、仅 `/` 为分隔符
            let prefix = std::path::Path::new("/home/user/zapdata/models");
            let path = std::path::Path::new("/home/user/zapdata/models/llm/model.gguf");
            assert_eq!(
                strip_prefix_ci(path, prefix).unwrap(),
                std::path::Path::new("llm/model.gguf")
            );
            // 大小写不同 → None
            let mixed = std::path::Path::new("/Home/User/zapdata/models/m.gguf");
            assert!(strip_prefix_ci(mixed, prefix).is_none());
            // 不在前缀下 → None
            let other = std::path::Path::new("/opt/other/x");
            assert!(strip_prefix_ci(other, prefix).is_none());
            // 前缀自身 → 返回空
            let exact = std::path::Path::new("/home/user/zapdata/models");
            assert_eq!(
                strip_prefix_ci(exact, prefix).unwrap(),
                std::path::Path::new("")
            );
            // 部分段重合（models2）不算前缀，防迁移误改写
            let sibling = std::path::Path::new("/home/user/zapdata/models2/x.gguf");
            assert!(strip_prefix_ci(sibling, prefix).is_none());
        }
    }

    #[test]
    fn test_get_data_dir_relative_falls_back() {
        run_with_temp_home(|_| {
            write_data_dir_settings(Some("relative/dir"));
            assert_eq!(get_data_dir(), None);
        });
    }

    #[test]
    fn test_get_data_dir_strips_verbatim_prefix() {
        run_with_temp_home(|home| {
            // 模拟旧版落盘的 canonicalize 产物：读取时须转回普通盘符形式
            write_data_dir_settings(Some(r"\\?\D:\zapdata"));
            let expected = if cfg!(windows) {
                Some(PathBuf::from(r"D:\zapdata"))
            } else {
                // 非 Windows 无 verbatim 概念，且该字符串不是合法绝对路径 → 回退默认
                None
            };
            assert_eq!(get_data_dir(), expected);
            assert_eq!(
                get_models_dir(),
                expected
                    .map(|d| d.join("models"))
                    .unwrap_or_else(|| home.join(".audiofn/models"))
            );
        });
    }

    #[test]
    fn test_strip_verbatim_prefix() {
        if cfg!(windows) {
            assert_eq!(
                strip_verbatim_prefix(PathBuf::from(r"\\?\D:\zapdata\models")),
                PathBuf::from(r"D:\zapdata\models")
            );
            // UNC verbatim → 普通 UNC
            assert_eq!(
                strip_verbatim_prefix(PathBuf::from(r"\\?\UNC\server\share\x")),
                PathBuf::from(r"\\server\share\x")
            );
            // 普通 / 相对路径原样返回
            assert_eq!(
                strip_verbatim_prefix(PathBuf::from(r"D:\zapdata")),
                PathBuf::from(r"D:\zapdata")
            );
            assert_eq!(
                strip_verbatim_prefix(PathBuf::from("relative/dir")),
                PathBuf::from("relative/dir")
            );
        } else {
            // 非 Windows 恒为 no-op
            assert_eq!(
                strip_verbatim_prefix(PathBuf::from(r"\\?\D:\zapdata")),
                PathBuf::from(r"\\?\D:\zapdata")
            );
        }
    }

    #[test]
    fn test_data_dir_cache_mtime_invalidation() {
        run_with_temp_home(|home| {
            let d1 = home.join("d1");
            let d2 = home.join("d2");
            write_data_dir_settings(Some(&d1.display().to_string()));
            assert_eq!(get_data_dir(), Some(d1.clone()));
            // 直接改文件（不经 refresh_data_dir_cache）：mtime 变化应自动失效缓存
            write_data_dir_settings(Some(&d2.display().to_string()));
            assert_eq!(get_data_dir(), Some(d2.clone()));
            // 显式刷新后同样正确
            write_data_dir_settings(Some(&d1.display().to_string()));
            refresh_data_dir_cache();
            assert_eq!(get_data_dir(), Some(d1));
        });
    }

    #[test]
    fn test_save_settings_safe_replace_and_tmp_cleanup() {
        run_with_temp_home(|home| {
            let config = AppConfig {
                log_level: "debug".to_string(),
                ..Default::default()
            };
            save_settings(&config).unwrap();
            // 正式文件存在
            assert!(home.join(".audiofn/settings.toml").is_file());
            // 临时文件被清理（rename 成功）
            let tmp = home.join(format!(".audiofn/settings.toml.tmp.{}", std::process::id()));
            assert!(!tmp.exists());
            // 覆盖保存仍成功且内容完整
            let config2 = AppConfig {
                log_level: "warn".to_string(),
                ..Default::default()
            };
            save_settings(&config2).unwrap();
            let loaded = load_settings().unwrap().unwrap();
            assert_eq!(loaded.log_level, "warn");
        });
    }
}
