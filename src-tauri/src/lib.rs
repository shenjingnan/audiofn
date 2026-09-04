//! ZapMomo 桌面应用（Tauri 2）。
//!
//! 复用根 crate `zapmomo` 的音频 / 配置逻辑：通过 Tauri command 暴露设备列表、
//! ASR / TTS 配置与模型库管理；识别/合成在独立线程执行，进度与结果经
//! `asr-*` / `tts-*` 事件推给前端。
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
#[cfg(target_os = "macos")]
use tauri::menu::PredefinedMenuItem;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use zapmomo::asr::config::AsrParamsPatch;
use zapmomo::asr::{AsrReaction, AsrResult, ReactionOutcome};
use zapmomo::config::settings::{
    self, AsrSettings, ChatboxSettings, CompanionWindowPosition, TtsSettings,
};
use zapmomo::model_library;
use zapmomo::model_library::{
    InstallState as LibInstallState, LibraryModel, RuntimeAction as LibRuntimeAction,
    SetCurrentResult, SystemResources, registry::ModelType as LibModelType,
    storage::StorageInfoView,
};
use zapmomo::tts::config::TtsParamsPatch;

/// `download_tts_model` 缺省安装的模型库条目（Qwen3-TTS 0.6B，延迟优先档）。
const DEFAULT_TTS_REGISTRY_ID: &str = "tts-qwen3-06b-base-q8-audiocpp";

// 文字输入条的 macOS 非激活面板：聚焦输入框不需要激活整个 App（Spotlight 式），
// 因此「显隐快捷键呼出输入条并聚焦」不会把本应用其它可见窗口（如设置窗）一并
// 带到最前。可以成为 key window 以接收键盘与中文 IME 输入，但永不成为 main window。
// 宏展开含 use 声明，须包在独立模块内。
#[cfg(target_os = "macos")]
mod chatbox_panel {
    // 宏生成代码需调用 WebviewWindow::app_handle()（Manager trait 方法）
    use tauri::Manager as _;

    tauri_nspanel::tauri_panel! {
        panel!(ChatboxPanel {
            config: {
                is_floating_panel: true,
                can_become_key_window: true,
                can_become_main_window: false,
            }
        })
    }
}
#[cfg(target_os = "macos")]
use chatbox_panel::ChatboxPanel;

/// RAII：进入监听时置 `active_model_dir`，无论正常/错误/panic 退出监听线程都会清空。
struct ActiveModelGuard {
    target: Arc<Mutex<Option<PathBuf>>>,
}

impl ActiveModelGuard {
    fn set(target: &Arc<Mutex<Option<PathBuf>>>, path: PathBuf) -> Self {
        *target.lock().unwrap_or_else(|e| e.into_inner()) = Some(path);
        Self {
            target: target.clone(),
        }
    }
}

