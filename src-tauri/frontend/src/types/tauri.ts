// Tauri 后端命令 / 事件的类型契约。
// 与 src-tauri/src/lib.rs 的命令签名一一对应。

/** `get_app_info` 返回 */
export interface AppInfo {
  version: string;
  product_name: string;
}

/** `asr-dictate-started/stopped`、`tts-stopped` 事件载荷（正常停止时 error 为 null） */
export interface ListenStopped {
  error: string | null;
}

/** `asr-model-download-progress` 事件载荷 */
export type DownloadStage = "downloading" | "verifying" | "done";

export interface DownloadProgress {
  stage: DownloadStage;
  percent: number;
  message: string;
}

/** `get_asr_config` 返回（含可经 `set_asr_params` 调整的引擎参数） */
export interface AsrConfigInfo {
  enabled: boolean;
  /** 模型类型（zipformer/sensevoice/whisper/qwen3_asr...），前端据此切换参数与文案 */
  model_type: string;
  /** 推理后端（sherpa/audiocpp）：audiocpp 时显示 audio.cpp 标识并隐藏热词参数 */
  backend: string;
  model_dir: string;
  provider: string;
  num_threads: number;
  sample_rate: number;
  chunk_size: number;
  decoding_method: string;
  enable_endpoint: boolean;
  rule1_min_trailing_silence: number;
  rule2_min_trailing_silence: number;
  rule3_min_utterance_length: number;
  blank_penalty: number;
  hotwords: string | null;
  enable_punctuation: boolean;
  debug: boolean;
  models_present: boolean;
  punctuation_present: boolean;
  model_downloading: boolean;
  settings_path: string;
}

/** `set_asr_params` 载荷：可调整的 ASR 引擎/运行参数（snake_case 直传，缺省项不修改）。 */
export interface AsrParamsPatch {
  num_threads?: number;
  chunk_size?: number;
  enable_endpoint?: boolean;
  rule1_min_trailing_silence?: number;
  rule2_min_trailing_silence?: number;
  rule3_min_utterance_length?: number;
  blank_penalty?: number;
  hotwords?: string;
  enable_punctuation?: boolean;
  language?: string;
  use_itn?: boolean;
  debug?: boolean;
}

/** `transcribe_audio` 返回（snake_case 直传） */
export interface TranscribeResult {
  text: string;
  model_type: string;
  model_dir: string;
}

/** `asr-dictate-result` 事件载荷（对应后端 AsrResult） */
export interface AsrResult {
  text: string;
  is_final: boolean;
}

/** `get_tts_config` 返回 */
export interface TtsConfigInfo {
  /** 模型类型（zipvoice/omnivoice/...），前端据此切换音色语义 */
  model_type: string;
  /** 推理后端（sherpa/audiocpp），前端据此显示引擎徽标 */
  backend: string;
  model_dir: string;
  provider: string;
  num_threads: number;
  enabled: boolean;
  models_present: boolean;
  model_downloading: boolean;
  settings_path: string;
  /** 扩散解码步数（质量/速度权衡），可经 `set_tts_params` 修改 */
  num_steps: number;
  /** 默认语速，可经 `set_tts_params` 修改 */
  speed: number;
  /** 调试输出，可经 `set_tts_params` 修改 */
  debug: boolean;
  /** 默认音色 id（`null` = 引擎内置音色），可经 `set_tts_voice` 修改 */
  voice: string | null;
}

/** `set_tts_params` 载荷：可调整的 TTS 合成参数（snake_case 直传，缺省项不修改）。 */
export interface TtsParamsPatch {
  num_steps?: number;
  speed?: number;
  num_threads?: number;
  debug?: boolean;
}

/** `tts-result` 事件载荷（对应后端 TtsResult） */
export interface TtsResult {
  path: string;
  duration: number;
  sample_rate: number;
}

/** `tts-progress` 事件载荷（对应后端 TtsProgress） */
export interface TtsProgress {
  percent: number;
}

/** `list_tts_voices` 返回的音色（对应后端 TtsVoice） */
export interface TtsVoice {
  id: string;
  name: string;
  wav_path: string;
  reference_text: string;
  /** 是否为用户自定义音色（true = 来自音色库，false = 模型包内置） */
  custom: boolean;
}

/** `save_tts_voice` 载荷：把源 wav 拷贝进音色库并登记。 */
export type SaveTtsVoiceRequest = {
  name: string;
  sourceWavPath: string;
  referenceText: string;
};

// ---- 全局快捷键 ----

/** 可绑定全局快捷键的操作标识（与 Rust `ShortcutAction::as_str` 一致）。 */
export type ShortcutActionId = "open_settings";
