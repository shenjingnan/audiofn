use crate::config::settings::{TtsSettings, resolve_env_ref};
/// TTS 配置解析与校验。
///
/// 负责把 `settings.toml` 的 `[tts]` 表与 CLI flag 合并成一份已展开、已填默认值的
/// `ResolvedTtsConfig`。优先级：CLI `--model-dir` > settings > 内置默认。
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 旧 sherpa 模型包内默认文件名（zipvoice distill int8 打包版）。
///
/// 仅作老配置字段（`[tts].encoder` 等）与默认模型目录探测的兜底值：audiocpp
/// 引擎不消费这些 onnx/词表文件，模型文件校验一律走族表（`audiocpp::families`）。
pub const DEFAULT_ENCODER: &str = "encoder.int8.onnx";
pub const DEFAULT_DECODER: &str = "decoder.int8.onnx";
/// 声码器（独立发布的单文件，已随 zipvoice 收录移除下载路径）。
pub const DEFAULT_VOCODER: &str = "vocos_24khz.onnx";
pub const DEFAULT_TOKENS: &str = "tokens.txt";
pub const DEFAULT_LEXICON: &str = "lexicon.txt";
/// espeak-ng 数据目录（相对模型目录）。
pub const DEFAULT_DATA_DIR: &str = "espeak-ng-data";
/// 默认参考音频（声音克隆的音色来源，相对模型目录）。
pub const DEFAULT_REFERENCE_WAV: &str = "test_wavs/leijun-1.wav";
/// 默认参考音频的逐字转写（来自模型包内 test_wavs/prompt.txt）。
pub const DEFAULT_REFERENCE_TEXT: &str = "那还是36年前, 1987年. 我呢考上了武汉大学的计算机系.";

/// TTS 模型类型（audio.cpp 后端收录的 Qwen3-TTS 尺寸）。
///
/// 全链路显式传递：`[tts].model_type`（持久化）→ `ResolvedTtsConfig.model_type` →
/// audiocpp 族表（`crate::audiocpp::families`）。默认 0.6B（延迟优先，1.7B 质量优先）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TtsModelKind {
    /// Qwen3-TTS 0.6B Base：10 语种音色克隆，24kHz。
    /// Base 版必须提供克隆参考音频（上游无 auto voice）。
    /// 显式 serde rename：派生 snake_case 会得到 `qwen3_tts06`，与 as_str/parse_str 不一致
    #[serde(rename = "qwen3_tts_06")]
    #[default]
    Qwen3Tts06,
    /// Qwen3-TTS 1.7B Base：质量优先变体。
    #[serde(rename = "qwen3_tts_17")]
    Qwen3Tts17,
}

impl TtsModelKind {
    /// snake_case 字符串（配置/JSON 直传）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Qwen3Tts06 => "qwen3_tts_06",
            Self::Qwen3Tts17 => "qwen3_tts_17",
        }
    }

    /// 解析 snake_case 字符串（与 `ModelType::from_str_value` 同款命名，避免与
    /// `std::str::FromStr` 混淆）。
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "qwen3_tts_06" => Some(Self::Qwen3Tts06),
            "qwen3_tts_17" => Some(Self::Qwen3Tts17),
            _ => None,
        }
    }
}

/// 手写 Deserialize：未知 kind 回落默认 Qwen3-TTS 0.6B。
///
/// 历史版本曾收录 zipvoice/omnivoice/voxcpm2/kitten/supertonic 等模型并持久化进
/// `settings.toml`，收录移除后老配置里的这些值若走派生反序列化会让**整份** settings
/// 加载失败。回落默认值让升级用户平滑迁移到 Qwen3-TTS（模型目录不匹配时预检会给出
/// install-model 提示，引导重新选择受支持的模型）。
impl<'de> Deserialize<'de> for TtsModelKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::parse_str(&s).unwrap_or_default())
    }
}

/// TTS 推理后端。
///
/// 一期裁剪后引擎只保留 audio.cpp sidecar 一条路径；`Sherpa` 变体仅为老配置
/// （`[tts].backend = "sherpa"`）保留解析入口，构造引擎时明确报错引导迁移。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TtsBackendKind {
    /// sherpa-onnx 进程内 `OfflineTts`（一期裁剪后已移除，仅老配置可达）
    Sherpa,
    /// audio.cpp sidecar 进程（audiocpp_server，OpenAI 风格 HTTP；缺省）
    #[default]
    Audiocpp,
}

impl TtsBackendKind {
    /// snake_case 字符串（配置/JSON 直传）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sherpa => "sherpa",
            Self::Audiocpp => "audiocpp",
        }
    }

    /// 解析 snake_case 字符串（与 `TtsModelKind::parse_str` 同款命名）。
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "sherpa" => Some(Self::Sherpa),
            "audiocpp" => Some(Self::Audiocpp),
            _ => None,
        }
    }
}