impl Drop for ActiveModelGuard {
    fn drop(&mut self) {
        *self.target.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// 下载进度事件载荷（推给前端）。
#[derive(Clone, Serialize)]
struct DownloadProgressPayload {
    stage: String,
    percent: f64,
    message: String,
}

/// 退出作用域（含 panic / 命令取消）时复位下载标志。
struct ResetOnDrop(Arc<AtomicBool>);

impl Drop for ResetOnDrop {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// 监听结束事件载荷（正常停止时 `error` 为 `None`）。
#[derive(Clone, Serialize)]
struct ListenStopped {
    error: Option<String>,
}

#[derive(Serialize)]
struct AppInfo {
    version: String,
    product_name: String,
}

#[tauri::command]
fn get_app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        product_name: env!("CARGO_PKG_NAME").to_string(),
    }
}

/// 列出可用麦克风输入设备。
#[tauri::command]
fn list_devices() -> Vec<String> {
    zapmomo::audio::list_input_devices()
}

/// 请求 macOS 麦克风授权（触发系统授权弹窗）。返回是否已授权。
///
/// macOS 未授权时输入设备被系统隐藏、枚举为空，需先经此授权恢复；
/// 调试模式下每次重新编译授权会失效，前端在设备列表为空时引导用户点击。
#[tauri::command]
fn request_mic_permission() -> Result<bool, String> {
    zapmomo::audio::request_mic_permission()
}

/// ASR 模型下载状态：防重入标志。
struct AsrDownloadState {
    in_progress: Arc<AtomicBool>,
}

impl Default for AsrDownloadState {
    fn default() -> Self {
        Self {
            in_progress: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// 离线听写线程状态：共享停止标志 + 线程句柄 + 当前使用的模型目录。
struct AsrDictateState {
    running: Arc<AtomicBool>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    /// 当前会话真正使用的模型目录（RuntimeActual）
    active_model_dir: Arc<Mutex<Option<PathBuf>>>,
}

impl AsrDictateState {
    fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
            active_model_dir: Arc::new(Mutex::new(None)),
        }
    }

    fn is_dictating(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn active_model_dir(&self) -> Option<PathBuf> {
        self.active_model_dir.lock().ok().and_then(|g| g.clone())
    }
}

/// 把听写结果（每段整句）通过 Tauri 事件发给前端。
struct TauriAsrDictateReaction {
    app: AppHandle,
}

impl AsrReaction for TauriAsrDictateReaction {
    fn on_result(&mut self, result: &AsrResult) -> ReactionOutcome {
        let _ = self.app.emit("asr-dictate-result", result);
        ReactionOutcome::Continue
    }
}

/// GUI 展示用的 ASR 配置信息（含可经 `set_asr_params` 调整的引擎参数）。
#[derive(Serialize)]
struct AsrConfigInfo {
    enabled: bool,
    /// 模型类型（zipformer/paraformer/sensevoice/whisper），前端据此隐藏流式专属参数
    model_type: String,
    /// 推理后端（sherpa/audiocpp），前端据此显示 audio.cpp 标识与隐藏热词参数
    backend: String,
    model_dir: String,
    provider: String,
    num_threads: i32,
    sample_rate: i32,
    chunk_size: usize,
    decoding_method: String,
    enable_endpoint: bool,
    rule1_min_trailing_silence: f32,
    rule2_min_trailing_silence: f32,
    rule3_min_utterance_length: f32,
    blank_penalty: f32,
    hotwords: Option<String>,
    enable_punctuation: bool,
    debug: bool,
    models_present: bool,
    punctuation_present: bool,
    /// Silero VAD 模型是否已就绪（离线听写首次启动会自动下载）
    vad_present: bool,
    model_downloading: bool,
    settings_path: String,
}

/// 读取合并后的 ASR 配置（settings.toml + 默认值），并给出模型是否就绪。
#[tauri::command]
fn get_asr_config(state: State<'_, AsrDownloadState>) -> Result<AsrConfigInfo, String> {
    let settings = zapmomo::config::settings::load_settings()?;
    let asr_settings = settings.as_ref().and_then(|s| s.asr.clone());
    let cfg = zapmomo::asr::config::resolve(asr_settings.as_ref(), None)?;

    // 族 + 后端感知：sherpa 按模型类型清单探测；audiocpp 按族表 GGUF 单文件探测
    let models_present = zapmomo::asr::config::models_present(&cfg);
    let punctuation_present = cfg.punctuation_model.is_file();
    tracing::info!(
        "get_asr_config: model_type={} backend={} settings.asr.enabled={:?} resolve.enabled={} models_present={}",
        cfg.model_type.as_str(),
        cfg.backend.as_str(),
        asr_settings.as_ref().and_then(|a| a.enabled),
        cfg.enabled,
        models_present
    );

    Ok(AsrConfigInfo {
        enabled: cfg.enabled,
        model_type: cfg.model_type.as_str().to_string(),
        backend: cfg.backend.as_str().to_string(),
        model_dir: cfg.model_dir.display().to_string(),
        provider: cfg.provider.clone(),
        num_threads: cfg.num_threads,
        sample_rate: cfg.sample_rate,
        chunk_size: cfg.chunk_size,
        decoding_method: cfg.decoding_method.clone(),
        enable_endpoint: cfg.enable_endpoint,
        rule1_min_trailing_silence: cfg.rule1_min_trailing_silence,
        rule2_min_trailing_silence: cfg.rule2_min_trailing_silence,
        rule3_min_utterance_length: cfg.rule3_min_utterance_length,
        blank_penalty: cfg.blank_penalty,
        hotwords: cfg.hotwords.clone(),
        enable_punctuation: cfg.enable_punctuation,
        debug: cfg.debug,
        models_present,
        punctuation_present,
        vad_present: zapmomo::asr::dictate::vad_model_present(),
        model_downloading: state.in_progress.load(Ordering::Relaxed),
        settings_path: zapmomo::config::settings::get_settings_path()
            .display()
            .to_string(),
    })
}

/// 一键离线转写的结果（snake_case 直传前端，与 AsrConfigInfo 同款）。
#[derive(Serialize)]
struct TranscribeResult {
    text: String,
    model_type: String,
    model_dir: String,
}

/// 一键离线转写 wav 文件（走 audiocpp qwen3_asr，经 `asr::transcribe_wav`）。
///
/// `wav_path` 为 None 时转写模型自带的 `test_wavs/` 示例音频（「测试识别」）；
/// 阻塞线程池执行避免卡 UI。
#[tauri::command]
async fn transcribe_audio(wav_path: Option<String>) -> Result<TranscribeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = zapmomo::config::settings::load_settings()?;
        let asr_settings = settings.as_ref().and_then(|s| s.asr.clone());
        let cfg = zapmomo::asr::config::resolve(asr_settings.as_ref(), None)?;
        let wav_path = wav_path
            .map(std::path::PathBuf::from)
            .or_else(|| zapmomo::asr::default_test_wav(&cfg.model_dir))
            .ok_or_else(|| "未指定音频路径，且模型目录没有 test_wavs/*.wav 示例音频".to_string())?;
        let text = zapmomo::asr::transcribe_wav(&cfg, &wav_path)?;
        Ok(TranscribeResult {
            text,
            model_type: cfg.model_type.as_str().to_string(),
            model_dir: cfg.model_dir.display().to_string(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 开始离线免提听写的内部实现（command 与「切换设备重启」共用）。
///
/// 守卫：仅在离线模型（非 zipformer/paraformer 流式族）下可用。
/// 线程内先惰性下载 Silero VAD 模型，再跑 `run_dictate`（VAD 分段 → 每段整句送 audiocpp qwen3_asr）。
fn start_asr_dictate_impl(
    app: AppHandle,
    state: &AsrDictateState,
    device: Option<String>,
) -> Result<(), String> {
    if state.is_dictating() {
        return Err("已在听写中".to_string());
    }

    let settings = zapmomo::config::settings::load_settings()?;
    let asr_settings = settings.as_ref().and_then(|s| s.asr.clone());
    let cfg = zapmomo::asr::config::resolve(asr_settings.as_ref(), None)?;

    // 流式模型不支持听写（离线模型专用）：前端已切走开关，这里双保险拦截
    if cfg.model_type.is_streaming() {
        return Err(format!(
            "当前模型类型 {} 不支持免提听写（离线模型专用）。请先切换 SenseVoice/Whisper/Qwen3-ASR 离线模型。",
            cfg.model_type.as_str()
        ));
    }

    // ASR 就绪预检（backend 感知：audiocpp 按 GGUF 单文件校验），避免在后台线程里才报错
    zapmomo::asr::config::preflight(&cfg)?;

    let running = state.running.clone();
    running.store(true, Ordering::Relaxed);
    // RuntimeActual：记录本次听写使用的模型目录；随线程退出自动清空
    let _active_guard = ActiveModelGuard::set(&state.active_model_dir, cfg.model_dir.clone());
    let thread_app = app.clone();
    let handle = std::thread::spawn(move || {
        let _active = _active_guard;
        tracing::info!("ASR dictate thread started");
        let mut reaction = TauriAsrDictateReaction {
            app: thread_app.clone(),
        };

        // 首次听写自动下载 Silero VAD 模型（~0.6MB，幂等）；失败则停止并报错
        let result = {
            let mut progress = |p: zapmomo::model_library::asset::DownloadProgress| {
                let stage = match p.stage {
                    zapmomo::model_library::asset::DownloadStage::Downloading => "downloading",
                    zapmomo::model_library::asset::DownloadStage::Verifying => "verifying",
                    zapmomo::model_library::asset::DownloadStage::Extracting => "extracting",
                    zapmomo::model_library::asset::DownloadStage::Done => "done",
                };
                let _ = thread_app.emit(
                    "asr-vad-download-progress",
                    DownloadProgressPayload {
                        stage: stage.to_string(),
                        percent: p.percent,
                        message: p.message,
                    },
                );
            };
            match zapmomo::asr::dictate::ensure_vad_model(&mut progress) {
                Ok(vad_path) => {
                    let vad_cfg =
                        zapmomo::asr::dictate::DictateConfig::new(vad_path).with_runtime(&cfg);
                    zapmomo::asr::dictate::run_dictate(
                        &cfg,
                        &vad_cfg,
                        device.as_deref(),
                        None,
                        &mut reaction,
                        Some(&running),
                    )
                }
                Err(e) => Err(e),
            }
        };

        running.store(false, Ordering::Relaxed);
        match &result {
            Ok(()) => tracing::info!("ASR dictate thread finished (clean)"),
            Err(e) => tracing::error!("ASR dictate thread finished with error: {e}"),
        }
        let payload = ListenStopped {
            error: result.err(),
        };
        let _ = reaction.app.emit("asr-dictate-stopped", payload);
    });
    *state
        .handle
        .lock()
        .expect("asr dictate handle lock poisoned") = Some(handle);
    // 通知前端听写已启动（含切换设备后的自动重启；启动瞬间前端未订阅时静默丢弃）
    let _ = app.emit("asr-dictate-started", ListenStopped { error: None });
    Ok(())
}

/// 开始离线免提听写。 —— Tauri command 外壳。
#[tauri::command]
fn start_asr_dictate(
    app: AppHandle,
    state: State<'_, AsrDictateState>,
    device: Option<String>,
) -> Result<(), String> {
    start_asr_dictate_impl(app, state.inner(), device)
}

/// 停止离线听写的内部实现（command 与「切换设备重启」共用）。
fn stop_asr_dictate_inner(state: &AsrDictateState) -> Result<(), String> {
    if !state.is_dictating() {
        return Err("当前没有在听写".to_string());
    }
    state.running.store(false, Ordering::Relaxed);
    let handle = state
        .handle
        .lock()
        .expect("asr dictate handle lock poisoned")
        .take();
    if let Some(handle) = handle {
        let _ = handle.join();
    }
    *state
        .active_model_dir
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}

/// 停止离线听写：置停止标志并等待线程退出。
#[tauri::command]
fn stop_asr_dictate(state: State<'_, AsrDictateState>) -> Result<(), String> {
    stop_asr_dictate_inner(state.inner())
}

/// 当前是否正在离线听写。
#[tauri::command]
fn is_asr_dictating(state: State<'_, AsrDictateState>) -> bool {
    state.is_dictating()
}

/// 下载并安装 ASR 模型（默认安装到 `~/.zapmomo/models/<模型名>`）。
///
/// 防重入；下载在阻塞线程池执行，进度经 `asr-model-download-progress` 事件推给前端。
#[tauri::command]
async fn download_asr_model(
    app: AppHandle,
    state: State<'_, AsrDownloadState>,
) -> Result<(), String> {
    let flag = state.in_progress.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return Err("模型下载已在进行中，请稍候".to_string());
    }
    let dest = zapmomo::asr::user_model_dir();
    let punct_dest = zapmomo::asr::punctuation_user_model_dir();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = ResetOnDrop(flag);
        let mut progress = |p: zapmomo::asr::DownloadProgress| {
            let stage = match p.stage {
                zapmomo::asr::DownloadStage::Downloading => "downloading",
                zapmomo::asr::DownloadStage::Verifying => "verifying",
                zapmomo::asr::DownloadStage::Extracting => "extracting",
                zapmomo::asr::DownloadStage::Done => "done",
            };
            let _ = app.emit(
                "asr-model-download-progress",
                DownloadProgressPayload {
                    stage: stage.to_string(),
                    percent: p.percent,
                    message: p.message,
                },
            );
        };
        zapmomo::asr::install_model_to(&dest, false, &mut progress).map_err(|e| e.to_string())?;
        // 顺带安装标点模型（自动开启）；失败仅警告，不阻断 ASR 下载成功。
        if let Err(e) =
            zapmomo::asr::install_punctuation_model_to(&punct_dest, false, &mut progress)
        {
            tracing::warn!("标点模型安装失败（ASR 仍可用，仅无标点）: {e}");
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("下载任务异常: {e}"))?
}

/// TTS 合成线程状态：共享 busy 标志 + 线程句柄。
struct TtsSynthesizeState {
    busy: Arc<AtomicBool>,
    handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl TtsSynthesizeState {
    fn new() -> Self {
        Self {
            busy: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
        }
    }

    fn is_synthesizing(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }
}

/// TTS 模型下载状态：防重入标志。
struct TtsDownloadState {
    in_progress: Arc<AtomicBool>,
}

impl Default for TtsDownloadState {
    fn default() -> Self {
        Self {
            in_progress: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// GUI 展示用的 TTS 配置信息。
#[derive(Serialize)]
struct TtsConfigInfo {
    /// 模型类型（qwen3_tts_06/qwen3_tts_17），前端据此切换音色语义
    model_type: String,
    /// 推理后端（sherpa/audiocpp），前端据此显示引擎徽标
    backend: String,
    model_dir: String,
    provider: String,
    num_threads: i32,
    enabled: bool,
    models_present: bool,
    model_downloading: bool,
    settings_path: String,
    /// 扩散解码步数（质量/速度权衡），可经 `set_tts_params` 修改
    num_steps: i32,
    /// 默认语速，可经 `set_tts_params` 修改
    speed: f32,
    /// 调试输出，可经 `set_tts_params` 修改
    debug: bool,
    /// 默认音色 id（`None` = 用内置 leijun），可经 `set_tts_voice` 修改
    voice: Option<String>,
}

/// 合成结果事件载荷（推给前端播放）。
#[derive(Clone, Serialize)]
struct TtsResult {
    path: String,
    duration: f32,
    sample_rate: i32,
}

/// 读取合并后的 TTS 配置（settings.toml + 默认值），并给出模型是否就绪。
#[tauri::command]
fn get_tts_config(state: State<'_, TtsDownloadState>) -> Result<TtsConfigInfo, String> {
    let settings = zapmomo::config::settings::load_settings()?;
    let tts_settings = settings.as_ref().and_then(|s| s.tts.clone());
    let cfg = zapmomo::tts::config::resolve(tts_settings.as_ref(), None)?;

    let models_present = zapmomo::tts::config::models_present(&cfg);

    Ok(TtsConfigInfo {
        model_type: cfg.model_type.as_str().to_string(),
        backend: cfg.backend.as_str().to_string(),
        model_dir: cfg.model_dir.display().to_string(),
        provider: cfg.provider.clone(),
        num_threads: cfg.num_threads,
        enabled: cfg.enabled,
        models_present,
        model_downloading: state.in_progress.load(Ordering::Relaxed),
        settings_path: zapmomo::config::settings::get_settings_path()
            .display()
            .to_string(),
        num_steps: cfg.num_steps,
        speed: cfg.speed,
        debug: cfg.debug,
        voice: cfg.voice.clone(),
    })
}

/// 在后台线程内合成文本，期间发 `tts-progress`，完成后发 `tts-result`。
fn synthesize_inner(
    app: &AppHandle,
    cfg: &zapmomo::tts::config::ResolvedTtsConfig,
    text: &str,
    speed: f32,
    voice: &zapmomo::tts::TtsVoiceParams,
) -> Result<(), String> {
    let engine = zapmomo::tts::TtsEngine::new(cfg.clone())?;
    let out_dir = zapmomo::config::settings::get_tts_output_dir();
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("创建输出目录失败: {e}"))?;
    // 放行 asset 协议 scope，前端 <audio> 才能通过 asset:// 播放生成的 wav。
    let _ = app.asset_protocol_scope().allow_directory(&out_dir, true);
    let out_path = zapmomo::tts::default_output_path();

    let progress_app = app.clone();
    let sample_count =
        engine.synthesize_to_wav_with_progress(text, speed, voice, &out_path, move |p| {
            let _ = progress_app.emit(
                "tts-progress",
                zapmomo::tts::reaction::TtsProgress { percent: p },
            );
            true
        })?;

    let sample_rate = engine.sample_rate();
    let duration = sample_count as f32 / sample_rate as f32;
    let _ = app.emit(
        "tts-result",
        TtsResult {
            path: out_path.display().to_string(),
            duration,
            sample_rate,
        },
    );
    Ok(())
}

/// 列出可用音色：参考音频克隆模型返回参考音色（模型包内置 + 用户自定义音色库）。
#[tauri::command]
fn list_tts_voices() -> Result<Vec<zapmomo::tts::TtsVoice>, String> {
    let settings = zapmomo::config::settings::load_settings()?;
    let tts_settings = settings.as_ref().and_then(|s| s.tts.clone());
    let cfg = zapmomo::tts::config::resolve(tts_settings.as_ref(), None)?;
    let mut voices = zapmomo::tts::voice::list_builtin_voices(&cfg.model_dir);
    voices.extend(zapmomo::tts::voice_store::list_custom_voices());
    Ok(voices)
}

/// 保存一个自定义音色：把源 wav 拷贝到音色库并登记（命名 + 参考转写文本）。
#[tauri::command]
fn save_tts_voice(
    name: String,
    source_wav_path: String,
    reference_text: String,
) -> Result<zapmomo::tts::TtsVoice, String> {
    zapmomo::tts::voice_store::save_voice(
        &name,
        std::path::Path::new(&source_wav_path),
        &reference_text,
    )
}

/// 删除一个自定义音色（清单 + wav 文件）。
#[tauri::command]
fn delete_tts_voice(id: String) -> Result<(), String> {
    zapmomo::tts::voice_store::delete_voice(&id)?;
    Ok(())
}

/// 列出音色库全部自定义音色（模型无关，供伙伴页音色绑定选择器）。
///
/// 与 `list_tts_voices` 的区别：后者按当前 TTS 模型过滤（非克隆模型返回空、
/// 含 builtin 条目），绑定是持久化元数据、跨模型有效，必须看到全量音色库。
#[tauri::command]
fn list_voice_library() -> Result<Vec<zapmomo::tts::TtsVoice>, String> {
    Ok(zapmomo::tts::voice_store::list_custom_voices())
}

/// 录制 N 秒麦克风并保存为 16k wav，返回 wav 路径（供后续保存为音色）。
#[tauri::command]
async fn record_tts_voice(seconds: u32, device: Option<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        zapmomo::audio::record_voice(seconds, device.as_deref()).map(|p| p.display().to_string())
    })
    .await
    .map_err(|e| format!("录音任务异常: {e}"))?
}

/// 用 ASR 离线转写参考音频，返回带标点的转写文本（供自定义音色自动填充）。
///
/// 依赖 ASR 模型（含标点模型）已下载；转写在阻塞线程池执行，避免卡住 UI。
#[tauri::command]
async fn transcribe_reference_audio(wav_path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = zapmomo::config::settings::load_settings()?;
        let asr_settings = settings.as_ref().and_then(|s| s.asr.clone());
        let cfg = zapmomo::asr::config::resolve(asr_settings.as_ref(), None)?;
        zapmomo::asr::transcribe_wav(&cfg, Path::new(&wav_path))
    })
    .await
    .map_err(|e| format!("转写任务异常: {e}"))?
}

/// 把文本合成为语音并写入 wav（后台线程执行）。
///
/// 校验模型文件后启动独立线程合成，进度经 `tts-progress` 事件推给前端；
/// 完成后发 `tts-result`（含 wav 路径），线程末发 `tts-stopped`。
#[tauri::command]
fn synthesize_tts(
    app: AppHandle,
    state: State<'_, TtsSynthesizeState>,
    text: String,
    speed: Option<f32>,
    voice: Option<String>,
    reference_wav: Option<String>,
    reference_text: Option<String>,
) -> Result<(), String> {
    if state.is_synthesizing() {
        return Err("正在合成中".to_string());
    }
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("文本不能为空".to_string());
    }

    let settings = zapmomo::config::settings::load_settings()?;
    let tts_settings = settings.as_ref().and_then(|s| s.tts.clone());
    let cfg = zapmomo::tts::config::resolve(tts_settings.as_ref(), None)?;

    // 启用门控：关闭时直接返回错误，前端据此禁用合成。
    if !cfg.enabled {
        return Err("语音合成已禁用，请在「模型与能力」中开启语音合成。".to_string());
    }

    // 预检模型文件（backend 感知：sherpa 按模型类型清单、audiocpp 按固定两文件），
    // 失败同步返回清晰错误（避免在后台线程里才报错）
    zapmomo::tts::config::preflight(&cfg).map_err(|e| {
        format!(
            "{e}\n\n请在「配置」面板点击「选择模型」，或运行 `zapmomo tts install-model` 下载模型。"
        )
    })?;

    // 合成音色参数统一解析（见 zapmomo::tts::voice）。用户显式参数
    // （音色/自定义参考音频）优先；在后台线程外解析，尽早报错。
    let custom_wav = reference_wav.map(std::path::PathBuf::from);
    let voice_params = zapmomo::tts::voice::resolve_voice_params(
        &cfg,
        voice.as_deref(),
        custom_wav.as_deref(),
        reference_text.as_deref(),
    )?;

    let speed = speed.unwrap_or(cfg.speed);

    let busy = state.busy.clone();
    busy.store(true, Ordering::Relaxed);
    let thread_app = app.clone();
    let handle = std::thread::spawn(move || {
        tracing::info!("TTS synthesize thread started");
        let result = synthesize_inner(&thread_app, &cfg, &text, speed, &voice_params);
        busy.store(false, Ordering::Relaxed);
        match &result {
            Ok(()) => tracing::info!("TTS synthesize thread finished (clean)"),
            Err(e) => tracing::error!("TTS synthesize thread finished with error: {e}"),
        }
        let payload = ListenStopped {
            error: result.err(),
        };
        let _ = thread_app.emit("tts-stopped", payload);
    });
    *state.handle.lock().expect("tts handle lock poisoned") = Some(handle);
    Ok(())
}

/// 停止 TTS 合成/播放的内部实现（command 与全局快捷键打断共用）。
fn stop_tts_inner(state: &TtsSynthesizeState) -> Result<(), String> {
    if !state.is_synthesizing() {
        return Err("当前没有在合成".to_string());
    }
    state.busy.store(false, Ordering::Relaxed);
    let handle = state
        .handle
        .lock()
        .expect("tts handle lock poisoned")
        .take();
    if let Some(handle) = handle {
        let _ = handle.join();
    }
    Ok(())
}

/// 停止正在进行的合成（等待线程退出）。
#[tauri::command]
fn stop_tts(state: State<'_, TtsSynthesizeState>) -> Result<(), String> {
    stop_tts_inner(state.inner())
}

/// 当前是否正在合成。
#[tauri::command]
fn is_tts_synthesizing(state: State<'_, TtsSynthesizeState>) -> bool {
    state.is_synthesizing()
}

/// 下载并安装 TTS 模型（缺省 Qwen3-TTS 0.6B，`~/.zapmomo/models/<模型名>`）。
///
/// 防重入；下载在阻塞线程池执行，进度经 `tts-model-download-progress` 事件推给前端。
#[tauri::command]
async fn download_tts_model(
    app: AppHandle,
    state: State<'_, TtsDownloadState>,
) -> Result<(), String> {
    let flag = state.in_progress.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return Err("模型下载已在进行中，请稍候".to_string());
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = ResetOnDrop(flag);
        let model =
            zapmomo::model_library::registry::model_for_current_platform(DEFAULT_TTS_REGISTRY_ID)
                .ok_or_else(|| format!("未知的模型库条目: {DEFAULT_TTS_REGISTRY_ID}"))?;
        let mut progress = |p: zapmomo::model_library::asset::DownloadProgress| {
            let stage = match p.stage {
                zapmomo::model_library::asset::DownloadStage::Downloading => "downloading",
                zapmomo::model_library::asset::DownloadStage::Verifying => "verifying",
                zapmomo::model_library::asset::DownloadStage::Extracting => "extracting",
                zapmomo::model_library::asset::DownloadStage::Done => "done",
            };
            let _ = app.emit(
                "tts-model-download-progress",
                DownloadProgressPayload {
                    stage: stage.to_string(),
                    percent: p.percent,
                    message: p.message,
                },
            );
        };
        zapmomo::model_library::install_managed_model(model, &mut progress, None)
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| format!("下载任务异常: {e}"))?
}

/// 持久化「是否启用语音合成」，写入 `[tts].enabled`（缺省 true）。
#[tauri::command]
fn set_tts_enabled(enabled: bool) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let tts = settings.tts.get_or_insert_with(TtsSettings::default);
    tts.enabled = Some(enabled);
    settings::save_settings(&settings)?;
    Ok(())
}

/// 批量持久化 TTS 合成参数（扩散步数/默认语速/线程/调试），写入 `[tts]`。
///
/// 载荷为 `{ params: { num_steps, speed, ... } }`（snake_case 直传）；
/// `None` 字段保持原有配置不变。值先整体校验、再写入，出错时不部分修改。
/// 引擎在每次合成时新建，因此保存后下一次合成即生效，无需重启。
#[tauri::command]
fn set_tts_params(params: TtsParamsPatch) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let tts = settings.tts.get_or_insert_with(TtsSettings::default);
    params.apply_to(tts)?;
    settings::save_settings(&settings)
}

