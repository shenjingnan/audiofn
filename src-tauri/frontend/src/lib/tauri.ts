import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  LibraryModel,
  ModelLibraryProgress,
  SetCurrentResult,
  StorageInfo,
  StorageMigrateProgress,
  StoragePrompt,
  SystemResources,
} from "@/types/modelLibrary";
import type {
  AppInfo,
  AsrConfigInfo,
  AsrParamsPatch,
  AsrResult,
  DownloadProgress,
  ListenStopped,
  SaveTtsVoiceRequest,
  ShortcutActionId,
  TranscribeResult,
  TtsConfigInfo,
  TtsParamsPatch,
  TtsProgress,
  TtsResult,
  TtsVoice,
} from "@/types/tauri";

/** 类型安全的 Tauri command 封装。 */
export const api = {
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  listDevices: () => invoke<string[]>("list_devices"),
  requestMicPermission: () => invoke<boolean>("request_mic_permission"),
  getMicrophone: () => invoke<string>("get_microphone"),
  setMicrophone: (args: { mic: string }) => invoke<void>("set_microphone", args),
  getAsrConfig: () => invoke<AsrConfigInfo>("get_asr_config"),
  setAsrEnabled: (args: { enabled: boolean }) => invoke<void>("set_asr_enabled", args),
  setAsrParams: (args: { params: AsrParamsPatch }) => invoke<void>("set_asr_params", args),
  startAsrDictate: (args: { device: string | null }) => invoke<void>("start_asr_dictate", args),
  stopAsrDictate: () => invoke<void>("stop_asr_dictate"),
  isAsrDictating: () => invoke<boolean>("is_asr_dictating"),
  downloadAsrModel: () => invoke<void>("download_asr_model"),
  transcribeAudio: (args: { wavPath: string | null }) =>
    invoke<TranscribeResult>("transcribe_audio", args),
  getTtsConfig: () => invoke<TtsConfigInfo>("get_tts_config"),
  listTtsVoices: () => invoke<TtsVoice[]>("list_tts_voices"),
  saveTtsVoice: (args: SaveTtsVoiceRequest) => invoke<TtsVoice>("save_tts_voice", args),
  deleteTtsVoice: (args: { id: string }) => invoke<void>("delete_tts_voice", args),
  recordTtsVoice: (args: { seconds: number; device: string | null }) =>
    invoke<string>("record_tts_voice", args),
  transcribeReferenceAudio: (args: { wavPath: string }) =>
    invoke<string>("transcribe_reference_audio", args),
  synthesizeTts: (args: {
    text: string;
    speed: number | null;
    voice: string | null;
    referenceWav: string | null;
    referenceText: string | null;
  }) => invoke<void>("synthesize_tts", args),
  stopTts: () => invoke<void>("stop_tts"),
  isTtsSynthesizing: () => invoke<boolean>("is_tts_synthesizing"),
  setTtsEnabled: (args: { enabled: boolean }) => invoke<void>("set_tts_enabled", args),
  setTtsParams: (args: { params: TtsParamsPatch }) => invoke<void>("set_tts_params", args),
  setTtsVoice: (voice: string | null) => invoke<void>("set_tts_voice", { voice }),
  /** 切换 TTS 推理后端（sherpa/audiocpp）；常规入口是「选择模型」弹窗的设为当前 */
  setTtsBackend: (backend: string) => invoke<void>("set_tts_backend", { backend }),
  // ---- 模型列表（registry 预设 + 安装状态；供各「选择模型」弹窗）----
  listModelLibrary: () => invoke<LibraryModel[]>("list_model_library"),
  getSystemResources: () => invoke<SystemResources>("get_system_resources"),
  // ---- 存储位置（数据目录）----
  getStorageInfo: () => invoke<StorageInfo>("get_storage_info"),
  getStoragePrompt: () => invoke<StoragePrompt>("get_storage_prompt"),
  acknowledgeStoragePrompt: () => invoke<void>("acknowledge_storage_prompt"),
  setStorageDir: (args: { path: string | null }) => invoke<StorageInfo>("set_data_dir", args),
  migrateStorage: () => invoke<void>("migrate_storage"),
  cancelStorageMigration: () => invoke<void>("cancel_storage_migration"),
  openStorageDir: () => invoke<void>("open_storage_dir"),
  downloadLibraryModel: (args: { id: string }) => invoke<void>("download_library_model", args),
  cancelModelDownload: () => invoke<void>("cancel_model_download"),
  setCurrentModel: (args: { id: string }) => invoke<SetCurrentResult>("set_current_model", args),
  deleteModel: (args: { id: string }) => invoke<void>("delete_model", args),
  getAutostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (args: { enabled: boolean }) => invoke<void>("set_autostart", args),
  getShortcuts: () => invoke<Record<string, string>>("get_shortcuts"),
  setShortcut: (args: { action: ShortcutActionId; accelerator: string }) =>
    invoke<void>("set_shortcut", args),
  clearShortcut: (args: { action: ShortcutActionId }) => invoke<void>("clear_shortcut", args),
  openSettings: () => invoke<void>("open_settings"),
  quitApp: () => invoke<void>("quit_app"),
  restartApp: () => invoke<void>("restart_app"),
};