/// 解析后的完整 TTS 配置。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTtsConfig {
    /// 是否启用语音合成
    pub enabled: bool,
    /// 模型类型（决定 audiocpp 族表条目；默认 Qwen3-TTS 0.6B）
    pub model_type: TtsModelKind,
    pub model_dir: PathBuf,
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub vocoder: PathBuf,
    pub tokens: PathBuf,
    pub lexicon: PathBuf,
    pub data_dir: PathBuf,
    pub reference_wav: PathBuf,
    pub reference_text: String,
    /// 默认音色 id（模型包内置参考音色 / 自定义音色库 id）。
    pub voice: Option<String>,
    /// 扩散解码步数（质量/速度权衡）
    pub num_steps: i32,
    /// 语速
    pub speed: f32,
    pub provider: String,
    pub num_threads: i32,
    pub debug: bool,
    /// 推理后端（缺省 Audiocpp；`Sherpa` 为老配置入口，构造引擎时明确报错）
    pub backend: TtsBackendKind,
    /// audiocpp 引擎二进制覆盖路径（开发/调试用；None = locator 自动定位）
    pub engine_path: Option<PathBuf>,
}

impl Default for ResolvedTtsConfig {
    fn default() -> Self {
        let model_dir = default_model_dir();
        let join = |name: &str| model_dir.join(name);
        Self {
            enabled: true,
            model_type: TtsModelKind::Qwen3Tts06,
            encoder: join(DEFAULT_ENCODER),
            decoder: join(DEFAULT_DECODER),
            vocoder: join(DEFAULT_VOCODER),
            tokens: join(DEFAULT_TOKENS),
            lexicon: join(DEFAULT_LEXICON),
            data_dir: join(DEFAULT_DATA_DIR),
            reference_wav: join(DEFAULT_REFERENCE_WAV),
            model_dir,
            reference_text: DEFAULT_REFERENCE_TEXT.to_string(),
            voice: None,
            num_steps: 4,
            speed: 1.0,
            provider: "cpu".to_string(),
            num_threads: 2,
            debug: false,
            backend: TtsBackendKind::Audiocpp,
            engine_path: None,
        }
    }
}

/// TTS 就绪预检（单一权威入口）。
///
/// 按模型族描述表（`crate::audiocpp::families`）的 `required_files` 逐文件校验；
/// 老配置的 `backend = "sherpa"` 在此明确报错引导迁移（引擎已无进程内路径）。
pub fn preflight(cfg: &ResolvedTtsConfig) -> Result<(), String> {
    if cfg.backend == TtsBackendKind::Sherpa {
        return Err(
            "TTS 后端 sherpa 已移除：请改用 audiocpp 后端（在模型库选择 Qwen3-TTS 模型）。"
                .to_string(),
        );
    }
    let desc = crate::audiocpp::families::family_desc(cfg.model_type).ok_or_else(|| {
        format!(
            "模型类型 {} 不支持 audiocpp 后端（请检查 [tts].model_type 与 backend 组合）",
            cfg.model_type.as_str()
        )
    })?;
    for name in desc.required_files {
        let p = cfg.model_dir.join(name);
        if !p.is_file() {
            return Err(format!(
                "缺少模型文件 {name}: {}\n请运行 `{}` 下载模型。",
                p.display(),
                desc.registry_hint
            ));
        }
    }
    Ok(())
}

/// 模型是否就绪（[`preflight`] 的布尔版，GUI `models_present` 徽标用）。
///
/// 引擎二进制定位失败不在此拦截（合成时报错更精确）。
pub fn models_present(cfg: &ResolvedTtsConfig) -> bool {
    preflight(cfg).is_ok()
}

/// `tts install-model` 缺省安装的模型库条目（audiocpp Qwen3-TTS 0.6B，单 GGUF）。
///
/// CLI（`src/cli.rs`）与 Tauri（`download_tts_model`）共用；目录基名取同一条目的
/// registry `model.name`（见 [`default_registry_model_dir_name`]，与
/// `install_managed_model` 的落位规则同源）。
pub const DEFAULT_TTS_REGISTRY_ID: &str = "tts-qwen3-06b-base-q8-audiocpp";

/// 缺省模型条目的安装目录基名（registry `model.name`）。
///
/// 与 `install_managed_model` 的落位（`models/<model.name>`）取同一事实源，
/// 保证「先 install-model 再 tts run」的缺省路径能对上同一目录。
/// （缺陷修复：此前缺省目录取已裁剪的 manifest role `"tts"` 资产名
/// `sherpa-onnx-zipvoice-distill-int8-…`，而 audiocpp 模型装到
/// `qwen3-tts-06b-base-audiocpp`——fresh HOME 下装完模型依旧报缺文件死循环。）
fn default_registry_model_dir_name() -> String {
    crate::model_library::registry::model_by_id(DEFAULT_TTS_REGISTRY_ID)
        .unwrap_or_else(|| panic!("模型库缺少 audiocpp Qwen3-TTS 条目: {DEFAULT_TTS_REGISTRY_ID}"))
        .name
        .clone()
}

