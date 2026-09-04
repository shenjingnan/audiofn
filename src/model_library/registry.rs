//! 模型库 Registry：编译期嵌入 `models/model_registry.json` 的目录解析。
//!
//! 一个 RegistryModel = 一个实际可加载的模型版本/变体（如 `asr-qwen3-0.6b-audiocpp`）。
//! 下载源（URL/sha256/size）不在此重复维护，而是通过 `download.manifest_role`
//! 引用 `models/manifest.json`（单一数据源）。

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::asr::config::AsrModelKind;
use crate::model_library::asset::{ModelAsset, asset_by_role};
use crate::tts::config::TtsModelKind;

/// 能力类型。
///
/// 一期裁剪后仅剩语音两族（KWS 随伴侣模块删除、LLM 改远程连接、声纹已移除）；
/// 历史 JSON/settings 里的 `kws`/`llm` 值反序列化为 `None`，由调用方按不支持处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    Asr,
    Tts,
}

impl ModelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelType::Asr => "asr",
            ModelType::Tts => "tts",
        }
    }

    pub fn from_str_value(s: &str) -> Option<Self> {
        match s {
            "asr" => Some(ModelType::Asr),
            "tts" => Some(ModelType::Tts),
            _ => None,
        }
    }
}

/// 顶层目录。
#[derive(Debug, Clone, Deserialize)]
pub struct ModelRegistry {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    pub models: Vec<RegistryModel>,
}

/// 单个目录条目。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryModel {
    pub id: String,
    /// 目录基名（= managed 安装目录名，`install_managed_model` 落位与缺省
    /// 模型目录解析的同一事实源）
    pub name: String,
    pub display_name: String,
    #[serde(rename = "model_type")]
    pub model_type: ModelType,
    /// TTS 子类型（qwen3_tts_06/qwen3_tts_17；仅 `model_type == Tts` 有意义，其余为 None）
    #[serde(default)]
    pub tts_kind: Option<TtsModelKind>,
    /// ASR 子类型（qwen3_asr；仅 `model_type == Asr` 有意义，其余为 None）
    #[serde(default)]
    pub asr_kind: Option<AsrModelKind>,
    pub runtime: String,
    pub format: String,
    pub description: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub parameter_count: Option<String>,
    #[serde(default)]
    pub quantization: Option<String>,
    pub version: String,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub homepage: Option<String>,
    /// 安装所需资产 role 列表（安装与完整性共用同一份定义）
    #[serde(default)]
    pub required_assets: Vec<String>,
    /// 可选增强资产 role 列表（缺失不影响可用性；当前 qwen3 三条目均为空）
    #[serde(default)]
    pub optional_assets: Vec<String>,
    /// 可用平台约束（`None` = 全平台；取值对齐 target triple 简写，如
    /// "darwin-aarch64"）。平台不符的条目在模型库中隐藏——如 audiocpp qwen3
    /// 依赖 GPU 加速，仅 macOS arm64 / Windows CUDA 的 sidecar 构建可用，
    /// 其余平台纯 CPU 实测不可用（技术方案 R1 预案）。
    #[serde(default)]
    pub platforms: Option<Vec<String>>,
    /// `None` = 无内置下载源（需导入本地文件；在册 qwen3 三条目均有 manifest 下载源）
    pub download: Option<RegistryDownload>,
}

/// 当前平台的 triple 简写（与 registry `platforms` 字段取值对齐；
/// crate 内共享事实源——audiocpp provider 平台缺省也从此取值）。
pub(crate) fn current_platform_triple() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "darwin-aarch64"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "darwin-x86_64"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "linux-x86_64"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "windows-x86_64"
    }
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    {
        "other"
    }
}

/// 下载引用：只存 manifest role，真实 URL/hash/size 由 manifest 单源解析。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryDownload {
    pub manifest_role: String,
    #[serde(default)]
    pub extra_roles: Vec<String>,
    #[serde(default)]
    pub kind: String,
}

const REGISTRY_JSON: &str = include_str!("../../models/model_registry.json");

/// 解析一次并缓存。
fn registry() -> &'static ModelRegistry {
    static CACHE: OnceLock<ModelRegistry> = OnceLock::new();
    CACHE.get_or_init(|| serde_json::from_str(REGISTRY_JSON).expect("内嵌模型目录无效"))
}