/// 设定默认音色（写入 `[tts].voice`；`None` 恢复内置默认 leijun）。
///
/// 所有不显式指定音色的合成（测试语音 / 语音会话）都会用该默认音色，
/// 经 `resolve_reference` 回退生效。保存后下一次合成即生效，无需重启。
#[tauri::command]
fn set_tts_voice(voice: Option<String>) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let tts = settings.tts.get_or_insert_with(TtsSettings::default);
    tts.voice = voice;
    settings::save_settings(&settings)
}

/// 切换 TTS 推理后端（写入 `[tts].backend`）。高级/测试入口：常规入口是模型库
/// 「设为当前」（`set_selected_model` 按 registry runtime 同步写入）。
///
/// 切后端时同步重置 `model_type` 交回 resolve 目录探测（旧 kind 属于另一后端的
/// 模型），并在切回 sherpa 时复位 backend 覆盖。保存后下一次合成即生效。
#[tauri::command]
fn set_tts_backend(backend: String) -> Result<(), String> {
    let kind = zapmomo::tts::config::TtsBackendKind::parse_str(&backend)
        .ok_or_else(|| format!("未知 TTS 后端: {backend}（支持 sherpa / audiocpp）"))?;
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let tts = settings.tts.get_or_insert_with(TtsSettings::default);
    tts.backend = Some(kind.as_str().to_string());
    // 旧 model_type 属于另一后端的模型，交回 resolve 按目录探测
    tts.model_type = None;
    settings::save_settings(&settings)
}

/// 持久化 ASR 启用状态，写入 `[asr].enabled`（语音会话「能识别」的前提）。
#[tauri::command]
fn set_asr_enabled(enabled: bool) -> Result<(), String> {
    tracing::info!("set_asr_enabled 命令被调用: enabled={enabled}");
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let asr = settings.asr.get_or_insert_with(AsrSettings::default);
    asr.enabled = Some(enabled);
    settings::save_settings(&settings)?;
    tracing::info!(
        "set_asr_enabled 已保存，[asr].enabled={:?}",
        settings.asr.as_ref().and_then(|a| a.enabled)
    );
    Ok(())
}

/// 持久化 ASR 引擎/运行参数（线程/块大小/断句/热词/标点/调试），写入 `[asr]`。
/// 引擎参数在启动识别时固化：修改后需重启识别才生效（由前端在保存后若在识别则重启）。
#[tauri::command]
fn set_asr_params(params: AsrParamsPatch) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let asr = settings.asr.get_or_insert_with(AsrSettings::default);
    params.apply_to(asr)?;
    settings::save_settings(&settings)
}

/// 读取全局默认麦克风输入设备名（空串 = 系统默认），KWS / ASR 共用。
#[tauri::command]
fn get_microphone() -> Result<String, String> {
    Ok(settings::load_settings()?
        .and_then(|s| s.microphone)
        .unwrap_or_default())
}

/// 设置并持久化全局默认麦克风（空串 → None = 系统默认）。
///
/// 若离线听写正在运行，用新设备自动重启监听，使切换立即生效；
/// 重启失败（如新设备不可用）返回错误，已停止的任务保持停止。
#[tauri::command]
fn set_microphone(
    app: AppHandle,
    asr_dictate: State<'_, AsrDictateState>,
    mic: String,
) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    settings.microphone = if mic.trim().is_empty() {
        None
    } else {
        Some(mic.trim().to_string())
    };
    settings::save_settings(&settings)?;

    let new_mic = settings.microphone.clone();

    // 离线听写运行中 → 用新设备重启。
    if asr_dictate.is_dictating() {
        stop_asr_dictate_inner(asr_dictate.inner())?;
        start_asr_dictate_impl(app.clone(), asr_dictate.inner(), new_mic.clone())?;
    }
    Ok(())
}