/// 用户默认模型目录：`~/.zapmomo/models/<模型名>`
pub fn user_default_model_dir() -> PathBuf {
    crate::config::settings::get_models_dir().join(default_registry_model_dir_name())
}

/// 源码仓库中的模型目录（开发者 `./models/<模型名>`，仅作开发回退）。
fn repo_models_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join(default_registry_model_dir_name())
}

/// 默认模型目录选择：用户已安装 > 旧默认根存量（data_dir 切换后）> 源码仓库已下载（开发便利）> 用户默认。
///
/// 探测文件是 audiocpp 族的主 GGUF（单文件即完整）；纯决策函数（不访问真实文件
/// 系统），便于测试注入路径。
fn choose_default_model_dir(user: &Path, legacy: Option<&Path>, repo: &Path) -> PathBuf {
    let probe = crate::audiocpp::families::QWEN3_TTS_06B.gguf_file;
    if user.join(probe).is_file() {
        user.to_path_buf()
    } else if legacy.is_some_and(|l| l.join(probe).is_file()) {
        legacy.unwrap().to_path_buf()
    } else if repo.join(probe).is_file() {
        repo.to_path_buf()
    } else {
        user.to_path_buf()
    }
}

/// 默认模型目录（运行时解析：优先用户目录，旧根存量兜底，源码开发时回退到仓库 `./models/`）。
pub fn default_model_dir() -> PathBuf {
    // legacy 与 user 层次对等：旧根下对应模型的子目录（user 是 `models/<模型名>`）
    let legacy = crate::config::settings::legacy_models_dir()
        .map(|l| l.join(default_registry_model_dir_name()));
    choose_default_model_dir(
        &user_default_model_dir(),
        legacy.as_deref(),
        &repo_models_dir(),
    )
}

/// 展开 settings 中的路径字符串（支持 `${env.VAR}`），未配置时用默认文件名。
/// 返回的路径若为相对路径则拼接在 `model_dir` 下。
fn resolve_file(
    settings_value: Option<&str>,
    default_name: &str,
    model_dir: &Path,
) -> Result<PathBuf, String> {
    match settings_value {
        Some(v) => {
            let expanded = resolve_env_ref(v)?;
            let p = PathBuf::from(&expanded);
            Ok(if p.is_absolute() {
                p
            } else {
                model_dir.join(p)
            })
        }
        None => Ok(model_dir.join(default_name)),
    }
}

/// 解析模型目录：CLI > settings > 默认。
fn resolve_model_dir(
    settings: Option<&TtsSettings>,
    cli_model_dir: Option<&Path>,
) -> Result<PathBuf, String> {
    if let Some(dir) = cli_model_dir {
        return Ok(dir.to_path_buf());
    }
    if let Some(dir) = settings.and_then(|s| s.model_dir.as_deref()) {
        let expanded = resolve_env_ref(dir)?;
        let p = PathBuf::from(expanded);
        return Ok(if p.is_absolute() {
            p
        } else {
            crate::config::settings::get_settings_dir().join(p)
        });
    }
    Ok(default_model_dir())
}