/// 所有目录条目（保持 JSON 顺序，即推荐顺序）。
pub fn all_models() -> &'static [RegistryModel] {
    &registry().models
}

/// 按 id 查找目录条目。
pub fn model_by_id(id: &str) -> Option<&'static RegistryModel> {
    registry().models.iter().find(|m| m.id == id)
}

/// 平台可见性判定（`platforms` 为 None = 全平台；纯函数便于按任意 triple 单测）。
pub fn platform_allows(m: &RegistryModel, triple: &str) -> bool {
    m.platforms
        .as_ref()
        .is_none_or(|list| list.iter().any(|p| p == triple))
}

/// 当前平台可用的目录条目（`platforms` 为 None 的条目恒可用）。
///
/// 模型库列表（`list_models`）与解析入口都应以此过滤，保证平台受限条目
/// （如 GPU 加速的 audiocpp 家族）在不支持的平台不可见、不可下载。
pub fn models_for_current_platform() -> Vec<&'static RegistryModel> {
    all_models()
        .iter()
        .filter(|m| platform_allows(m, current_platform_triple()))
        .collect()
}

/// 按 id 取「当前平台可见」的目录条目；平台不可见 → `None`。
///
/// 下载入口（`download_library_model`）以此替代 [`model_by_id`]，
/// 堵住「绕过 UI 平台过滤直接按 id 下载」的口子。
pub fn model_for_current_platform(id: &str) -> Option<&'static RegistryModel> {
    model_by_id(id).filter(|m| platform_allows(m, current_platform_triple()))
}

/// 按下载引用解析 manifest 资产。
pub fn asset_for(model: &RegistryModel) -> Option<&'static ModelAsset> {
    let role = model.download.as_ref()?.manifest_role.as_str();
    asset_by_role(role)
}

/// manifest role 对应的必需文件清单。
///
/// 安装（`install_raw_file_to_cancellable` 的幂等/校验）与完整性判断使用**同一份**定义，
/// 避免出现「安装要求 A+B、完整性只查 A」的不一致。
///
/// 一期裁剪后 manifest 只剩 qwen3 三资产（单文件 GGUF），必需文件名统一从
/// audiocpp 族表（`audiocpp::families` / `audiocpp::asr_families`）单源推导；
/// 已移除的 sherpa/KWS/标点 role 不再维护静态表（未知 role 返回空，只会让
/// 完整性校验失败，不会误判「已安装」）。
pub fn required_files_for_role(role: &str) -> &'static [&'static str] {
    match role {
        "asr-audiocpp-qwen3-06b" => &[crate::audiocpp::asr_families::QWEN3_ASR_06B.gguf_file],
        // Qwen3-TTS 两尺寸（音色克隆）：钉死各自 gguf 主文件名
        "tts-audiocpp-qwen3-06b" => &[crate::audiocpp::families::QWEN3_TTS_06B.gguf_file],
        "tts-audiocpp-qwen3-17b" => &[crate::audiocpp::families::QWEN3_TTS_17B.gguf_file],
        _ => &[],
    }
}

/// 按 registry id 查 TTS 子类型（非 TTS 或无 `tts_kind` 时返回 None）。
pub fn registry_tts_kind(id: &str) -> Option<TtsModelKind> {
    model_by_id(id).and_then(|m| m.tts_kind)
}