/** 类型安全的事件订阅（返回的 Promise resolve 后得到取消订阅函数）。 */

/** ASR 模型下载进度（`asr-model-download-progress`）。 */
export function onAsrDownloadProgress(
  handler: (payload: DownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen<DownloadProgress>("asr-model-download-progress", (e) => handler(e.payload));
}

export function onAsrDictateResult(handler: (result: AsrResult) => void): Promise<UnlistenFn> {
  return listen<AsrResult>("asr-dictate-result", (e) => handler(e.payload));
}

export function onAsrDictateStarted(
  handler: (payload: ListenStopped) => void,
): Promise<UnlistenFn> {
  return listen<ListenStopped>("asr-dictate-started", (e) => handler(e.payload));
}

export function onAsrDictateStopped(
  handler: (payload: ListenStopped) => void,
): Promise<UnlistenFn> {
  return listen<ListenStopped>("asr-dictate-stopped", (e) => handler(e.payload));
}

export function onTtsResult(handler: (result: TtsResult) => void): Promise<UnlistenFn> {
  return listen<TtsResult>("tts-result", (e) => handler(e.payload));
}

export function onTtsProgress(handler: (p: TtsProgress) => void): Promise<UnlistenFn> {
  return listen<TtsProgress>("tts-progress", (e) => handler(e.payload));
}

export function onTtsStopped(handler: (payload: ListenStopped) => void): Promise<UnlistenFn> {
  return listen<ListenStopped>("tts-stopped", (e) => handler(e.payload));
}

/** 开机自启动状态变化（设置页为唯一订阅者：托盘菜单改动后同步开关）。 */
export function onAutostartChanged(handler: (enabled: boolean) => void): Promise<UnlistenFn> {
  return listen<boolean>("autostart-changed", (e) => handler(e.payload));
}

export function onModelLibraryDownloadProgress(
  handler: (p: ModelLibraryProgress) => void,
): Promise<UnlistenFn> {
  return listen<ModelLibraryProgress>("model-library-download-progress", (e) => handler(e.payload));
}

/** 存储迁移进度（`storage-migrate-progress`）。 */
export function onStorageMigrateProgress(
  handler: (p: StorageMigrateProgress) => void,
): Promise<UnlistenFn> {
  return listen<StorageMigrateProgress>("storage-migrate-progress", (e) => handler(e.payload));
}

/** 数据目录已变更（`set_data_dir` / 迁移完成后 emit），订阅方应刷新缓存。 */
export function onStorageDirChanged(handler: () => void): Promise<UnlistenFn> {
  return listen<null>("storage-dir-changed", () => handler());
}

/**
 * 把本地绝对路径转成 Tauri asset 协议 URL，供 `<audio>` 播放合成结果等本机文件。
 *
 * 不能直接用 `@tauri-apps/api/core` 的 `convertFileSrc`：它用 `encodeURIComponent`
 * 编码整个路径（含 `/`），导致 URL 的 path 退化成单个段、没有目录结构。
 *
 * 这里改为逐段编码、保留 `/` 分隔符——Tauri 的 asset handler 会「skip leading /」，
 * 去掉一个 `/` 后得到的仍是绝对路径（如 `/Users/...`）。
 *
 * 平台差异（同 convertFileSrc 的规则）：
 * - Windows 的 WebView2 是 Chromium 内核，禁止对自定义 scheme 发跨源请求，
 *   必须用虚拟主机形式 `http://asset.localhost/<path>`（CSP 已放行该来源）；
 * - macOS/Linux 保持 `asset://localhost/<path>`。
 *
 * 另外 Tauri 返回的是原生路径：Windows 用 `\` 分隔，需先归一化为 `/`，
 * 否则整条路径会被编码成单个段（`%5C`）。
 */
export function toAssetUrl(path: string): string {
  const isWindows = navigator.userAgent.includes("Windows");
  const segments = path
    .replace(/\\/g, "/")
    .split("/")
    .map((s) => encodeURIComponent(s))
    .join("/");
  return isWindows ? `http://asset.localhost/${segments}` : `asset://localhost/${segments}`;
}