/// 合并配置并填充默认值。
pub fn resolve(
    settings: Option<&TtsSettings>,
    cli_model_dir: Option<&Path>,
) -> Result<ResolvedTtsConfig, String> {
    let mut cfg = ResolvedTtsConfig {
        model_dir: resolve_model_dir(settings, cli_model_dir)?,
        ..ResolvedTtsConfig::default()
    };

    let s = settings;
    cfg.enabled = s.and_then(|s| s.enabled).unwrap_or(true);
    // 模型类型：settings 显式 > 默认 Qwen3-TTS 0.6B（managed 目录名 → kind 的权威
    // 写入在 `set_selected_model`；无字段或残留已移除 kind 的老配置 → 默认 0.6B）
    cfg.model_type = s.and_then(|s| s.model_type).unwrap_or_default();

    let file = |field: &str, default_name: &str| {
        let value = match field {
            "encoder" => s.and_then(|s| s.encoder.as_deref()),
            "decoder" => s.and_then(|s| s.decoder.as_deref()),
            "vocoder" => s.and_then(|s| s.vocoder.as_deref()),
            "tokens" => s.and_then(|s| s.tokens.as_deref()),
            "lexicon" => s.and_then(|s| s.lexicon.as_deref()),
            "data_dir" => s.and_then(|s| s.data_dir.as_deref()),
            "reference_wav" => s.and_then(|s| s.reference_wav.as_deref()),
            _ => None,
        };
        resolve_file(value, default_name, &cfg.model_dir)
    };

    cfg.encoder = file("encoder", DEFAULT_ENCODER)?;
    cfg.decoder = file("decoder", DEFAULT_DECODER)?;
    cfg.vocoder = file("vocoder", DEFAULT_VOCODER)?;
    cfg.tokens = file("tokens", DEFAULT_TOKENS)?;
    cfg.lexicon = file("lexicon", DEFAULT_LEXICON)?;
    cfg.data_dir = file("data_dir", DEFAULT_DATA_DIR)?;
    cfg.reference_wav = file("reference_wav", DEFAULT_REFERENCE_WAV)?;

    // audiocpp 族不消费 sherpa 文件字段：GGUF 定位由 `AudiocppTts` 内部经
    // families 表完成（model_dir + gguf_file）。

    cfg.reference_text = s
        .and_then(|s| s.reference_text.clone())
        .unwrap_or_else(|| DEFAULT_REFERENCE_TEXT.to_string());
    cfg.voice = s.and_then(|s| s.voice.clone());
    cfg.num_steps = s.and_then(|s| s.num_steps).unwrap_or(4);
    cfg.speed = s.and_then(|s| s.speed).unwrap_or(1.0);
    cfg.num_threads = s.and_then(|s| s.num_threads).unwrap_or(2);
    cfg.debug = s.and_then(|s| s.debug).unwrap_or(false);
    // 推理后端：缺省 audiocpp（唯一在册引擎），非法值显式报错
    cfg.backend = match s.and_then(|s| s.backend.as_deref()) {
        Some(v) => TtsBackendKind::parse_str(v)
            .ok_or_else(|| format!("未知 TTS 后端: {v}（支持 audiocpp）"))?,
        None => TtsBackendKind::default(),
    };
    // 推理设备：用户显式配置优先；缺省时 audiocpp 后端按平台取默认
    // （见 `audiocpp::provider`：macOS Metal / Windows CUDA / 其余 CPU，
    // 无可用 GPU 由 server 层自动回退 CPU）。
    cfg.provider = match s.and_then(|s| s.provider.clone()) {
        Some(p) => p,
        None => {
            if cfg.backend == TtsBackendKind::Audiocpp
                && crate::audiocpp::families::family_desc(cfg.model_type).is_some()
            {
                crate::audiocpp::provider::current_default_provider().to_string()
            } else {
                "cpu".to_string()
            }
        }
    };
    cfg.engine_path = s
        .and_then(|s| s.engine_path.as_deref())
        .map(resolve_env_ref)
        .transpose()?
        .map(PathBuf::from);

    Ok(cfg)
}

/// `set_tts_params` 载荷：可调整的 TTS 合成参数（snake_case 直传，缺省项不修改）。
///
/// 与 `AsrParamsPatch` 对称，放在 lib crate 内以便 `cargo test` 单测。
/// 引擎在每次合成时新建（`synthesize_tts` → `TtsEngine::new`），因此保存后**下一次合成即生效**，无需重启。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TtsParamsPatch {
    /// 扩散解码步数（质量/速度权衡）
    pub num_steps: Option<i32>,
    /// 默认语速（单次合成可经 `synthesize_tts.speed` 覆盖）
    pub speed: Option<f32>,
    /// 推理线程数
    pub num_threads: Option<i32>,
    /// 调试输出
    pub debug: Option<bool>,
}