/// 持久化文字输入条窗口位置（逻辑像素），供下次启动恢复。
///
/// 由前端在用户手动拖动窗口后（debounce）调用，写入 `~/.zapmomo/settings.toml`
/// 的 `[chatbox.window_position]` 段。
#[tauri::command]
fn save_chatbox_position(x: i32, y: i32) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    let chatbox = settings
        .chatbox
        .get_or_insert_with(ChatboxSettings::default);
    chatbox.window_position = Some(CompanionWindowPosition { x, y });
    settings::save_settings(&settings)
}

/// 隐藏文字输入条窗口并持久化开关（前端 Esc 关闭时调用，保持菜单勾选态一致）。
#[tauri::command]
fn hide_chatbox(app: AppHandle) {
    set_chatbox_visible(&app, false, false);
}

/// 显示/隐藏文字输入条窗口并持久化开关状态（托盘/右键菜单勾选共用）。
///
/// `focus` 仅显示时生效（显示后可直接打字）。macOS 上 chatbox 是非激活面板，
/// 显示与聚焦走 NSPanel 专用 API（`orderFrontRegardless` + `makeKeyWindow`）：
/// 聚焦输入不激活应用，不会把本应用其它可见窗口（如设置窗）一并带到最前；
/// 其它平台为普通窗口，聚焦会激活应用。
fn set_chatbox_visible(app: &AppHandle, visible: bool, focus: bool) {
    if let Some(window) = app.get_webview_window("chatbox") {
        // macOS：tao 的 window.show()/set_focus() 底层是 makeKeyAndOrderFront /
        // activateIgnoringOtherApps——后者无条件激活整个 App，AppKit 激活时会把本应用
        // 全部可见窗口（如一直开着的设置窗）整体带到最前，显隐快捷键因此表现为
        // 「连带弹出主界面」。NonactivatingPanel mask 只抑制用户点击面板时的隐式
        // 激活，管不住显式 activate 调用。改走 panel 的 orderFrontRegardless +
        // makeKeyWindow：输入条成为 key window 可直接打字，但 App 不激活（Spotlight 式）。
        #[cfg(target_os = "macos")]
        {
            use tauri_nspanel::ManagerExt;
            match app.get_webview_panel("chatbox") {
                Ok(panel) => {
                    if visible {
                        if focus {
                            panel.show_and_make_key();
                        } else {
                            panel.show();
                        }
                    } else {
                        panel.hide();
                    }
                }
                // panel 注册前的时序兜底（正常运行不可达）：退回 tauri 原路径
                Err(_) => {
                    let _ = if visible {
                        window.show()
                    } else {
                        window.hide()
                    };
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = if visible && focus {
                window.show().and_then(|()| window.set_focus())
            } else if visible {
                window.show()
            } else {
                window.hide()
            };
        }
    }
    if let Ok(mut settings) = settings::load_settings() {
        let settings = settings.get_or_insert_with(Default::default);
        let chatbox = settings
            .chatbox
            .get_or_insert_with(ChatboxSettings::default);
        chatbox.visible = Some(visible);
        let _ = settings::save_settings(settings);
    }
    rebuild_tray_menu(app);
}

/// 读取是否在 macOS Dock / Cmd+Tab 中隐藏应用图标（Accessory 模式）。
#[tauri::command]
fn get_hide_dock_icon() -> Result<bool, String> {
    Ok(settings::load_settings()?
        .unwrap_or_default()
        .hide_dock_icon)
}

/// 设置并持久化是否在 macOS Dock / Cmd+Tab 中隐藏应用图标，并立即生效。
///
/// 写入 `~/.zapmomo/settings.toml` 顶层的 `hide_dock_icon` 字段；非 macOS 仅持久化，
/// 不改变激活策略（该设置仅对 macOS 的 Dock / Cmd+Tab 有意义）。
///
/// `app` 仅在 macOS 上用于切换 ActivationPolicy，其它平台未使用，故非 macOS 允许未使用变量。
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
#[tauri::command]
fn set_hide_dock_icon(app: AppHandle, hide: bool) -> Result<(), String> {
    let mut settings = settings::load_settings()?.unwrap_or_default();
    settings.hide_dock_icon = hide;
    settings::save_settings(&settings)?;
    #[cfg(target_os = "macos")]
    {
        let policy = if hide {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        };
        app.set_activation_policy(policy)
            .map_err(|e| format!("切换激活策略失败: {e}"))?;
    }
    Ok(())
}

/// 自启动拉起检测：命令行精确携带 `--autostart`（开启自启动时由插件附加到
/// 系统启动项）。前缀/去杠变体（`--autostart-x`、`autostart`）不命中。
fn is_launched_by_autostart<I>(args: I) -> bool
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    args.into_iter().any(|a| a.as_ref() == "--autostart")
}

/// 自启动菜单项的 (id, 文案)：按当前状态显示相反动作，点击应用固定值（幂等）。
fn autostart_item_labels(enabled: bool) -> (&'static str, &'static str) {
    if enabled {
        ("disable_autostart", "关闭开机自启动")
    } else {
        ("enable_autostart", "开机自启动")
    }
}

/// 读当前开机自启动状态。
///
/// 注意：与 hide_dock_icon 等落盘开关不同，自启动是系统级注册（注册表 Run 键 /
/// LaunchAgent / XDG .desktop），不随应用退出消失，用户可在系统设置外部增删；
/// 系统状态即唯一真值，不在 settings.toml 落盘，读取直查插件（单次本地文件 /
/// 注册表检查，调用点仅 command 与托盘重建，无需缓存）。
fn current_autostart_enabled(app: &AppHandle) -> bool {
    // 函数内 use：避免与 tauri-nspanel 的同名 ManagerExt trait 冲突。
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// 设置并生效开机自启动（内部实现，供 command 与原生菜单事件共用）。
///
/// 注册/移除系统启动项后经 `autostart-changed` 事件通知设置页刷新开关，并重建
/// 托盘菜单翻转「开机自启动/关闭开机自启动」文案。
fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    if enabled {
        app.autolaunch()
            .enable()
            .map_err(|e| format!("开启开机自启动失败（写入系统启动项被拒）：{e}"))?;
    } else {
        app.autolaunch()
            .disable()
            .map_err(|e| format!("关闭开机自启动失败（移除系统启动项被拒）：{e}"))?;
    }
    let _ = app.emit("autostart-changed", enabled);
    rebuild_tray_menu(app);
    Ok(())
}

/// 读取是否开启开机自启动（系统注册状态直读）。
#[tauri::command]
fn get_autostart(app: AppHandle) -> Result<bool, String> {
    Ok(current_autostart_enabled(&app))
}

/// 设置开机自启动（设置页经此 command 间接操作插件，不暴露权限给前端）。
#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    apply_autostart(&app, enabled)
}

/// 处理应用菜单、托盘菜单与角色窗口右键菜单事件。
fn show_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 仅显示设置窗口、不抢键盘焦点（启动 2 秒后自动打开用）：自动弹出只为可发现性，
/// 键盘焦点应留给输入条（ChatboxBar 挂载即聚焦，见 set_chatbox_visible）。
///
/// macOS 不走 tao 的 `window.show()`——其底层 makeKeyAndOrderFront 会把 key window
/// 从输入条抢走；`orderFront:` 只调整 Z 序、不改变 key 归属。AppKit 调用须在主线程，
/// 经 `run_on_main_thread` 跳板（与 `rebuild_tray_menu_threadsafe` 同款）。
fn show_settings_window_unfocused(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = handle.get_webview_window("settings") {
            #[cfg(target_os = "macos")]
            {
                use objc2_app_kit::NSWindow;
                if let Ok(ns) = window.ns_window() {
                    // SAFETY: ns_window 返回 tauri 持有的合法 NSWindow 指针，
                    // 且本闭包经 run_on_main_thread 在主线程执行。
                    let ns = unsafe { &*(ns.cast::<NSWindow>()) };
                    ns.orderFront(None);
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = window.show();
            }
        }
    });
}

/// 打开设置窗口（供角色窗口右键菜单调用）。
#[tauri::command]
fn open_settings(app: AppHandle) {
    show_settings_window(&app);
}

/// 全局快捷键触发分发（复用托盘/菜单同款内部函数）。
fn dispatch_shortcut(app: &AppHandle, action: zapmomo::config::shortcuts::ShortcutAction) {
    use zapmomo::config::shortcuts::ShortcutAction;
    match action {
        ShortcutAction::OpenSettings => show_settings_window(app),
    }
}

/// 启动时按 `[shortcuts]` 配置注册全局快捷键：单个失败仅告警不阻塞启动
/// （键位可能已被其他软件占用），其余照常注册。
fn register_shortcuts_at_startup(app: &AppHandle) {
    use zapmomo::config::shortcuts::ShortcutAction;
    let shortcuts = settings::load_settings()
        .ok()
        .flatten()
        .and_then(|s| s.shortcuts)
        .unwrap_or_default();
    for action in ShortcutAction::ALL {
        let Some(acc) = shortcuts.get(action).map(str::to_string) else {
            continue;
        };
        let result = app
            .global_shortcut()
            .on_shortcut(acc.as_str(), move |app, _sc, ev| {
                // 插件在按下和松开各回调一次：只响应按下，否则一次按键切换两次
                // （表现为「按住消失、松开又出现」）
                if ev.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    dispatch_shortcut(app, action);
                }
            });
        match result {
            Ok(()) => tracing::info!("全局快捷键已注册：{} = {}", action.as_str(), acc),
            Err(e) => tracing::warn!(
                "全局快捷键 {} ({}) 注册失败，已跳过: {e}",
                action.as_str(),
                acc
            ),
        }
    }
}

/// 读取用户自定义快捷键（action 标识 → accelerator，仅含已绑定项）。
#[tauri::command]
fn get_shortcuts() -> Result<std::collections::HashMap<String, String>, String> {
    let shortcuts = settings::load_settings()?
        .unwrap_or_default()
        .shortcuts
        .unwrap_or_default();
    let mut map = std::collections::HashMap::new();
    for action in zapmomo::config::shortcuts::ShortcutAction::ALL {
        if let Some(acc) = shortcuts.get(action) {
            map.insert(action.as_str().to_string(), acc.to_string());
        }
    }
    Ok(map)
}

