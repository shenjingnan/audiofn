/// 语音识别结果与「反应」接口。
///
/// `AsrReaction` trait 是可插拔的识别结果钩子：每次 `decode` 产出（部分或最终）
/// 转写文本后由 ASR 引擎调用。默认 `ConsoleAsrReaction` 打印到控制台；桌面 GUI
/// （Tauri）实现自己的 `AsrReaction`（发事件给前端显示实时字幕）。
use crate::kws::reaction::ReactionOutcome;
use serde::Serialize;
use sherpa_onnx::RecognizerResult;

/// 一次识别结果（owned 结构，避免把 sherpa 类型泄漏到公开 API）。
///
/// `Serialize` 供桌面 GUI 通过 Tauri 事件把结果发给前端。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AsrResult {
    /// 转写文本（部分或最终，取决于 `is_final`）
    pub text: String,
    /// token 序列
    pub tokens: Vec<String>,
    /// 各 token 的时间戳（秒）
    pub timestamps: Option<Vec<f32>>,
    /// 起始时间（秒）
    pub start_time: Option<f32>,
    /// 是否为最终结果（端点检测触发后为 true）
    pub is_final: bool,
}

impl From<&RecognizerResult> for AsrResult {
    fn from(r: &RecognizerResult) -> Self {
        Self {
            text: r.text.clone(),
            tokens: r.tokens.clone(),
            timestamps: r.timestamps.clone(),
            start_time: r.start_time,
            is_final: r.is_final,
        }
    }
}

/// 可插拔的识别反应接口。`Send` 允许反应被移动到其他线程（如 GUI 主线程）。
pub trait AsrReaction: Send {
    /// 识别出（部分/最终）文本时回调。返回 `Stop` 可终止识别循环。
    fn on_result(&mut self, result: &AsrResult) -> ReactionOutcome;
}

/// 默认反应：控制台打印 + tracing 日志。
pub struct ConsoleAsrReaction;

impl AsrReaction for ConsoleAsrReaction {
    fn on_result(&mut self, result: &AsrResult) -> ReactionOutcome {
        if result.is_final {
            println!("[识别] {}", result.text);
            tracing::info!(text = %result.text, "ASR final: {}", result.text);
        } else if !result.text.is_empty() {
            print!("\r[识别] {}", result.text);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
        ReactionOutcome::Continue
    }
}

/// 测试用反应：收集所有识别结果。
pub struct CollectAsrReaction {
    pub results: Vec<AsrResult>,
}

impl CollectAsrReaction {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }
}

impl Default for CollectAsrReaction {
    fn default() -> Self {
        Self::new()
    }
}

impl AsrReaction for CollectAsrReaction {
    fn on_result(&mut self, result: &AsrResult) -> ReactionOutcome {
        self.results.push(result.clone());
        ReactionOutcome::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_result() -> RecognizerResult {
        RecognizerResult {
            text: "你好世界".to_string(),
            tokens: vec!["你".to_string(), "好".to_string()],
            timestamps: Some(vec![0.1, 0.2]),
            segment: Some(0),
            start_time: Some(0.0),
            is_final: true,
        }
    }

    #[test]
    fn test_asr_result_from_recognizer_result() {
        let r = sample_result();
        let a: AsrResult = AsrResult::from(&r);
        assert_eq!(a.text, "你好世界");
        assert!(a.is_final);
        assert_eq!(a.tokens, vec!["你".to_string(), "好".to_string()]);
        assert_eq!(a.timestamps, Some(vec![0.1, 0.2]));
    }

    #[test]
    fn test_collect_reaction_collects() {
        let r = sample_result();
        let mut reaction = CollectAsrReaction::new();
        let outcome = reaction.on_result(&AsrResult::from(&r));
        assert_eq!(outcome, ReactionOutcome::Continue);
        assert_eq!(reaction.results.len(), 1);
        assert_eq!(reaction.results[0].text, "你好世界");
    }
}