impl TtsParamsPatch {
    /// 先整体校验（任一越界立即 Err），再逐项写入 `TtsSettings`，保证出错时不部分修改。
    pub fn apply_to(&self, tts: &mut TtsSettings) -> Result<(), String> {
        if let Some(v) = self.num_steps
            && !(1..=32).contains(&v)
        {
            return Err(format!("扩散步数需在 1~32，当前 {v}"));
        }
        if let Some(v) = self.speed
            && !(0.5..=2.0).contains(&v)
        {
            return Err(format!("语速需在 0.5~2.0，当前 {v}"));
        }
        if let Some(v) = self.num_threads
            && !(1..=32).contains(&v)
        {
            return Err(format!("线程数需在 1~32，当前 {v}"));
        }

        if let Some(v) = self.num_steps {
            tts.num_steps = Some(v);
        }
        if let Some(v) = self.speed {
            tts.speed = Some(v);
        }
        if let Some(v) = self.num_threads {
            tts.num_threads = Some(v);
        }
        if let Some(v) = self.debug {
            tts.debug = Some(v);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::TtsSettings;
    use crate::test_util::run_with_temp_home;

    #[test]
    fn test_default_model_dir_dual_root_fallback() {
        run_with_temp_home(|home| {
            crate::test_util::set_custom_data_dir(home);
            let new_dir = user_default_model_dir();
            let legacy_dir = home
                .join(".zapmomo")
                .join("models")
                .join(new_dir.file_name().unwrap());
            let gguf = crate::audiocpp::families::QWEN3_TTS_06B.gguf_file;

            for d in [&new_dir, &legacy_dir] {
                std::fs::create_dir_all(d).unwrap();
                std::fs::write(d.join(gguf), b"t").unwrap();
            }
            assert_eq!(default_model_dir(), new_dir);

            std::fs::remove_dir_all(&new_dir).unwrap();
            assert_eq!(default_model_dir(), legacy_dir);

            std::fs::remove_dir_all(&legacy_dir).unwrap();
            assert_ne!(default_model_dir(), legacy_dir);
        });
    }

    /// 缺省目录基名 == 缺省 registry 条目的 managed 安装目录名（同一事实源）。
    ///
    /// 回归锚点：此前缺省目录解析已裁剪的 manifest role `"tts"`（zipvoice 目录名），
    /// 与 `install_managed_model` 的落位目录不一致 → fresh HOME 下 `tts install-model`
    /// 装完模型后 `tts run` 依旧 preflight 报缺文件死循环。
    #[test]
    fn test_default_model_dir_matches_registry_install_dir() {
        run_with_temp_home(|_| {
            let model = crate::model_library::registry::model_by_id(DEFAULT_TTS_REGISTRY_ID)
                .expect("模型库应含缺省 Qwen3-TTS 条目");
            // 落位目录（install_managed_model 的 commit 目标）
            let install_dir = crate::model_library::managed_install_dir(model);
            assert_eq!(user_default_model_dir(), install_dir);
            assert_eq!(
                default_model_dir().file_name(),
                install_dir.file_name(),
                "缺省目录基名必须等于 registry 条目目录名"
            );
            assert_eq!(default_model_dir(), install_dir);

            // fresh HOME：模拟「install-model 装完」→ 缺省解析命中该目录且 preflight 通过
            std::fs::create_dir_all(&install_dir).unwrap();
            std::fs::write(
                install_dir.join(crate::audiocpp::families::QWEN3_TTS_06B.gguf_file),
                b"x",
            )
            .unwrap();
            assert_eq!(default_model_dir(), install_dir);
            let cfg = resolve(None, None).unwrap();
            assert_eq!(cfg.model_dir, install_dir);
            crate::tts::config::preflight(&cfg)
                .expect("缺省条目安装完成后 tts run preflight 应通过");

            // 未安装时不 panic：解析回落用户目录，preflight 报缺文件 + install-model 提示
            std::fs::remove_dir_all(&install_dir).unwrap();
            let cfg = resolve(None, None).unwrap();
            let err = preflight(&cfg).unwrap_err();
            assert!(
                err.contains(crate::audiocpp::families::QWEN3_TTS_06B.gguf_file),
                "err: {err}"
            );
            assert!(err.contains("tts install-model"), "err: {err}");
            assert!(err.contains(DEFAULT_TTS_REGISTRY_ID), "err: {err}");
        });
    }

    #[test]
    fn test_default_config_points_to_default_model_dir() {
        let cfg = ResolvedTtsConfig::default();
        assert_eq!(
            cfg.model_dir
                .file_name()
                .map(|s| s.to_string_lossy().to_string()),
            Some("qwen3-tts-06b-base-audiocpp".to_string()),
            "缺省目录 = 缺省 registry 条目的安装目录"
        );
        // 缺省即 Qwen3-TTS 0.6B + audiocpp 后端（唯一在册引擎）
        assert_eq!(cfg.model_type, TtsModelKind::Qwen3Tts06);
        assert_eq!(cfg.backend, TtsBackendKind::Audiocpp);
        assert_eq!(cfg.encoder.file_name().unwrap(), DEFAULT_ENCODER);
        assert_eq!(cfg.decoder.file_name().unwrap(), DEFAULT_DECODER);
        assert_eq!(cfg.vocoder.file_name().unwrap(), DEFAULT_VOCODER);
        assert_eq!(cfg.tokens.file_name().unwrap(), DEFAULT_TOKENS);
        assert_eq!(cfg.lexicon.file_name().unwrap(), DEFAULT_LEXICON);
        assert_eq!(cfg.data_dir.file_name().unwrap(), DEFAULT_DATA_DIR);
        assert_eq!(cfg.reference_wav.file_name().unwrap(), "leijun-1.wav");
        assert_eq!(cfg.reference_text, DEFAULT_REFERENCE_TEXT);
        assert_eq!(cfg.num_steps, 4);
        assert_eq!(cfg.speed, 1.0);
        assert_eq!(cfg.provider, "cpu");
    }

    #[test]
    fn test_user_default_model_dir() {
        run_with_temp_home(|home| {
            let dir = super::user_default_model_dir();
            assert_eq!(
                dir,
                home.join(".zapmomo/models")
                    .join("qwen3-tts-06b-base-audiocpp")
            );
        });
    }

    #[test]
    fn test_choose_default_model_dir_priority() {
        let probe = crate::audiocpp::families::QWEN3_TTS_06B.gguf_file;
        let base = tempfile::tempdir().unwrap();
        let user = base.path().join("user-model");
        let repo = base.path().join("repo-model");

        assert_eq!(choose_default_model_dir(&user, None, &repo), user);

        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join(probe), b"t").unwrap();
        assert_eq!(choose_default_model_dir(&user, None, &repo), repo);

        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(user.join(probe), b"t").unwrap();
        assert_eq!(choose_default_model_dir(&user, None, &repo), user);

        std::fs::remove_file(user.join(probe)).unwrap();
        let legacy = base.path().join("legacy-model");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join(probe), b"t").unwrap();
        assert_eq!(
            choose_default_model_dir(&user, Some(&legacy), &repo),
            legacy
        );
    }

