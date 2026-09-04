//! audiocpp 模型族静态描述表（单一事实源）。
//!
//! 每个接入 audio.cpp sidecar 的 TTS 模型族一条 [`AudiocppFamilyDesc`] 记录，
//! 取代此前散落在 `mod.rs` 常量 / `tts::config::preflight` / `server_config` /
//! `client` 各处的 pocket 单模型硬编码。一期只收录 Qwen3-TTS 两个尺寸；
//! 新增模型族 = 本表加一条记录 + registry/manifest 各一个条目 + 前端 preset 一条
//! （技术方案 §4.3）。

use crate::tts::config::TtsModelKind;
use std::path::Path;

/// 音色语义（决定 [`crate::tts::TtsVoiceParams`] 到请求体字段的映射，见 client）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceSemantics {
    /// 参考音频克隆（当前未收录，为二期加族保留的语义分支）：`Reference` →
    /// `voice_ref`+`reference_text`；`Named` → 透传 `voice`；
    /// `Sid`/缺省 → 省略 voice 字段（server auto voice）。
    ReferenceClone,
    /// 强制参考音频克隆（qwen3_tts Base）：与 [`VoiceSemantics::ReferenceClone`]
    /// 同款 `voice_ref`+`reference_text` 映射，但 `Sid`/缺省**必须拦截**--
    /// 上游 Base 版无 auto voice（实测报错 "requires voice clone reference
    /// audio"），AudioFn 侧提前报错给中文文案。
    ReferenceCloneRequired,
}

/// 单个 audiocpp 模型族的静态描述。
#[derive(Debug)]
pub struct AudiocppFamilyDesc {
    /// server config `models[].id` 与 `/v1/audio/speech` 请求体 `model`（两侧同源）。
    pub model_id: &'static str,
    /// audio.cpp `model_specs` 的 family 标识。
    pub family: &'static str,
    /// 主 GGUF 文件名（相对模型目录，与 manifest asset 一致）。
    pub gguf_file: &'static str,
    /// preflight / registry 完整性共用清单（相对模型目录）。
    pub required_files: &'static [&'static str],
    /// 输出采样率初值（Hz；client 首响应按 wav 头校准）。
    pub sample_rate: i32,
    /// 音色语义。
    pub voice_semantics: VoiceSemantics,
    /// 是否透传 `Named` 具名音色（ReferenceClone 族的差异项）：omnivoice 支持
    /// （server 端 preset/voice_dir 通道）；qwen3_tts 上游仅接受 speaker
    /// reference，具名请求会被 server 拒绝——client 据此提前拦截并给中文文案。
    /// 当前收录族恒 false，保留字段作为二期加族的差异项扩展点。
    pub allows_named_voice: bool,
    /// 是否支持 SSE 伪流式（server config `mode` 与请求体 `stream_format` 的依据）。
    /// 流式矩阵（audio.cpp release-0.6.1 实测/README）：omnivoice ✅、voxcpm2 ✅、
    /// qwen3_tts ❌（上游 modes 仅 offline）、sherpa 全族 ❌
    /// （`OfflineTts` 整段合成，无 sidecar 语义）。
    /// offline-mode server 会拒绝 SSE 请求（实测 HTTP 500），故该标记同时决定
    /// server config 的 `mode:"streaming"` 翻转——两者必须同源。
    /// 当前收录族恒 false，保留字段作为二期加流的差异项扩展点。
    pub supports_streaming: bool,
    /// preflight 缺文件时的安装提示命令。
    pub registry_hint: &'static str,
}

impl AudiocppFamilyDesc {
    /// server config `load_options`（当前收录族均自动推导，恒空对象；
    /// 保留方法作为新增模型族的族差异扩展点）。
    pub fn load_options(&self) -> serde_json::Value {
        serde_json::json!({})
    }

    /// 请求体 `options` 的族差异项（整段与流式两路径都携带）。
    ///
    /// 当前收录的 qwen3_tts 族无族差异项（恒空对象）。此前 voxcpm2 的
    /// `"retry_badcase": false` 硬约束随该族一并移除——二期加族若上游有类似
    /// offline-only 约束，收敛在本方法而非流式专用路径。
    pub fn request_options(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}

/// Qwen3-TTS 0.6B Base q8_0（10 语种 3 秒音色克隆，24kHz）。
///
/// 单文件 GGUF（权重 + speech tokenizer + 全部 sidecar 内嵌，实测
/// `audiocpp.embedded_files` 含 11 个文件）。**Base 版必须参考音频**（无
/// auto voice 兜底，见 `VoiceSemantics::ReferenceCloneRequired`）；CustomVoice/
/// VoiceDesign 变体不在本期接入范围。GGUF 文件名无 `_v2` 后缀。
pub const QWEN3_TTS_06B: AudiocppFamilyDesc = AudiocppFamilyDesc {
    model_id: "qwen3-tts-0.6b",
    family: "qwen3_tts",
    gguf_file: "qwen3-tts-12hz-0.6b-base-q8_0.gguf",
    required_files: &["qwen3-tts-12hz-0.6b-base-q8_0.gguf"],
    sample_rate: 24_000,
    voice_semantics: VoiceSemantics::ReferenceCloneRequired,
    allows_named_voice: false,
    supports_streaming: false,
    registry_hint: "audiofn tts install-model --registry-id tts-qwen3-06b-base-q8-audiocpp",
};

/// Qwen3-TTS 1.7B Base q8_0（质量优先变体；GGUF 为上游 `_v2` 重打包版，文件名带 `_v2`）。
///
/// 同 0.6B 语义；1.7B RTF 预计 ~1.0+，句级流水线可能句间间隙，定位质量优先。
pub const QWEN3_TTS_17B: AudiocppFamilyDesc = AudiocppFamilyDesc {
    model_id: "qwen3-tts-1.7b",
    family: "qwen3_tts",
    gguf_file: "qwen3-tts-12hz-1.7b-base-q8_0_v2.gguf",
    required_files: &["qwen3-tts-12hz-1.7b-base-q8_0_v2.gguf"],
    sample_rate: 24_000,
    voice_semantics: VoiceSemantics::ReferenceCloneRequired,
    allows_named_voice: false,
    supports_streaming: false,
    registry_hint: "audiofn tts install-model --registry-id tts-qwen3-17b-base-q8-audiocpp",
};

/// 已收录模型族全表（目录 GGUF 探测与覆盖断言共用）。
pub const ALL_FAMILIES: &[&AudiocppFamilyDesc] = &[&QWEN3_TTS_06B, &QWEN3_TTS_17B];

/// 按模型类型查表；未收录 kind 返回 None（audiocpp 后端不支持该组合）。
pub fn family_desc(kind: TtsModelKind) -> Option<&'static AudiocppFamilyDesc> {
    match kind {
        TtsModelKind::Qwen3Tts06 => Some(&QWEN3_TTS_06B),
        TtsModelKind::Qwen3Tts17 => Some(&QWEN3_TTS_17B),
    }
}

