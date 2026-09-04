/// 语音对话记录持久化。
///
/// 把「用户一句 ↔ 桌宠一句」的对话记录序列化为 `~/.zapmomo/conversations.json`
/// （与 settings 同目录），跨应用重启保留。前端「对话记录」页首次打开时经
/// `get_conversation_records` 载入，会话进行中由事件转发层（`make_voice_emit`）
/// 逐条追加——本模块是纯文件级的无状态读写，不持有会话状态。
///
/// 并发：追加仅发生在语音会话线程（串行），读取发生在 Tauri 命令线程；用全局
/// `Mutex` 包住「读-改-写」，写文件采用「临时文件 + rename」保证读者要么看到旧
/// 文件要么看到新文件，不会读到半截内容。
use crate::config::settings;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// 记录文件最大条数（超出丢弃最旧的）。
const MAX_RECORDS: usize = 200;

/// 记录文件锁：串行化所有对记录文件的读改写。
static RECORDS_LOCK: Mutex<()> = Mutex::new(());

/// 记录角色。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordRole {
    User,
    Assistant,
}

/// 一条对话记录。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationRecord {
    pub role: RecordRole,
    pub text: String,
    /// ISO 8601 时间戳（`crate::datetime::iso_timestamp_now`）
    pub at: String,
}

/// 记录文件路径：`~/.zapmomo/conversations.json`。
pub fn records_path() -> PathBuf {
    settings::get_settings_dir().join("conversations.json")
}

/// 读取全部记录（文件不存在 / 解析失败 → 空列表）。
pub fn load_records() -> Vec<ConversationRecord> {
    let _guard = RECORDS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    load_records_unlocked()
}

/// 追加一条记录（cap 到 [`MAX_RECORDS`]）。写入失败仅记 warn，不影响会话流程。
pub fn append_record(rec: ConversationRecord) {
    let _guard = RECORDS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut records = load_records_unlocked();
    records.push(rec);
    if records.len() > MAX_RECORDS {
        let drop = records.len() - MAX_RECORDS;
        records.drain(..drop);
    }
    if let Err(e) = save_records(&records) {
        tracing::warn!("对话记录写入失败: {e}");
    }
}

/// 清空全部记录（删除文件；文件不存在视为成功）。
pub fn clear_records() -> Result<(), String> {
    let _guard = RECORDS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    match std::fs::remove_file(records_path()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("删除对话记录失败: {e}")),
    }
}

fn load_records_unlocked() -> Vec<ConversationRecord> {
    let path = records_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// 写记录文件（临时文件 + rename 替换，与 `save_settings` 同模式）。
fn save_records(records: &[ConversationRecord]) -> Result<(), String> {
    let path = records_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建记录目录失败: {e}"))?;
    }
    let content =
        serde_json::to_string_pretty(records).map_err(|e| format!("序列化记录失败: {e}"))?;
    let tmp = path.with_file_name(format!("conversations.json.tmp.{}", std::process::id()));
    std::fs::write(&tmp, &content).map_err(|e| format!("写入临时记录失败: {e}"))?;
    match std::fs::rename(&tmp, &path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows：目标存在时 rename 可能失败，先移除旧文件再重试
            if path.exists() {
                std::fs::remove_file(&path).map_err(|e| format!("移除旧记录失败: {e}"))?;
            }
            std::fs::rename(&tmp, &path).map_err(|e| format!("替换记录文件失败: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::run_with_temp_home;

    fn rec(role: RecordRole, text: &str) -> ConversationRecord {
        ConversationRecord {
            role,
            text: text.to_string(),
            at: "2026-08-19T10:00:00+08:00".to_string(),
        }
    }

    #[test]
    fn test_append_then_load_roundtrip() {
        run_with_temp_home(|_| {
            assert!(load_records().is_empty(), "初始应为空");
            append_record(rec(RecordRole::User, "你好"));
            append_record(rec(RecordRole::Assistant, "你好，我是桌宠！"));
            let records = load_records();
            assert_eq!(records.len(), 2);
            assert_eq!(records[0].role, RecordRole::User);
            assert_eq!(records[0].text, "你好");
            assert_eq!(records[1].role, RecordRole::Assistant);
            assert_eq!(records[1].text, "你好，我是桌宠！");
            assert_eq!(records[0].at, "2026-08-19T10:00:00+08:00");
        });
    }

    #[test]
    fn test_append_caps_at_max() {
        run_with_temp_home(|_| {
            for i in 0..(MAX_RECORDS + 50) {
                append_record(rec(RecordRole::User, &format!("消息{i}")));
            }
            let records = load_records();
            assert_eq!(records.len(), MAX_RECORDS);
            // 保留最新的：第一条是原第 50 条，最后一条是原第 MAX+49 条
            assert_eq!(records.first().unwrap().text, "消息50");
            assert_eq!(
                records.last().unwrap().text,
                format!("消息{}", MAX_RECORDS + 49)
            );
        });
    }

    #[test]
    fn test_clear_records_empties_file() {
        run_with_temp_home(|_| {
            append_record(rec(RecordRole::User, "你好"));
            assert_eq!(load_records().len(), 1);
            clear_records().unwrap();
            assert!(load_records().is_empty());
            // 再次清空（文件不存在）也应成功
            clear_records().unwrap();
        });
    }

    #[test]
    fn test_load_records_corrupt_file_returns_empty() {
        run_with_temp_home(|_| {
            std::fs::create_dir_all(records_path().parent().unwrap()).unwrap();
            std::fs::write(records_path(), "不是合法 json{{{").unwrap();
            assert!(load_records().is_empty());
        });
    }

    #[test]
    fn test_load_records_missing_file_returns_empty() {
        run_with_temp_home(|_| {
            assert!(load_records().is_empty());
        });
    }
}