    /// 探针是 audiocpp 族主 GGUF，而非旧 sherpa 目录的 `tokens.txt`：
    /// 已装 qwen3 模型的目录（无 tokens.txt）必须被识别为「已安装」。
    #[test]
    fn test_default_dir_probe_is_family_gguf_not_legacy_tokens() {
        run_with_temp_home(|_| {
            let dir = user_default_model_dir();
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(crate::audiocpp::families::QWEN3_TTS_06B.gguf_file),
                b"x",
            )
            .unwrap();
            assert!(
                !dir.join(DEFAULT_TOKENS).exists(),
                "audiocpp 族目录不含旧 sherpa 的 tokens.txt"
            );
            assert_eq!(default_model_dir(), dir, "GGUF 探针应命中已装目录");
        });
    }

    #[test]
    fn test_resolve_enabled_default_true_and_override() {
        // 未配置时默认启用，避免破坏现有用户
        assert!(resolve(None, None).unwrap().enabled);
        // settings 显式关闭时生效
        let settings = TtsSettings {
            enabled: Some(false),
            ..TtsSettings::default()
        };
        assert!(!resolve(Some(&settings), None).unwrap().enabled);
    }

    #[test]
    fn test_resolve_no_settings_uses_defaults() {
        // 用临时 HOME 隔离，避免与其它 `run_with_temp_home` 测试并行时 HOME 竞态
        // 导致 `resolve` 与 `ResolvedTtsConfig::default` 两次读取到不同 HOME。
        run_with_temp_home(|_| {
            let cfg = resolve(None, None).unwrap();
            // 缺省 provider 按 audiocpp 平台取默认（mac Metal / Windows CUDA / 其余 CPU），
            // 与 `Default` 的中性 "cpu" 不同
            assert_eq!(
                cfg,
                ResolvedTtsConfig {
                    provider: crate::audiocpp::provider::current_default_provider().to_string(),
                    ..ResolvedTtsConfig::default()
                }
            );
        });
    }

    fn abs_path(rel: &str) -> PathBuf {
        std::path::absolute(rel).unwrap()
    }

    #[test]
    fn test_resolve_cli_model_dir_overrides_settings() {
        let settings = TtsSettings {
            model_dir: Some("settings-model".to_string()),
            ..TtsSettings::default()
        };
        let cli = abs_path("tmp/cli-tts");
        let cfg = resolve(Some(&settings), Some(&cli)).unwrap();
        assert_eq!(cfg.model_dir, cli);
        assert_eq!(cfg.encoder.parent().unwrap(), cli);
    }

    #[test]
    fn test_resolve_settings_overrides_numeric_and_text() {
        let settings = TtsSettings {
            num_threads: Some(4),
            num_steps: Some(6),
            speed: Some(1.5),
            reference_text: Some("自定义参考文本".to_string()),
            debug: Some(true),
            ..TtsSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.num_threads, 4);
        assert_eq!(cfg.num_steps, 6);
        assert_eq!(cfg.speed, 1.5);
        assert_eq!(cfg.reference_text, "自定义参考文本");
        assert!(cfg.debug);
    }

    #[test]
    fn test_resolve_relative_model_dir_anchored_to_user_dir() {
        run_with_temp_home(|home| {
            let settings = TtsSettings {
                model_dir: Some("models/my-tts".to_string()),
                ..TtsSettings::default()
            };
            let cfg = resolve(Some(&settings), None).unwrap();
            assert_eq!(cfg.model_dir, home.join(".zapmomo/models/my-tts"));
        });
    }

    #[test]
    fn test_resolve_voice_default_none_and_override() {
        // 未配置默认音色 → None（用 reference_wav 即 leijun）
        let cfg = resolve(None, None).unwrap();
        assert_eq!(cfg.voice, None);
        // settings 配置音色 id → 解析生效
        let settings = TtsSettings {
            voice: Some("custom-123".to_string()),
            ..TtsSettings::default()
        };
        let cfg = resolve(Some(&settings), None).unwrap();
        assert_eq!(cfg.voice.as_deref(), Some("custom-123"));
    }

    #[test]
    fn test_resolve_model_kind_default_and_legacy_fallback() {
        // 无字段 → 默认 Qwen3-TTS 0.6B
        assert_eq!(
            resolve(None, None).unwrap().model_type,
            TtsModelKind::Qwen3Tts06
        );
        // settings 显式 1.7B → 生效
        let settings = TtsSettings {
            model_type: Some(TtsModelKind::Qwen3Tts17),
            ..TtsSettings::default()
        };
        assert_eq!(
            resolve(Some(&settings), None).unwrap().model_type,
            TtsModelKind::Qwen3Tts17
        );
    }

    /// 老版本 settings 里的已移除 kind（zipvoice/omnivoice/kokoro 等）反序列化
    /// 不炸整份配置，回落默认 Qwen3-TTS 0.6B（升级迁移路径）。
    #[test]
    fn test_kind_deserialize_unknown_falls_back_to_default() {
        for legacy in ["kokoro", "zipvoice", "omnivoice", "voxcpm2", "kitten"] {
            let toml_str = format!(
                r#"
enabled = true
model_type = "{legacy}"
"#
            );
            let settings: TtsSettings = toml::from_str(&toml_str).unwrap();
            assert_eq!(
                settings.model_type,
                Some(TtsModelKind::Qwen3Tts06),
                "{legacy} 应回落默认 kind"
            );
        }
        // 合法值照常解析
        let toml_str = r#"
model_type = "qwen3_tts_17"
"#;
        let settings: TtsSettings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.model_type, Some(TtsModelKind::Qwen3Tts17));
    }

    /// qwen3_tts 两尺寸 kind 解析/序列化往返；未知串不解析。
    #[test]
    fn test_qwen3_tts_kind_semantics() {
        for (s, kind) in [
            ("qwen3_tts_06", TtsModelKind::Qwen3Tts06),
            ("qwen3_tts_17", TtsModelKind::Qwen3Tts17),
        ] {
            assert_eq!(TtsModelKind::parse_str(s), Some(kind), "{s}");
            assert_eq!(kind.as_str(), s);
        }
        assert_eq!(TtsModelKind::parse_str("zipvoice"), None);
        assert_eq!(TtsModelKind::parse_str("omnivoice"), None);
        assert_eq!(TtsModelKind::default(), TtsModelKind::Qwen3Tts06);
    }

    #[test]
    fn test_params_patch_applies_all_fields() {
        let mut tts = TtsSettings::default();
        let patch = TtsParamsPatch {
            num_steps: Some(8),
            speed: Some(1.2),
            num_threads: Some(4),
            debug: Some(true),
        };
        patch.apply_to(&mut tts).unwrap();
        assert_eq!(tts.num_steps, Some(8));
        assert_eq!(tts.speed, Some(1.2));
        assert_eq!(tts.num_threads, Some(4));
        assert_eq!(tts.debug, Some(true));
    }

    #[test]
    fn test_params_patch_validates_before_writing() {
        // 任一字段越界即整体失败，且不部分修改其它字段
        let mut tts = TtsSettings {
            num_steps: Some(4),
            ..TtsSettings::default()
        };
        let err = TtsParamsPatch {
            num_steps: Some(100),
            num_threads: Some(4),
            ..TtsParamsPatch::default()
        }
        .apply_to(&mut tts)
        .unwrap_err();
        assert!(err.contains("扩散步数"), "err: {err}");
        assert_eq!(tts.num_threads, None, "校验失败时不应写入其它字段");
        assert_eq!(tts.num_steps, Some(4));

        let err = TtsParamsPatch {
            speed: Some(3.0),
            ..TtsParamsPatch::default()
        }
        .apply_to(&mut TtsSettings::default())
        .unwrap_err();
        assert!(err.contains("语速"), "err: {err}");

        let err = TtsParamsPatch {
            num_threads: Some(64),
            ..TtsParamsPatch::default()
        }
        .apply_to(&mut TtsSettings::default())
        .unwrap_err();
        assert!(err.contains("线程数"), "err: {err}");
    }

    #[test]
    fn test_params_patch_none_leaves_unchanged() {
        let mut tts = TtsSettings {
            num_steps: Some(6),
            speed: Some(1.5),
            num_threads: Some(8),
            debug: Some(true),
            ..TtsSettings::default()
        };
        TtsParamsPatch::default().apply_to(&mut tts).unwrap();
        assert_eq!(tts.num_steps, Some(6));
        assert_eq!(tts.speed, Some(1.5));
        assert_eq!(tts.num_threads, Some(8));
        assert_eq!(tts.debug, Some(true));
    }

    #[test]
    fn test_backend_kind_str_and_parse() {
        for (s, kind) in [
            ("sherpa", TtsBackendKind::Sherpa),
            ("audiocpp", TtsBackendKind::Audiocpp),
        ] {
            assert_eq!(TtsBackendKind::parse_str(s), Some(kind), "{s}");
            assert_eq!(kind.as_str(), s);
        }
        assert_eq!(TtsBackendKind::parse_str("unknown"), None);
        // 缺省 audiocpp（唯一在册引擎）
        assert_eq!(TtsBackendKind::default(), TtsBackendKind::Audiocpp);
    }

    #[test]
    fn test_resolve_backend_default_explicit_and_invalid() {
        // 缺省 → audiocpp（唯一在册引擎）
        assert_eq!(
            resolve(None, None).unwrap().backend,
            TtsBackendKind::Audiocpp
        );
        // 显式 audiocpp → 生效
        let settings = TtsSettings {
            backend: Some("audiocpp".to_string()),
            ..TtsSettings::default()
        };
        assert_eq!(
            resolve(Some(&settings), None).unwrap().backend,
            TtsBackendKind::Audiocpp
        );
        // 老配置残留 sherpa：解析不炸（引擎层明确报错引导迁移）
        let settings = TtsSettings {
            backend: Some("sherpa".to_string()),
            ..TtsSettings::default()
        };
        assert_eq!(
            resolve(Some(&settings), None).unwrap().backend,
            TtsBackendKind::Sherpa
        );
        // 非法值 → 报错（含支持列表）
        let settings = TtsSettings {
            backend: Some("vllm".to_string()),
            ..TtsSettings::default()
        };
        let err = resolve(Some(&settings), None).unwrap_err();
        assert!(err.contains("未知 TTS 后端"), "err: {err}");
        assert!(err.contains("支持 audiocpp"), "err: {err}");
    }

    #[test]
    fn test_resolve_engine_path_passthrough() {
        // 未配置 → None（locator 自动定位）
        assert_eq!(resolve(None, None).unwrap().engine_path, None);
        // 显式配置 → 透传（支持 env 引用语义与 model_dir 一致）
        let settings = TtsSettings {
            engine_path: Some("/opt/audiocpp/audiocpp_server".to_string()),
            ..TtsSettings::default()
        };
        assert_eq!(
            resolve(Some(&settings), None).unwrap().engine_path,
            Some(PathBuf::from("/opt/audiocpp/audiocpp_server"))
        );
    }

    #[test]
    fn test_resolve_provider_platform_default_and_explicit_override() {
        // audiocpp 族缺省 provider 按平台取值（mac Metal / Windows CUDA / 其余 CPU）
        let cfg = resolve(None, None).unwrap();
        assert_eq!(
            cfg.provider,
            crate::audiocpp::provider::current_default_provider(),
            "audiocpp 缺省 provider 按平台取值"
        );
        // 显式 provider 永远优先（含显式 cpu——无 GPU 用户的手动兜底）
        let settings = TtsSettings {
            provider: Some("cpu".to_string()),
            ..TtsSettings::default()
        };
        assert_eq!(resolve(Some(&settings), None).unwrap().provider, "cpu");
    }

    /// qwen3 0.6B：空目录报缺 base gguf（提示语指向 qwen3 registry id），
    /// 单文件齐 → 通过；1.7B 校验各自的 _v2 gguf。
    #[test]
    fn test_preflight_audiocpp_qwen3() {
        let base = tempfile::tempdir().unwrap();
        let cfg = ResolvedTtsConfig {
            backend: crate::tts::config::TtsBackendKind::Audiocpp,
            model_type: TtsModelKind::Qwen3Tts06,
            model_dir: base.path().to_path_buf(),
            ..ResolvedTtsConfig::default()
        };

        // 空目录 -> 报缺 base gguf（提示语指向 qwen3 registry id）
        let err = preflight(&cfg).unwrap_err();
        assert!(
            err.contains("qwen3-tts-12hz-0.6b-base-q8_0.gguf"),
            "err: {err}"
        );
        assert!(err.contains("tts-qwen3-06b-base-q8-audiocpp"), "err: {err}");

        // 单文件齐 -> 通过
        std::fs::write(
            cfg.model_dir.join("qwen3-tts-12hz-0.6b-base-q8_0.gguf"),
            b"x",
        )
        .unwrap();
        assert!(preflight(&cfg).is_ok());
        assert!(models_present(&cfg));

        // 1.7B：0.6B 文件不算数，缺自己的 _v2 gguf
        let cfg17 = ResolvedTtsConfig {
            model_type: TtsModelKind::Qwen3Tts17,
            ..cfg.clone()
        };
        let err = preflight(&cfg17).unwrap_err();
        assert!(
            err.contains("qwen3-tts-12hz-1.7b-base-q8_0_v2.gguf"),
            "err: {err}"
        );
        assert!(err.contains("tts-qwen3-17b-base-q8-audiocpp"), "err: {err}");
        std::fs::write(
            cfg17
                .model_dir
                .join("qwen3-tts-12hz-1.7b-base-q8_0_v2.gguf"),
            b"x",
        )
        .unwrap();
        assert!(preflight(&cfg17).is_ok());
    }

    /// 老配置 `backend = "sherpa"`：预检明确报迁移错误（引擎已无进程内路径）。
    #[test]
    fn test_preflight_rejects_legacy_sherpa_backend() {
        let cfg = ResolvedTtsConfig {
            backend: TtsBackendKind::Sherpa,
            model_dir: PathBuf::from("/nonexistent/model"),
            ..ResolvedTtsConfig::default()
        };
        let err = preflight(&cfg).unwrap_err();
        assert!(err.contains("sherpa 已移除"), "err: {err}");
        assert!(!models_present(&cfg));
    }
}
