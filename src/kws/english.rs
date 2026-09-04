//! 英文关键词 → ARPAbet 音素 token。
//!
//! 模型（zipformer-zh-en）的 tokens.txt 中英文部分即标准 CMUdict ARPAbet 音素
//! （`AA0`..`ZH`，带 0/1/2 重音）。英文唤醒词需先转成这样的音素序列才能编码。
//!
//! 策略：查模型包自带的 `en.phone` 发音词典（CMUdict 格式，12.6 万词，精确且零新依赖）。
//! 词典未命中的词（如自定义品牌名 `momo`）无法自动转换，需用户改用手写音素。
use std::collections::HashMap;
use std::path::Path;

/// 读取 en.phone 词典（每行 `WORD PHONEMES`，大写；`WORD(N)` 为候补读音）。
/// 只保留基础发音（无 `(N)` 后缀），key 为词的大写形式，value 为音素串。
pub fn load_phone_dict(path: &Path) -> Result<HashMap<String, String>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("无法读取发音词典 {}: {}", path.display(), e))?;
    let mut dict = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(word) = parts.next() else {
            continue;
        };
        // 跳过候补读音条目（如 `HI(2)`）
        if word.ends_with(')') {
            continue;
        }
        let phonemes = parts.collect::<Vec<_>>().join(" ");
        if phonemes.is_empty() {
            continue;
        }
        // 同词只保留首个（基础）发音
        dict.entry(word.to_string()).or_insert(phonemes);
    }
    if dict.is_empty() {
        return Err(format!("发音词典 {} 为空", path.display()));
    }
    Ok(dict)
}

/// 把一个英文单词转成 ARPAbet 音素 token 序列（词典查表）。
fn word_to_tokens(word: &str, dict: &HashMap<String, String>) -> Result<Vec<String>, String> {
    let key = word.to_uppercase();
    match dict.get(&key) {
        Some(phonemes) => Ok(phonemes.split_whitespace().map(str::to_string).collect()),
        None => Err(format!(
            "单词 `{word}` 不在发音词典中。\n\
             请使用模型支持的拼音/音素格式（如 `HH AY1`），或改用词典中已有的英文单词。"
        )),
    }
}

/// 把一个英文短语（按空白拆词）转成扁平的 ARPAbet 音素 token 列表。
pub fn english_phrase_to_tokens(phrase: &str, dict_path: &Path) -> Result<Vec<String>, String> {
    let dict = load_phone_dict(dict_path)?;
    let mut out = Vec::new();
    for word in phrase.split_whitespace() {
        let tokens = word_to_tokens(word, &dict)?;
        out.extend(tokens);
    }
    if out.is_empty() {
        return Err(format!("短语 `{phrase}` 中没有可转换的英文单词"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_phone_dict(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn test_load_phone_dict_parses_and_keeps_primary() {
        let f = temp_phone_dict("HI HH AY1\nHI(2) HH AY1\nHELLO HH AH0 L OW1\n");
        let dict = load_phone_dict(f.path()).unwrap();
        assert_eq!(dict.get("HI").map(String::as_str), Some("HH AY1"));
        assert_eq!(dict.get("HELLO").map(String::as_str), Some("HH AH0 L OW1"));
        assert!(!dict.contains_key("HI(2)"), "应跳过候补读音条目");
    }

    #[test]
    fn test_word_to_tokens_case_insensitive_dict_hit() {
        let f = temp_phone_dict("HI HH AY1\n");
        let dict = load_phone_dict(f.path()).unwrap();
        assert_eq!(word_to_tokens("hi", &dict).unwrap(), vec!["HH", "AY1"]);
        assert_eq!(word_to_tokens("Hi", &dict).unwrap(), vec!["HH", "AY1"]);
    }

    #[test]
    fn test_word_to_tokens_oov_errors() {
        // 词典未命中的 OOV 词（如自定义品牌名）应给出清晰报错，提示改用手写音素
        let f = temp_phone_dict("HI HH AY1\n");
        let dict = load_phone_dict(f.path()).unwrap();
        let err = word_to_tokens("momo", &dict).unwrap_err();
        assert!(err.contains("momo"), "err: {err}");
        assert!(err.contains("HH AY1"), "应提示手写音素示例, err: {err}");
    }

    #[test]
    fn test_english_phrase_to_tokens_dict_path() {
        let f = temp_phone_dict("HI HH AY1\nHELLO HH AH0 L OW1\n");
        let tokens = english_phrase_to_tokens("hi hello", f.path()).unwrap();
        assert_eq!(tokens, vec!["HH", "AY1", "HH", "AH0", "L", "OW1"]);
    }

    #[test]
    fn test_english_phrase_empty_errors() {
        let f = temp_phone_dict("HI HH AY1\n");
        assert!(english_phrase_to_tokens("   ", f.path()).is_err());
    }
}