/// 按 registry id 查 ASR 子类型（非 ASR 或无 `asr_kind` 时返回 None）。
pub fn registry_asr_kind(id: &str) -> Option<AsrModelKind> {
    model_by_id(id).and_then(|m| m.asr_kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_parses() {
        let models = all_models();
        assert_eq!(
            models.len(),
            3,
            "模型清单收敛为 qwen3 三族：1 ASR + 2 TTS（KWS/LLM/声纹、sherpa 全族、omnivoice/voxcpm2 已移除）"
        );
        assert!(
            models
                .iter()
                .all(|m| !m.id.is_empty() && !m.display_name.is_empty())
        );
        // id 唯一
        let mut ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), models.len(), "Registry id 必须唯一");
        // 只剩 audiocpp 运行时的 qwen3 三族
        for m in models {
            assert_eq!(
                m.runtime, "audiocpp",
                "{} 应为 audiocpp 条目（sherpa 运行时已移除）",
                m.id
            );
        }
    }

    #[test]
    fn test_registry_manifest_roles_exist() {
        // 所有 download.manifest_role / required_assets / optional_assets 都必须在 manifest 中存在
        for m in all_models() {
            if let Some(d) = &m.download {
                assert!(
                    asset_by_role(&d.manifest_role).is_some(),
                    "manifest_role '{}' 在 manifest 中不存在 (model {})",
                    d.manifest_role,
                    m.id
                );
                for extra in &d.extra_roles {
                    assert!(
                        asset_by_role(extra).is_some(),
                        "extra_roles '{}' 在 manifest 中不存在 (model {})",
                        extra,
                        m.id
                    );
                }
            }
            for role in m.required_assets.iter().chain(m.optional_assets.iter()) {
                assert!(
                    asset_by_role(role).is_some(),
                    "asset role '{}' 在 manifest 中不存在 (model {})",
                    role,
                    m.id
                );
            }
        }
    }

    #[test]
    fn test_model_by_id_and_order() {
        let m = model_by_id("asr-qwen3-0.6b-audiocpp").expect("按 id 查找");
        assert_eq!(m.model_type, ModelType::Asr);
        // 推荐顺序 = registry 原始顺序（ASR 在前）
        assert_eq!(all_models()[0].id, "asr-qwen3-0.6b-audiocpp");
    }

    /// manifest role → 必需文件清单：只剩 qwen3 三 role，文件名单源 audiocpp 族表；
    /// 已移除的 sherpa/KWS/标点 role 返回空（不会误判「已安装」）。
    #[test]
    fn test_required_files_for_role() {
        // audiocpp Qwen3-ASR：单文件 GGUF（families 常量单源）
        assert_eq!(
            required_files_for_role("asr-audiocpp-qwen3-06b"),
            &[crate::audiocpp::asr_families::QWEN3_ASR_06B.gguf_file]
        );
        // Qwen3-TTS 两尺寸：钉死各自 gguf 主文件名（families 常量单源）
        assert_eq!(
            required_files_for_role("tts-audiocpp-qwen3-06b"),
            &[crate::audiocpp::families::QWEN3_TTS_06B.gguf_file]
        );
        assert_eq!(
            required_files_for_role("tts-audiocpp-qwen3-17b"),
            &[crate::audiocpp::families::QWEN3_TTS_17B.gguf_file]
        );
        // 已随清单裁剪移除的 role：空清单（完整性按「无必需文件」处理，不再命中旧表）
        for legacy in [
            "wake-word",
            "asr",
            "asr-sensevoice",
            "asr-whisper-tiny",
            "asr-paraformer-bilingual-zh-en",
            "asr-qwen3",
            "punctuation",
            "tts",
            "tts-vocoder",
        ] {
            assert!(
                required_files_for_role(legacy).is_empty(),
                "{legacy} 已随清单裁剪移除"
            );
        }
        assert!(required_files_for_role("unknown").is_empty());
    }

    #[test]
    fn test_registry_tts_kind() {
        assert_eq!(
            registry_tts_kind("tts-qwen3-06b-base-q8-audiocpp"),
            Some(TtsModelKind::Qwen3Tts06)
        );
        assert_eq!(
            registry_tts_kind("tts-qwen3-17b-base-q8-audiocpp"),
            Some(TtsModelKind::Qwen3Tts17)
        );
        // 已随清单裁剪移除的 TTS 条目：registry 反查不到 → None
        for id in [
            "tts-zipvoice-distill-int8",
            "tts-omnivoice-q8-audiocpp",
            "tts-voxcpm2-q8-audiocpp",
            "tts-vits-melo-zh-en",
            "tts-kokoro-multi-lang-v1-1",
        ] {
            assert!(
                model_by_id(id).and_then(|m| m.tts_kind).is_none(),
                "{id} 应已从 registry 移除"
            );
        }
        // 非 TTS 或不存在 → None
        assert_eq!(registry_tts_kind("asr-qwen3-0.6b-audiocpp"), None);
        assert_eq!(registry_tts_kind("不存在"), None);
    }

    /// 平台过滤：audiocpp qwen3 家族 = darwin-aarch64（Windows 构建随一期裁剪移除，
    /// 不再在册）；无 platforms 的条目全平台可见。本机命中解锁平台时条目在列；
    /// 其它平台的 CI 通过「显式三元组判定函数」覆盖，不依赖宿主平台。
    #[test]
    fn test_platforms_filter() {
        let expected = ["darwin-aarch64"];
        for id in [
            "tts-qwen3-06b-base-q8-audiocpp",
            "tts-qwen3-17b-base-q8-audiocpp",
        ] {
            let m = model_by_id(id).unwrap();
            assert_eq!(
                m.platforms.as_deref(),
                Some(&expected.map(String::from)[..]),
                "{id} 平台清单"
            );
        }
        // 显式判定（不依赖宿主平台）：解锁 darwin-aarch64；darwin-x86_64（引擎无
        // Metal）/ linux（CPU-only 引擎）/ windows（已移除）保持隐藏
        let q06 = model_by_id("tts-qwen3-06b-base-q8-audiocpp").unwrap();
        assert!(platform_allows(q06, "darwin-aarch64"));
        assert!(!platform_allows(q06, "windows-x86_64"));
        assert!(!platform_allows(q06, "darwin-x86_64"));
        assert!(!platform_allows(q06, "linux-x86_64"));
        // ASR 条目无平台约束（CPU 可跑，GPU 只是加速）
        let asr = model_by_id("asr-qwen3-0.6b-audiocpp").unwrap();
        assert!(asr.platforms.is_none());
        assert!(platform_allows(asr, "linux-x86_64"));
        // 全量条目在当前平台的过滤数 ≤ 总数
        assert!(models_for_current_platform().len() <= all_models().len());
    }

    /// 下载门控纯函数：`model_for_current_platform` 按 id + 当前平台过滤
    /// （宿主无关的路径以 `platform_allows` 覆盖，见 `test_platforms_filter`）。
    #[test]
    fn test_model_for_current_platform() {
        let triple = current_platform_triple();
        let unlocked = triple == "darwin-aarch64";
        let q06 = model_for_current_platform("tts-qwen3-06b-base-q8-audiocpp");
        assert_eq!(q06.is_some(), unlocked, "{triple} 上 Qwen3-TTS 可见性");
        // 无平台约束的 ASR 全平台可见
        assert!(model_for_current_platform("asr-qwen3-0.6b-audiocpp").is_some());
        // 不存在的 id 恒 None
        assert!(model_for_current_platform("不存在").is_none());
    }

    #[test]
    fn test_registry_asr_kind() {
        use crate::asr::config::AsrModelKind;
        // 唯一在册 ASR：audiocpp Qwen3-ASR 0.6B
        assert_eq!(
            registry_asr_kind("asr-qwen3-0.6b-audiocpp"),
            Some(AsrModelKind::Qwen3Asr)
        );
        // 非 ASR 或不存在 → None
        assert_eq!(registry_asr_kind("tts-qwen3-06b-base-q8-audiocpp"), None);
        assert_eq!(registry_asr_kind("不存在"), None);
        // 已随清单裁剪移除的模型（sherpa zipformer/Paraformer/SenseVoice/Whisper/
        // sherpa Qwen3）：registry 反查不到 → None，已装目录交回
        // detect_kind_from_dir 探测兜底
        assert_eq!(registry_asr_kind("asr-streaming-bilingual-zh-en"), None);
        assert_eq!(registry_asr_kind("asr-qwen3-0.6b"), None);
        assert_eq!(registry_asr_kind("asr-sensevoice-zh-en-ja-ko-yue"), None);
        assert_eq!(registry_asr_kind("asr-whisper-tiny"), None);
        assert_eq!(registry_asr_kind("asr-paraformer-bilingual-zh-en"), None);
    }

    /// 已裁剪的 manifest role 资产不可再解析（KWS/标点/TTS-sherpa/声纹资产已删除），
    /// 防止旧代码路径经 `asset_by_role` 拿到已下架资产。
    #[test]
    fn test_removed_manifest_roles_are_gone() {
        for role in [
            "wake-word",
            "asr",
            "punctuation",
            "tts",
            "tts-vocoder",
            "asr-vad",
            "tts-audiocpp-omnivoice",
            "tts-audiocpp-voxcpm2",
            "speaker-embedding",
        ] {
            assert!(
                asset_by_role(role).is_none(),
                "role {role} 应已从 manifest 移除"
            );
        }
        // 剩余资产恰为 qwen3 三项，且全部是裸单文件（raw）
        let roles = crate::model_library::asset::manifest_roles();
        assert_eq!(
            roles,
            [
                "tts-audiocpp-qwen3-06b",
                "tts-audiocpp-qwen3-17b",
                "asr-audiocpp-qwen3-06b"
            ]
        );
    }
}