/// 绑定快捷键：校验 → 查重 → **先注册成功再落盘**（键位被系统/其他应用占用时
/// 注册失败，配置保持原值，杜绝「界面已绑定但实际不生效」的假状态）。
#[tauri::command]
fn set_shortcut(app: AppHandle, action: String, accelerator: String) -> Result<(), String> {
    use zapmomo::config::shortcuts::{ShortcutAction, validate_accelerator};
    let action =
        ShortcutAction::from_ident(&action).ok_or_else(|| format!("未知的操作：{action}"))?;
    let accelerator = accelerator.trim().to_string();
    validate_accelerator(&accelerator)?;

    let mut cfg = settings::load_settings()?.unwrap_or_default();
    let shortcuts = cfg.shortcuts.get_or_insert_with(Default::default);
    if let Some(other) = shortcuts.find_conflict(action, &accelerator) {
        return Err(format!("该快捷键已绑定到「{}」", other.label()));
    }
    // 幂等：与当前值相同直接成功
    if shortcuts.get(action) == Some(accelerator.as_str()) {
        return Ok(());
    }
    let old = shortcuts.get(action).map(str::to_string);
    app.global_shortcut()
        .on_shortcut(accelerator.as_str(), move |app, _sc, ev| {
            // 同启动注册路径：只响应按下，避免松开时二次触发
            if ev.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                dispatch_shortcut(app, action);
            }
        })
        .map_err(|e| format!("注册失败，可能已被其他应用占用：{e}"))?;
    // 新键注册成功后才解绑旧键
    if let Some(old) = old
        && let Err(e) = app.global_shortcut().unregister(old.as_str())
    {
        tracing::warn!("解绑旧快捷键 {old} 失败: {e}");
    }
    shortcuts.set(action, Some(accelerator));
    settings::save_settings(&cfg)?;
    Ok(())
}

/// 清除操作的快捷键绑定（解绑 + 配置置空）。
#[tauri::command]
fn clear_shortcut(app: AppHandle, action: String) -> Result<(), String> {
    use zapmomo::config::shortcuts::ShortcutAction;
    let action =
        ShortcutAction::from_ident(&action).ok_or_else(|| format!("未知的操作：{action}"))?;
    let mut cfg = settings::load_settings()?.unwrap_or_default();
    if let Some(shortcuts) = cfg.shortcuts.as_mut() {
        if let Some(cur) = shortcuts.get(action).map(str::to_string)
            && let Err(e) = app.global_shortcut().unregister(cur.as_str())
        {
            tracing::warn!("解绑快捷键 {cur} 失败: {e}");
        }
        shortcuts.set(action, None);
    }
    settings::save_settings(&cfg)?;
    Ok(())
}

/// 退出应用。退出前回收 audio.cpp sidecar 进程。
#[tauri::command]
fn quit_app(app: AppHandle) {
    zapmomo::audiocpp::server::shutdown_blocking();
    app.exit(0);
}

/// 重启应用（退出后自动重新拉起，供设置页按钮调用）。退出前回收 audio.cpp sidecar 进程。
#[tauri::command]
fn restart_app(app: AppHandle) {
    zapmomo::audiocpp::server::shutdown_blocking();
    app.request_restart();
}

// ===========================================================================
// 模型库（Model Library）
// ===========================================================================

/// 模型库下载任务状态：单任务 + 可取消 + 记录当前下载的模型 id。
#[derive(Default)]
struct ModelLibraryState {
    in_progress: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    current_id: Arc<Mutex<Option<String>>>,
}

/// 模型库下载进度事件载荷。
#[derive(Clone, Serialize)]
struct ModelLibraryProgressPayload {
    model_id: String,
    stage: String,
    asset: String,
    overall_percent: f64,
    bytes_downloaded: u64,
    total_bytes: u64,
    message: String,
}

/// 下载任务 guard：所有出口（成功/失败/取消/panic）都复位下载标志与 cancel。
struct LibraryDownloadGuard {
    in_progress: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    current_id: Arc<Mutex<Option<String>>>,
}

impl Drop for LibraryDownloadGuard {
    fn drop(&mut self) {
        self.in_progress.store(false, Ordering::SeqCst);
        self.cancel.store(false, Ordering::SeqCst);
        *self.current_id.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

fn download_stage_str(stage: zapmomo::model_library::asset::DownloadStage) -> &'static str {
    use zapmomo::model_library::asset::DownloadStage::*;
    match stage {
        Downloading => "downloading",
        Verifying => "verifying",
        Extracting => "extracting",
        Done => "done",
    }
}

/// 从模型库列表解析模型（按 `id` 或 `install_id`；Current/Delete 可唯一定位具体安装实例）。
fn resolve_library_model(id: &str) -> Result<LibraryModel, String> {
    model_library::resolve_model(id).ok_or_else(|| format!("未知的模型：{id}"))
}

/// 平台化打开目录（macOS `open` / Linux `xdg-open` / Windows `explorer`）。
fn open_path(p: &Path) -> Result<(), String> {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(cmd)
        .arg(p)
        .spawn()
        .map_err(|e| format!("打开目录失败：{e}"))?;
    Ok(())
}

/// 模型库列表（含每个模型的安装状态 / current / runtime_status）。
#[tauri::command]
fn list_model_library(
    asr_dictate: State<'_, AsrDictateState>,
    tts: State<'_, TtsSynthesizeState>,
) -> Result<Vec<LibraryModel>, String> {
    let mut models = model_library::list_models();
    // 离线听写在跑 → ASR RuntimeActual 置位（模型库卡片显示 Active）
    let asr_actual = asr_dictate.active_model_dir();
    // TTS 无常驻引擎：actual = 当前 selection（与 current 判定同源，写配置即切换），
    // active = 是否有合成线程在跑。
    // LLM 已改为远程连接：无本地 runtime，llm 相关 actual 恒 None/false。
    let tts_actual = model_library::selection_path(LibModelType::Tts);
    let actuals = model_library::RuntimeActuals {
        // KWS 已随伴侣模块删除：RuntimeActual 恒 None（ModelType::Kws 收口在后续任务）
        kws: None,
        asr: asr_actual.as_deref(),
        tts: tts_actual.as_deref(),
        tts_active: tts.is_synthesizing(),
        llm: None,
        llm_switching: false,
        llm_switch_target: None,
        llm_load_error_path: None,
    };
    model_library::enrich_runtime_status(&mut models, &actuals);
    Ok(models)
}

/// 系统资源（独立命令，CPU 采样在阻塞线程执行）。
#[tauri::command]
async fn get_system_resources() -> Result<SystemResources, String> {
    tauri::async_runtime::spawn_blocking(model_library::sysinfo::get_system_resources)
        .await
        .map_err(|e| format!("资源检测失败：{e}"))
}

/// 下载并安装模型库中的 registry 模型（单任务，真实进度，可取消）。
#[tauri::command]
async fn download_library_model(
    app: AppHandle,
    state: State<'_, ModelLibraryState>,
    id: String,
) -> Result<(), String> {
    let flag = state.in_progress.clone();
    if flag.swap(true, Ordering::SeqCst) {
        return Err("已有模型下载进行中，请稍候".to_string());
    }
    state.cancel.store(false, Ordering::SeqCst);
    *state.current_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id.clone());

    // 平台门控：与列表层同一事实源（`list_models` 的过滤），
    // 堵住「前端硬编码预设 / 外部调用绕过 UI 直接按 id 下载」的口子
    let model = model_library::registry::model_for_current_platform(&id).ok_or_else(|| {
        match model_library::registry::model_by_id(&id) {
            Some(m) => format!(
                "该模型在当前平台不可用（{}仅支持 {}）",
                m.display_name,
                m.platforms.as_deref().unwrap_or_default().join(" / ")
            ),
            None => format!("未知的 Registry 模型：{id}"),
        }
    })?;
    if model.download.is_none() {
        flag.store(false, Ordering::SeqCst);
        *state.current_id.lock().unwrap_or_else(|e| e.into_inner()) = None;
        return Err("该模型没有内置下载源".to_string());
    }

