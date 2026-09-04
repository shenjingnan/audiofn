/// 唤醒词检测结果与「反应」接口。
///
/// `Reaction` trait 是可插拔的唤醒反应钩子：检测到唤醒词后由 KWS 引擎调用。
/// 默认实现 `ConsoleReaction` 打印到控制台并写 tracing 日志；桌面 GUI（Tauri）
/// 实现自己的 `Reaction`（弹窗、播放提示音、发事件给前端）接入。
use serde::Serialize;
use sherpa_onnx::KeywordResult;

/// 一次唤醒词检测结果（owned 结构，避免把 sherpa 类型泄漏到公开 API）。
///
/// `Serialize` 供桌面 GUI 通过 Tauri 事件把结果发给前端。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KwsResult {
    /// 显示词（如「文森特卡索」）
    pub keyword: String,
    /// 命中的 token 序列（原始字符串）
    pub tokens: String,
    /// 命中的 token 序列（数组形式）
    pub tokens_arr: Vec<String>,
    /// 各 token 的时间戳（秒）
    pub timestamps: Vec<f32>,
    /// 唤醒词起始时间（秒）
    pub start_time: f32,
    /// 原始 JSON 结果
    pub json: String,
}

impl From<&KeywordResult> for KwsResult {
    fn from(r: &KeywordResult) -> Self {
        Self {
            keyword: r.keyword.clone(),
            tokens: r.tokens.clone(),
            tokens_arr: r.tokens_arr.clone(),
            timestamps: r.timestamps.clone(),
            start_time: r.start_time,
            json: r.json.clone(),
        }
    }
}

/// 反应控制信号：`Continue` = 继续监听，`Stop` = 停止检测。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionOutcome {
    Continue,
    Stop,
}

/// 可插拔的唤醒反应接口。`Send` 允许反应被移动到其他线程（如 GUI 主线程）。
pub trait Reaction: Send {
    /// 检测到唤醒词时回调。返回 `Stop` 可终止检测循环。
    fn on_keyword(&mut self, result: &KwsResult) -> ReactionOutcome;
}

/// 默认反应：控制台打印 + tracing 日志。
pub struct ConsoleReaction;

impl Reaction for ConsoleReaction {
    fn on_keyword(&mut self, result: &KwsResult) -> ReactionOutcome {
        println!(
            "[唤醒] 检测到: {} (start_time={:.2}s)",
            result.keyword, result.start_time
        );
        tracing::info!(
            keyword = %result.keyword,
            start_time = result.start_time,
            "KWS keyword detected: {}",
            result.json
        );
        ReactionOutcome::Continue
    }
}

/// 测试用反应：收集所有检测结果。
pub struct CollectReaction {
    pub results: Vec<KwsResult>,
}

impl CollectReaction {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }
}

impl Default for CollectReaction {
    fn default() -> Self {
        Self::new()
    }
}

impl Reaction for CollectReaction {
    fn on_keyword(&mut self, result: &KwsResult) -> ReactionOutcome {
        self.results.push(result.clone());
        ReactionOutcome::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_result() -> KeywordResult {
        KeywordResult {
            keyword: "文森特卡索".to_string(),
            tokens: "w é n ...".to_string(),
            tokens_arr: vec!["w".to_string(), "én".to_string()],
            timestamps: vec![0.64, 0.76],
            start_time: 0.64,
            json: r#"{"start_time":0.64,"keyword":"文森特卡索"}"#.to_string(),
        }
    }

    #[test]
    fn test_kws_result_from_keyword_result() {
        let r = sample_result();
        let kws: KwsResult = KwsResult::from(&r);
        assert_eq!(kws.keyword, "文森特卡索");
        assert_eq!(kws.start_time, 0.64);
        assert_eq!(kws.tokens_arr, vec!["w".to_string(), "én".to_string()]);
        assert_eq!(kws.timestamps, vec![0.64, 0.76]);
        assert!(kws.json.contains("文森特卡索"));
    }

    #[test]
    fn test_console_reaction_continues() {
        let r = sample_result();
        let mut reaction = ConsoleReaction;
        let outcome = reaction.on_keyword(&KwsResult::from(&r));
        assert_eq!(outcome, ReactionOutcome::Continue);
    }

    #[test]
    fn test_collect_reaction_collects() {
        let r = sample_result();
        let mut reaction = CollectReaction::new();
        let outcome = reaction.on_keyword(&KwsResult::from(&r));
        assert_eq!(outcome, ReactionOutcome::Continue);
        assert_eq!(reaction.results.len(), 1);
        assert_eq!(reaction.results[0].keyword, "文森特卡索");
    }

    #[test]
    fn test_reaction_outcome_partial_eq() {
        assert_eq!(ReactionOutcome::Continue, ReactionOutcome::Continue);
        assert_ne!(ReactionOutcome::Continue, ReactionOutcome::Stop);
    }
}