/// 目录内按 GGUF 主文件名探测 audiocpp TTS 族（模型库完整性/安装态判断用）。
///
/// 与 ASR 的 [`super::asr_families::detect_gguf_in_dir`] 平行：外部导入/手工放置
/// 目录没有 kind 元数据，只能靠族清单文件名反查。
pub fn detect_family_in_dir(dir: &Path) -> Option<&'static AudiocppFamilyDesc> {
    ALL_FAMILIES
        .iter()
        .copied()
        .find(|d| dir.join(d.gguf_file).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 表覆盖锚点：qwen3 两尺寸可查，且全表恰为这两个族（无多余遗留条目）。
    #[test]
    fn test_family_desc_coverage() {
        assert_eq!(
            family_desc(TtsModelKind::Qwen3Tts06).unwrap().family,
            "qwen3_tts"
        );
        assert_eq!(
            family_desc(TtsModelKind::Qwen3Tts17).unwrap().family,
            "qwen3_tts"
        );
        assert_eq!(
            ALL_FAMILIES.len(),
            2,
            "一期只收录 qwen3_tts 两尺寸，不应有遗留族"
        );
    }

    /// qwen3_tts 两尺寸记录形状：单文件清单 / 强制克隆 / 无流式 / 提示语可执行。
    #[test]
    fn test_family_records_shape() {
        let q06 = family_desc(TtsModelKind::Qwen3Tts06).unwrap();
        assert_eq!(q06.model_id, "qwen3-tts-0.6b");
        assert_eq!(q06.family, "qwen3_tts");
        assert_eq!(q06.gguf_file, "qwen3-tts-12hz-0.6b-base-q8_0.gguf");
        assert_eq!(q06.required_files, &["qwen3-tts-12hz-0.6b-base-q8_0.gguf"]);
        assert_eq!(q06.sample_rate, 24_000);
        assert_eq!(q06.voice_semantics, VoiceSemantics::ReferenceCloneRequired);
        assert!(!q06.allows_named_voice, "Base 版仅接受 speaker reference");
        assert!(!q06.supports_streaming, "上游 modes 仅 offline");
        assert!(q06.registry_hint.contains("tts-qwen3-06b-base-q8-audiocpp"));
        assert_eq!(q06.load_options(), serde_json::json!({}));
        assert_eq!(q06.request_options(), serde_json::json!({}));

        let q17 = family_desc(TtsModelKind::Qwen3Tts17).unwrap();
        assert_eq!(q17.model_id, "qwen3-tts-1.7b");
        assert_eq!(
            q17.gguf_file, "qwen3-tts-12hz-1.7b-base-q8_0_v2.gguf",
            "1.7B 为上游 _v2 重打包版"
        );
        assert_eq!(
            q17.required_files,
            &["qwen3-tts-12hz-1.7b-base-q8_0_v2.gguf"]
        );
        assert_eq!(q17.sample_rate, 24_000);
        assert_eq!(q17.voice_semantics, VoiceSemantics::ReferenceCloneRequired);
        assert!(!q17.allows_named_voice);
        assert!(!q17.supports_streaming);
        assert!(q17.registry_hint.contains("tts-qwen3-17b-base-q8-audiocpp"));
        assert_eq!(q17.load_options(), serde_json::json!({}));
        assert_eq!(q17.request_options(), serde_json::json!({}));
    }

    /// 目录 GGUF 探测：命中两尺寸 / 空目录 / 不存在目录。
    #[test]
    fn test_detect_family_in_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_family_in_dir(dir.path()).is_none());
        std::fs::write(dir.path().join(QWEN3_TTS_17B.gguf_file), b"x").unwrap();
        assert_eq!(
            detect_family_in_dir(dir.path()).unwrap().model_id,
            "qwen3-tts-1.7b"
        );
        assert!(detect_family_in_dir(Path::new("/nonexistent-tts-gguf")).is_none());
    }
}