    let app = app.clone();
    let cancel = state.cancel.clone();
    let current_id = state.current_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = LibraryDownloadGuard {
            in_progress: flag,
            cancel: cancel.clone(),
            current_id,
        };
        let emit = |stage: &str, percent: f64, msg: &str| {
            let _ = app.emit(
                "model-library-download-progress",
                ModelLibraryProgressPayload {
                    model_id: id.clone(),
                    stage: stage.to_string(),
                    asset: String::new(),
                    overall_percent: percent,
                    bytes_downloaded: 0,
                    total_bytes: 0,
                    message: msg.to_string(),
                },
            );
        };
        emit("preparing", 0.0, "准备下载…");
        let mut progress = |p: zapmomo::model_library::asset::DownloadProgress| {
            let _ = app.emit(
                "model-library-download-progress",
                ModelLibraryProgressPayload {
                    model_id: id.clone(),
                    stage: download_stage_str(p.stage).to_string(),
                    asset: String::new(),
                    overall_percent: p.percent,
                    bytes_downloaded: p.bytes_downloaded,
                    total_bytes: p.total_bytes,
                    message: p.message,
                },
            );
        };
        let install_cancel = cancel.clone();
        let result =
            model_library::install_managed_model(model, &mut progress, Some(&*install_cancel));
        match result {
            Ok(_) => {
                emit("done", 100.0, "模型安装完成");
                Ok(())
            }
            Err(zapmomo::model_library::asset::ModelError::Cancelled) => {
                emit("cancelled", 0.0, "已取消下载");
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| format!("下载任务异常：{e}"))?
}

/// 取消当前下载。
#[tauri::command]
fn cancel_model_download(state: State<'_, ModelLibraryState>) -> Result<(), String> {
    if !state.in_progress.load(Ordering::Relaxed) {
        return Err("没有正在进行的下载".to_string());
    }
    state.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

/// 设为当前模型（「使用」）。
///
/// 只写 `model_dir`，**绝不写 enabled / 自动启动能力**。
/// ASR 识别中提示重启；TTS 写 selection（下次合成生效）。
#[tauri::command]
async fn set_current_model(
    app: AppHandle,
    asr_dictate: State<'_, AsrDictateState>,
    id: String,
) -> Result<SetCurrentResult, String> {
    let model = resolve_library_model(&id)?;
    if model.install_state != LibInstallState::Installed {
        return Err("该模型未安装或正在下载，无法设为当前模型".to_string());
    }
    let path = PathBuf::from(model.local_path.clone().ok_or("该模型没有可用路径")?);
    let mt = model.model_type;

    // ---- KWS / ASR / TTS：写 selection；ASR 识别中提示重启 ----
    if mt != LibModelType::Llm {
        model_library::set_selected_model(mt, &path)?;

        let _ = &app;
        let (action, effective, message) = match mt {
            // KWS 已随伴侣模块删除：仅保留 selection 写入语义（无 runtime 可重启）
            LibModelType::Kws => (
                LibRuntimeAction::None,
                true,
                format!("已将 {} 设为当前模型", model.display_name),
            ),
            LibModelType::Asr if asr_dictate.is_dictating() => (
                LibRuntimeAction::RestartRequired,
                false,
                format!(
                    "已将 {} 设为 ASR 当前模型，将在下次启动识别时生效",
                    model.display_name
                ),
            ),
            LibModelType::Tts | LibModelType::Llm | LibModelType::Asr => (
                LibRuntimeAction::None,
                true,
                format!("已将 {} 设为当前模型", model.display_name),
            ),
        };
        Ok(SetCurrentResult {
            model_type: mt,
            model_id: model.id,
            path: path.display().to_string(),
            runtime_action: action,
            effective_immediately: effective,
            message,
        })
    } else {
        // LLM：本地推理已移除（远程连接配置在设置页）
        Err("本地 LLM 模型已移除：LLM 改为远程连接，请在设置页填写 API 地址与 Key".to_string())
    }
}

/// 删除模型：managed 删文件；external 只移除注册。后端全量安全检查。
#[tauri::command]
fn delete_model(
    dl: State<'_, ModelLibraryState>,
    asr_dictate: State<'_, AsrDictateState>,
    id: String,
) -> Result<(), String> {
    let model = resolve_library_model(&id)?;
    let downloading = dl.in_progress.load(Ordering::Relaxed)
        && dl
            .current_id
            .lock()
            .map(|g| g.as_deref() == Some(id.as_str()))
            .unwrap_or(false);
    if downloading {
        return Err("该模型正在下载，请先取消下载".to_string());
    }
    if model.current {
        return Err("该模型当前正在使用，请先切换到其他模型".to_string());
    }
    if let Some(lp) = &model.local_path {
        let lp = Path::new(lp);
        let loaded = asr_dictate
            .active_model_dir()
            .is_some_and(|d| model_library::paths_equal(&d, lp));
        if loaded {
            return Err("该模型当前仍在运行，请先停止或切换模型".to_string());
        }
    }

    if let Some(ext_id) = model_library::external_binding_to_remove(&id) {
        // external：只移除注册，绝不删原始文件
        model_library::remove_local_model_record(&ext_id)?;
        return Ok(());
    }
    // HF 安装：删除具体 artifact 目录（只删该 variant），并清理空父目录
    if model.source == model_library::ModelSource::Hf {
        if let Some(lp) = &model.local_path {
            let dir = model_library::runtime_to_install_dir(Path::new(lp));
            model_library::delete_hf_install_dir(&dir)?;
        }
        return Ok(());
    }
    let reg = model_library::registry::model_by_id(&id)
        .ok_or_else(|| format!("未知的 Registry 模型：{id}"))?;
    // 优先按 local_path（双根定位后的实际位置）推导目录；无 local_path（NotInstalled）
    // 再回退主根标准目录——旧根存量也能删到，而不是对着新根路径误判/漏删。
    let dir = model
        .local_path
        .map(|lp| model_library::runtime_to_install_dir(Path::new(&lp)))
        .filter(|d| d.exists())
        .unwrap_or_else(|| model_library::managed_install_dir(reg));
    if dir.exists() {
        model_library::delete_managed_dir(&dir)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 自定义数据目录（存储位置）
// ---------------------------------------------------------------------------

/// 存储迁移状态：防重入 + 取消标志。
#[derive(Default)]
struct StorageMigrateState {
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

impl StorageMigrateState {
    fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

/// 迁移 guard：所有出口（成功/失败/取消/panic）复位 running 与 cancel。
struct StorageMigrateGuard {
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

impl Drop for StorageMigrateGuard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.cancel.store(false, Ordering::SeqCst);
    }
}

/// 检查「设置/迁移数据目录」是否被占用（下载中 / 语音会话 / 监听 / 迁移中）。
///
/// 命中返回具体错误。
fn check_storage_busy(
    dl_asr: &AsrDownloadState,
    dl_tts: &TtsDownloadState,
    lib_dl: &ModelLibraryState,
    asr_dictate: &AsrDictateState,
) -> Result<(), String> {
    if dl_asr.in_progress.load(Ordering::Relaxed)
        || dl_tts.in_progress.load(Ordering::Relaxed)
        || lib_dl.in_progress.load(Ordering::Relaxed)
    {
        return Err("有模型正在下载，请先等待下载完成或取消后再操作".to_string());
    }
    if asr_dictate.is_dictating() {
        return Err("有识别任务正在运行，请先停止后再操作".to_string());
    }
    Ok(())
}

/// 读取存储信息（当前/旧根、占用大小、迁移可用性、磁盘空间）。
#[tauri::command]
async fn get_storage_info(mig: State<'_, StorageMigrateState>) -> Result<StorageInfoView, String> {
    let mut info =
        tauri::async_runtime::spawn_blocking(zapmomo::model_library::storage::collect_storage_info)
            .await
            .map_err(|e| e.to_string())??;
    info.migrating = mig.is_running();
    Ok(info)
}

/// 首次下载/导入前的存储位置引导信息（轻量查询，不做旧根全量遍历，可频繁调用）。
#[tauri::command]
async fn get_storage_prompt() -> Result<zapmomo::model_library::storage::StoragePromptView, String>
{
    tauri::async_runtime::spawn_blocking(zapmomo::model_library::storage::collect_prompt_info)
        .await
        .map_err(|e| e.to_string())?
}

/// 标记存储位置引导已确认（一次性标记，之后前端不再弹引导窗）。
#[tauri::command]
async fn acknowledge_storage_prompt() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        zapmomo::model_library::update_settings(|cfg| {
            cfg.storage_prompt_acknowledged = true;
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 设置（或清除）自定义数据目录。切换立即生效：新下载走新目录，存量模型保持可见可用。
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn set_data_dir(
    app: AppHandle,
    path: Option<String>,
    dl_asr: State<'_, AsrDownloadState>,
    dl_tts: State<'_, TtsDownloadState>,
    lib_dl: State<'_, ModelLibraryState>,
    asr_dictate: State<'_, AsrDictateState>,
    mig: State<'_, StorageMigrateState>,
) -> Result<StorageInfoView, String> {
    if mig.is_running() {
        return Err("正在迁移模型，请稍候".to_string());
    }
    check_storage_busy(
        dl_asr.inner(),
        dl_tts.inner(),
        lib_dl.inner(),
        asr_dictate.inner(),
    )?;

    let data_dir_value = match &path {
        Some(p) if !p.trim().is_empty() => Some(
            zapmomo::model_library::storage::validate_data_dir(Path::new(p))?
                .display()
                .to_string(),
        ),
        _ => None,
    };
    zapmomo::model_library::update_settings(|cfg| {
        cfg.data_dir = data_dir_value.clone();
        // 用户已在设置里对存储位置做出明确选择（含恢复默认），引导不再弹
        cfg.storage_prompt_acknowledged = true;
    })?;
    zapmomo::config::settings::refresh_data_dir_cache();
    let _ = app.emit("storage-dir-changed", ());

    tauri::async_runtime::spawn_blocking(zapmomo::model_library::storage::collect_storage_info)
        .await
        .map_err(|e| e.to_string())?
}

/// 迁移旧根存量到新数据目录（后台执行，进度经 `storage-migrate-progress` 事件推送）。
#[tauri::command]
async fn migrate_storage(
    app: AppHandle,
    mig: State<'_, StorageMigrateState>,
    dl_asr: State<'_, AsrDownloadState>,
    dl_tts: State<'_, TtsDownloadState>,
    lib_dl: State<'_, ModelLibraryState>,
    asr_dictate: State<'_, AsrDictateState>,
) -> Result<(), String> {
    if mig.is_running() {
        return Err("迁移已在进行中".to_string());
    }
    check_storage_busy(
        dl_asr.inner(),
        dl_tts.inner(),
        lib_dl.inner(),
        asr_dictate.inner(),
    )?;
    mig.running.store(true, Ordering::SeqCst);
    mig.cancel.store(false, Ordering::SeqCst);
    let running = mig.running.clone();
    let cancel = mig.cancel.clone();
    let emit_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = StorageMigrateGuard {
            running: running.clone(),
            cancel: cancel.clone(),
        };
        let outcome = zapmomo::model_library::storage::run_migration(
            false,
            &mut |p| {
                let _ = emit_app.emit("storage-migrate-progress", &p);
            },
            Some(&cancel),
        );
        match &outcome {
            Ok(o) => {
                if o.failed.is_empty() {
                    tracing::info!(
                        "存储迁移完成（moved={} skipped={}）",
                        o.moved.len(),
                        o.skipped.len()
                    );
                } else {
                    tracing::warn!("存储迁移部分失败：{:?}", o.failed);
                }
            }
            Err(e) => tracing::error!("存储迁移异常: {e}"),
        }
        outcome
    })
    .await
    .map_err(|e| format!("迁移任务异常: {e}"))??;

    let _ = app.emit("storage-dir-changed", ());
    Ok(())
}

/// 取消进行中的存储迁移（条目间/拷贝块间生效；已迁移条目保留）。
#[tauri::command]
fn cancel_storage_migration(mig: State<'_, StorageMigrateState>) -> Result<(), String> {
    if !mig.is_running() {
        return Err("当前没有迁移在运行".to_string());
    }
    mig.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

/// 在文件管理器中打开当前模型目录。
#[tauri::command]
fn open_storage_dir() -> Result<(), String> {
    open_path(&zapmomo::config::settings::get_models_dir())
}

/// 文字输入条窗口尺寸（逻辑像素，与 `setup` 中的 `inner_size` 保持一致）。
/// 高度预留：底部 26px 透明外边距给 CSS 阴影留扩散空间（透明窗口会裁剪窗口外阴影），
/// 其余留给多行生长与行内错误提示（发送失败时展示，正常时不占视觉空间——透明窗口）。
const CHATBOX_W: f64 = 520.0;
const CHATBOX_H: f64 = 96.0;
/// 输入条默认位置距屏幕工作区底边的留白（逻辑像素）：galgame 对话框位，明显高于贴边。
const CHATBOX_BOTTOM_MARGIN: f64 = 120.0;
/// 计算输入条窗口首次出现的位置（逻辑像素）：主屏工作区底部居中
/// （galgame 对话框位；排除 Dock / 任务栏）。拖动后由配置记忆接管。
fn default_chatbox_position(app: &AppHandle) -> Option<(f64, f64)> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let work = monitor.work_area();
    let scale = monitor.scale_factor();
    let left = work.position.x as f64 / scale;
    let top = work.position.y as f64 / scale;
    let w = work.size.width as f64 / scale;
    let h = work.size.height as f64 / scale;
    Some((
        left + (w - CHATBOX_W) / 2.0,
        top + h - CHATBOX_H - CHATBOX_BOTTOM_MARGIN,
    ))
}

/// 逻辑像素坐标是否落在任一显示器的可见工作区内。
///
/// 用于过滤「拔掉外接屏后残留的屏幕外记忆位置」：多屏布局变化后恢复窗口
/// 会导致窗口出现在不可见区域，此时应回退默认定位。查询失败时不拦截（保持恢复行为）。
fn position_on_any_monitor(app: &AppHandle, x: f64, y: f64) -> bool {
    match app.available_monitors() {
        Ok(monitors) => monitors.iter().any(|m| {
            let scale = m.scale_factor();
            let work = m.work_area();
            let left = work.position.x as f64 / scale;
            let top = work.position.y as f64 / scale;
            let right = left + work.size.width as f64 / scale;
            let bottom = top + work.size.height as f64 / scale;
            x >= left && x < right && y >= top && y < bottom
        }),
        Err(_) => true,
    }
}

/// 托盘 id（档位变化后 `tray_by_id` 定位托盘并重建菜单）。
const TRAY_ID: &str = "zapmomo-tray";

/// macOS 面板层级：Floating（3）之上 1 级，恒高于普通窗口（输入条/气泡不遮挡）。
const MACOS_OVERLAY_PANEL_LEVEL: i64 = 5;

/// 构建托盘「开机自启动」动作项（按当前状态显示相反动作）。
fn build_autostart_item(app: &AppHandle) -> tauri::Result<MenuItem<tauri::Wry>> {
    let (id, label) = autostart_item_labels(current_autostart_enabled(app));
    MenuItem::with_id(app, id, label, true, None::<&str>)
}

/// 托盘菜单：文字输入条（可勾选）、开机自启动（可勾选）、打开设置、重启、退出。
fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let chatbox = build_chatbox_item(app)?;
    let autostart = build_autostart_item(app)?;
    let open_settings = MenuItem::with_id(app, "open_settings", "打开设置", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, "restart", "重启", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let items: Vec<&dyn IsMenuItem<tauri::Wry>> =
        vec![&chatbox, &autostart, &open_settings, &restart, &quit];
    Menu::with_items(app, &items)
}

/// 档位/勾选态变化后重建托盘菜单，刷新勾选态。
fn rebuild_tray_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID)
        && let Ok(menu) = build_tray_menu(app)
    {
        let _ = tray.set_menu(Some(menu));
    }
}

/// 托盘/菜单事件分发。
fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "show_settings" | "open_settings" => show_settings_window(app),
        // 文字输入条：勾选态即目标态（点击时 muda 已自动翻转 checked，这里取反读到的
        // 配置值等价于「切到另一态」；set_chatbox_visible 内 rebuild 刷新勾选）
        "toggle_chatbox" => {
            let visible = !current_chatbox_visible();
            // 菜单勾选是用户显式打开输入条：显示后直接聚焦可打字
            set_chatbox_visible(app, visible, true);
        }
        // 退出/重启前回收 audio.cpp sidecar 进程（幂等）。
        "restart" => {
            zapmomo::audiocpp::server::shutdown_blocking();
            app.request_restart();
        }
        "quit" => {
            zapmomo::audiocpp::server::shutdown_blocking();
            app.exit(0);
        }
        // 开机自启动：按当前状态显示相反动作项，点击应用固定值（幂等）。
        "enable_autostart" => {
            let _ = apply_autostart(app, true);
        }
        "disable_autostart" => {
            let _ = apply_autostart(app, false);
        }
        _ => {}
    }
}

/// 读取文字输入条显隐开关（缺省显示）。
fn resolve_chatbox_visible(chatbox: Option<&ChatboxSettings>) -> bool {
    chatbox.and_then(|c| c.visible).unwrap_or(true)
}

/// 当前输入条显隐（托盘/菜单勾选态）。
fn current_chatbox_visible() -> bool {
    match settings::load_settings() {
        Ok(Some(s)) => resolve_chatbox_visible(s.chatbox.as_ref()),
        _ => true,
    }
}

/// 构建托盘「文字输入条」勾选项。
fn build_chatbox_item(app: &AppHandle) -> tauri::Result<CheckMenuItem<tauri::Wry>> {
    let visible = current_chatbox_visible();
    CheckMenuItem::with_id(
        app,
        "toggle_chatbox",
        "文字输入条",
        true,
        visible,
        None::<&str>,
    )
}

/// Tauri 应用入口。
pub fn run() {
    zapmomo::logging::init_logging();
    tauri::Builder::default()
        // 单实例防护：官方要求注册为第一个插件。自启常驻后用户再手动点图标时，
        // 回调在已有实例内执行（第二进程自行退出）：前置设置窗。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_settings_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // 开机自启动：macOS 用 LaunchAgent（AppleScript 变体依赖 osascript，无必要）；
        // 注册时附加 `--autostart` 参数，setup 检测到则跳过设置窗自动弹出（静默启动）。
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .manage(AsrDictateState::new())
        .manage(AsrDownloadState::default())
        .manage(TtsSynthesizeState::new())
        .manage(TtsDownloadState::default())
        .manage(ModelLibraryState::default())
        .manage(StorageMigrateState::default())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            list_devices,
            request_mic_permission,
            get_microphone,
            set_microphone,
            get_asr_config,
            set_asr_enabled,
            set_asr_params,
            download_asr_model,
            transcribe_audio,
            start_asr_dictate,
            stop_asr_dictate,
            is_asr_dictating,
            get_tts_config,
            list_tts_voices,
            list_voice_library,
            save_tts_voice,
            delete_tts_voice,
            record_tts_voice,
            transcribe_reference_audio,
            synthesize_tts,
            stop_tts,
            is_tts_synthesizing,
            download_tts_model,
            set_tts_enabled,
            set_tts_params,
            set_tts_voice,
            set_tts_backend,
            list_model_library,
            get_system_resources,
            download_library_model,
            cancel_model_download,
            set_current_model,
            delete_model,
            get_storage_info,
            get_storage_prompt,
            acknowledge_storage_prompt,
            set_data_dir,
            migrate_storage,
            cancel_storage_migration,
            open_storage_dir,
            save_chatbox_position,
            hide_chatbox,
            get_hide_dock_icon,
            set_hide_dock_icon,
            get_autostart,
            set_autostart,
            open_settings,
            get_shortcuts,
            set_shortcut,
            clear_shortcut,
            quit_app,
            restart_app
        ])
        .setup(|app| {
            // macOS：默认以普通应用出现（Dock + Cmd+Tab 可见，有全局菜单栏）；
            // 用户可在设置中开启「隐藏应用图标」，此时切换为 Accessory（从 Dock 与 Cmd+Tab 消失）。
            let loaded = settings::load_settings().ok().flatten();
            #[cfg(target_os = "macos")]
            let hide_dock_icon = loaded.as_ref().map(|s| s.hide_dock_icon).unwrap_or(false);

            #[cfg(target_os = "macos")]
            {
                app.handle().set_activation_policy(if hide_dock_icon {
                    tauri::ActivationPolicy::Accessory
                } else {
                    tauri::ActivationPolicy::Regular
                })?;
            }

            // audio.cpp sidecar 环境：注入引擎搜索目录（externalBin 落位点 = 主程序
            // 同目录 + resource 目录），并启用 45s 空闲保活（GUI 测试语音/会话在窗口
            // 内复用热 server，热请求 0.1s 级）。不在此预热：backend 缺省 sherpa 的
            // 用户不触发进程；首次 audiocpp 合成时按需 spawn。
            {
                /// 还原 `\\?\` / `\\?\UNC\` 扩展路径前缀为普通路径。Tauri 的
                /// `resource_dir()` 在 Windows 返回带前缀的扩展路径，而子进程
                /// PATH 里的 `\\?\` 条目对 Windows DLL 搜索**无效**（实测：引擎
                /// 的 ggml/CUDA DLL 全部解析失败，cuda 与 cpu 后端一起挂），必须
                /// strip 后再注入。
                fn strip_extended_prefix(p: std::path::PathBuf) -> std::path::PathBuf {
                    let s = p.as_os_str().to_string_lossy();
                    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
                        std::path::PathBuf::from(format!(r"\\{rest}"))
                    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
                        std::path::PathBuf::from(rest)
                    } else {
                        p
                    }
                }
                let mut search_dirs: Vec<std::path::PathBuf> = Vec::new();
                if let Some(exe_dir) = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                {
                    search_dirs.push(strip_extended_prefix(exe_dir));
                }
                if let Ok(resource_dir) = app.path().resource_dir() {
                    let resource_dir = strip_extended_prefix(resource_dir);
                    if !search_dirs.contains(&resource_dir) {
                        search_dirs.push(resource_dir.clone());
                    }
                    // Windows 引擎运行时 DLL 随 resources 落 `resource_dir\audiocpp\`
                    // （ggml 动态后端 + MSVC CRT + CUDA 12.4 运行时）。子进程 PATH
                    // 是平铺目录搜索、不递归子目录，必须把 DLL 所在目录本身加进
                    // 去（见 server.rs::augmented_child_path）。注意不能以
                    // `resource_dir != exe_dir` 为前提裁剪这一条：Windows NSIS 下
                    // resource_dir == exe_dir（都落在安装根目录），引擎 exe 在根
                    // 目录而 DLL 在 audiocpp\ 子目录——裁剪掉它子进程 PATH 就再也
                    // 不含 DLL 目录，ggml 后端（cuda 和 cpu）一个都注册不上
                    // （实测签名：cuda → "not registered (available: none)"，
                    // cpu → "Failed to initialize CPU backend"）。
                    search_dirs.push(resource_dir.join("audiocpp"));
                }
                tracing::info!(target: "audiocpp", resource_dir = ?app.path().resource_dir().ok(), search_dirs = ?search_dirs, "audiocpp 引擎搜索目录注入");
                zapmomo::audiocpp::locator::set_search_dirs(search_dirs);
                zapmomo::audiocpp::server::set_idle_keepalive(Some(
                    std::time::Duration::from_secs(45),
                ));
            }

            // 设置窗口：默认隐藏，由 cmd+, 或托盘菜单打开；关闭时隐藏而非退出。
            let mut settings =
                WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
                    .title("ZapMomo 设置")
                    .inner_size(1180.0, 760.0)
                    .min_inner_size(1180.0, 640.0)
                    .resizable(true)
                    .visible(false);

            // macOS 用 titleBarStyle: Overlay 保留红绿灯；其它平台去掉系统标题栏。
            // title_bar_style / hidden_title 是 macOS 专属方法（Linux 上不存在），
            // 必须用 #[cfg] 编译期隔离，而非 cfg! 运行时判断。
            #[cfg(target_os = "macos")]
            {
                // macOS 保留系统红绿灯与阴影；窗口默认不透明。
                settings = settings
                    .title_bar_style(TitleBarStyle::Overlay)
                    .hidden_title(true)
                    .shadow(true);
            }
            // Linux：去掉系统标题栏，保留透明窗口供 CSS 圆角裁出（三键悬浮右上角，与 Windows 一致）。
            #[cfg(target_os = "linux")]
            {
                settings = settings.decorations(false).transparent(true);
            }
            // Windows：去掉系统标题栏即可；无 CSS 圆角处理，无需透明窗口
            //（不透明窗口性能更好）。同时关 tao shadow：undecorated+shadow 会被
            // tao 在 WM_NCCALCSIZE 里左右底三边缩进客户区、由 DWM 画黑色窗框，
            // 而顶部 inset 在 Win10 强制为 0（否则画出原生标题栏），形成三边黑框。
            // 原生阴影改由建窗后 DwmExtendFrameIntoClientArea 注入（不触发 inset，
            // Win10/Win11 通吃，见 apply_settings_window_shadow）；四边细边框仍由
            // 前端 AppShell 用 CSS 自绘。三键悬浮右上角。
            #[cfg(target_os = "windows")]
            {
                settings = settings.decorations(false).shadow(false);
            }
            settings.build()?;
            // Windows：注入 DWM 扩展边框换原生窗口阴影（其余平台无需处理：
            // macOS 建窗参数已 shadow(true)，Linux 由窗口管理器绘制）。
            #[cfg(windows)]
            apply_settings_window_shadow(app.handle());

            // 文字输入条窗口：显隐走持久化开关（缺省显示——首次启动随角色一同
            // 出现），托盘/右键菜单「文字输入条」可勾选开关；关闭走全局
            // CloseRequested → hide。macOS 建窗后转为非激活面板
            // （见下方 to_panel::<ChatboxPanel>）：聚焦输入不激活应用，IME 行为
            // 由 can_become_key_window 保证；其它平台保持普通可激活窗口。
            let chatbox_cfg = loaded.as_ref().and_then(|s| s.chatbox.clone());
            let mut chatbox =
                WebviewWindowBuilder::new(app, "chatbox", WebviewUrl::App("chatbox.html".into()))
                    .title("ZapMomo 输入")
                    .inner_size(CHATBOX_W, CHATBOX_H)
                    .resizable(false)
                    .decorations(false)
                    .transparent(true)
                    .always_on_top(true)
                    .skip_taskbar(true)
                    // 透明窗口的原生阴影按整个窗口矩形绘制，与居中的圆角胶囊错位
                    // （视觉上像错位的边框）——与角色窗口一致关闭，胶囊自带 CSS shadow。
                    .shadow(false)
                    .visible(false);
            // macOS：首次点击直达 webview（无需先点一下聚焦），与角色窗口一致——
            // 否则按住把手的第一下只用于激活窗口，第二下才能拖动。
            #[cfg(target_os = "macos")]
            {
                chatbox = chatbox.accept_first_mouse(true);
            }
            // 定位：配置记忆（落在所有显示器之外时视为多屏布局已变化，回退默认）> 屏幕底部居中
            let saved_pos = chatbox_cfg
                .as_ref()
                .and_then(|c| c.window_position.clone())
                .map(|p| (p.x as f64, p.y as f64))
                .filter(|&(x, y)| position_on_any_monitor(app.handle(), x, y));
            if let Some((x, y)) = saved_pos.or_else(|| default_chatbox_position(app.handle())) {
                chatbox = chatbox.position(x, y);
            }
            chatbox.build()?;
            // macOS：转成非激活面板——聚焦输入框不激活应用（不会把开着的设置窗带到最前），
            // 键盘/中文 IME 输入由 can_become_key_window 保证。
            #[cfg(target_os = "macos")]
            {
                use tauri_nspanel::{CollectionBehavior, StyleMask, WebviewWindowExt};

                if let Some(window) = app.get_webview_window("chatbox")
                    && let Ok(panel) = window.to_panel::<ChatboxPanel>()
                {
                    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
                    panel.set_collection_behavior(
                        CollectionBehavior::new()
                            .stationary()
                            .move_to_active_space()
                            .full_screen_auxiliary()
                            .into(),
                    );
                    // 层级常置 Floating 之上 1 级（3 = NSModalPanelWindowLevel 之上，
                    // 恒高于普通窗口，输入条不被遮挡）。
                    panel.set_level(MACOS_OVERLAY_PANEL_LEVEL);
                }
            }
            // 恢复持久化的可见性（缺省显示）。
            // 走与显隐快捷键相同的单一写点并聚焦：macOS 经 NSPanel show_and_make_key
            // 使输入条持有 key window，配合前端挂载时的 textarea.focus()，启动即可直接
            // 打字；自启动（--autostart）不聚焦——桌宠静默出现，不抢其它应用的键盘输入
            // （与设置窗自启动不弹出同一原则）。
            let launched_by_autostart = is_launched_by_autostart(std::env::args());
            if resolve_chatbox_visible(chatbox_cfg.as_ref()) {
                set_chatbox_visible(app.handle(), true, !launched_by_autostart);
            }

            // 自动打开设置窗口：仅用于「无全局菜单栏」的场景（macOS Accessory 模式或非 macOS），
            // 否则 Cmd+, 快捷键不可靠，自动打开可避免「找不到设置」；普通模式有菜单栏，无需自动弹出。
            // 弹出走 show_settings_window_unfocused：自动打开只为可发现性，键盘焦点留给输入条。
            // 自启动拉起（--autostart）时跳过：桌宠静默出现，设置窗不自动弹出；
            // 手动启动行为不变。
            #[cfg(target_os = "macos")]
            let auto_open_settings = hide_dock_icon;
            #[cfg(not(target_os = "macos"))]
            let auto_open_settings = true;
            if auto_open_settings && !launched_by_autostart {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    show_settings_window_unfocused(&app_handle);
                });
            }

            // 应用菜单（仅 macOS）：「ZapMomo」子菜单（偏好设置 cmd+, / 退出 Cmd+Q）
            // 与「编辑」菜单。macOS 的 Cmd+C/V/X/A/Z 依赖菜单中的「编辑」项
            // （key equivalent）才能派发到 WebView 输入框；自定义菜单若缺少这些项，
            // 复制/粘贴/全选会全部失效。
            //
            // Windows/Linux 不设 app 级菜单：Tauri 的 set_menu 会把它作为原生菜单栏
            // 渲染进每个窗口，页面顶部会多出一条菜单；
            // 而这些平台的 Ctrl+C/V 无需菜单即可生效，设置入口走托盘/右键菜单。
            #[cfg(target_os = "macos")]
            {
                let show_settings = MenuItem::with_id(
                    app,
                    "show_settings",
                    "偏好设置…",
                    true,
                    Some("CmdOrCtrl+,"),
                )?;
                let undo = PredefinedMenuItem::undo(app, None)?;
                let redo = PredefinedMenuItem::redo(app, None)?;
                let edit_sep1 = PredefinedMenuItem::separator(app)?;
                let cut = PredefinedMenuItem::cut(app, None)?;
                let copy = PredefinedMenuItem::copy(app, None)?;
                let paste = PredefinedMenuItem::paste(app, None)?;
                let select_all = PredefinedMenuItem::select_all(app, None)?;
                let edit_menu = Submenu::with_items(
                    app,
                    "编辑",
                    true,
                    &[&undo, &redo, &edit_sep1, &cut, &copy, &paste, &select_all],
                )?;
                // 退出项必须用自定义 MenuItem 而非 PredefinedMenuItem::quit：
                // 后者在 macOS 绑定原生 `terminate:`，而 terminate 会逐个询问可见窗口
                // `windowShouldClose:`——被下方 on_window_event 的 prevent_close 拦截后
                // 整个退出被取消（Cmd+Q 表现为窗口隐藏、进程残留）。自定义项直接走
                // handle_menu("quit") → app.exit(0)，绕过窗口询问，与托盘「退出」一致。
                let quit =
                    MenuItem::with_id(app, "quit", "退出 ZapMomo", true, Some("CmdOrCtrl+Q"))?;
                // 注意：muda 在 macOS 只把 Submenu 渲染为菜单栏项，顶级普通 MenuItem
                // 不显示（快捷键仍可派发）。因此偏好设置/退出须收进 app 名子菜单，
                // 保持「Apple | ZapMomo | 编辑」的 macOS 惯例结构。
                let sep = PredefinedMenuItem::separator(app)?;
                let app_submenu =
                    Submenu::with_items(app, "ZapMomo", true, &[&show_settings, &sep, &quit])?;
                let app_menu = Menu::with_items(app, &[&app_submenu, &edit_menu])?;
                app.set_menu(app_menu)?;
            }

            // 托盘菜单：文字输入条、开机自启动、打开设置、重启、退出。
            let tray_menu = build_tray_menu(app.handle())?;

            // 托盘图标：使用专用托盘图标（tray-icon.png）——真实应用图标的无边距版本，
            // 撑满菜单栏，避免 512px 主图标 9% 留白导致的偏小。
            let tray_icon =
                tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
                    .expect("托盘图标加载失败");
            // 菜单事件统一由 app 级 on_menu_event 处理（见下方 Builder::on_menu_event）。
            // 不可在 TrayIcon 上再注册 on_menu_event：tauri 会把 TrayIcon 的 handler
            // 也注册到全局菜单监听器，与 app 级并列，导致每个菜单事件被 handle_menu
            // 处理两次（CheckMenuItem 取反因此净效果为零，表现为点击无效）。
            TrayIconBuilder::with_id(TRAY_ID)
                .icon(tray_icon)
                .menu(&tray_menu)
                .build(app)?;

            // 注册用户自定义全局快捷键（[shortcuts] 分节；单个失败仅告警）
            register_shortcuts_at_startup(app.handle());

            Ok(())
        })
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // 关闭设置/输入条窗口时仅隐藏，不退出进程；退出走托盘/菜单 Cmd+Q
                //（菜单退出项须用自定义 MenuItem——原生 quit 会走 terminate: →
                //  windowShouldClose:，被本拦截器取消，见上方菜单构建处注释）。
                api.prevent_close();
                let _ = window.hide();
            }
        })
        // RunEvent::Exit 兜底回收 audio.cpp sidecar：覆盖全部退出路径（含未来新增
        // 出口与系统强退前的钩子），与三处显式 shutdown_blocking（幂等）双保险。
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                zapmomo::audiocpp::server::shutdown_blocking();
            }
        });
}

#[cfg(test)]
mod chatbox_visible_tests {
    use super::resolve_chatbox_visible;
    use zapmomo::config::settings::ChatboxSettings;

    #[test]
    fn test_resolve_chatbox_visible_missing_defaults_true() {
        // 首次启动（无 [chatbox] 段）：输入条随角色默认显示
        assert!(resolve_chatbox_visible(None));
        assert!(resolve_chatbox_visible(Some(&ChatboxSettings::default())));
    }

    #[test]
    fn test_resolve_chatbox_visible_reads_flag() {
        let on = ChatboxSettings {
            visible: Some(true),
            ..Default::default()
        };
        let off = ChatboxSettings {
            visible: Some(false),
            ..Default::default()
        };
        assert!(resolve_chatbox_visible(Some(&on)));
        assert!(!resolve_chatbox_visible(Some(&off)));
    }
}

#[cfg(test)]
mod autostart_tests {
    use super::{autostart_item_labels, is_launched_by_autostart};

    #[test]
    fn test_autostart_flag_hits_at_any_position() {
        // 尾部命中（系统拉起的典型形态：可执行路径 + 插件附加参数）
        assert!(is_launched_by_autostart([
            "/usr/bin/ZapMomo",
            "--autostart"
        ]));
        // 中段命中（未来若再附加其它参数）
        assert!(is_launched_by_autostart([
            "/Applications/ZapMomo.app/Contents/MacOS/ZapMomo",
            "--autostart",
            "--other"
        ]));
    }

    #[test]
    fn test_autostart_flag_requires_exact_match() {
        // 空命令行 / 仅可执行路径
        assert!(!is_launched_by_autostart(Vec::<String>::new()));
        assert!(!is_launched_by_autostart(["target/debug/ZapMomo"]));
        // 前缀 / 去杠 / 赋值变体均不命中（精确匹配，避免误吞用户显式参数）
        assert!(!is_launched_by_autostart(["--autostart-x"]));
        assert!(!is_launched_by_autostart(["autostart"]));
        assert!(!is_launched_by_autostart(["--autostart=1"]));
    }

    #[test]
    fn test_autostart_item_labels_flip_by_state() {
        assert_eq!(
            autostart_item_labels(false),
            ("enable_autostart", "开机自启动")
        );
        assert_eq!(
            autostart_item_labels(true),
            ("disable_autostart", "关闭开机自启动")
        );
    }
}
