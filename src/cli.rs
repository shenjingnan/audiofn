use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "audiofn",
    // 帮助/用法/补全统一按目标产品名展示（bin 文件名仍是旧名，品牌改名见 Task 15）；
    // 显式 bin_name 优先于运行时从 argv[0] 推导的名称
    bin_name = "audiofn",
    version = VERSION,
    about = "Audio toolkit：Qwen3-ASR 语音转写 + Qwen3-TTS 音色克隆（audiocpp 引擎）",
    subcommand_required = true,
    arg_required_else_help = true,
    disable_help_subcommand = true,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
#[non_exhaustive]
pub enum Commands {
    /// 显示配置信息
    Config,
    /// 生成 Shell 补全脚本
    #[command(hide = true)]
    Completion {
        /// Shell 类型：bash、zsh、fish、powershell、elvish
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// 语音识别（ASR）
    Asr {
        #[command(subcommand)]
        cmd: AsrCmd,
    },
    /// 文本转语音（TTS）
    Tts {
        #[command(subcommand)]
        cmd: TtsCmd,
    },
}

/// ASR 子命令
#[derive(Subcommand)]
pub enum AsrCmd {
    /// 离线转写 wav 文件（不需要麦克风；audiocpp qwen3_asr，自动识别语种）
    Transcribe {
        /// wav 路径；缺省用模型目录内 test_wavs/ 示例音频
        #[arg(long)]
        wav: Option<PathBuf>,
        /// 模型目录（覆盖 settings.toml 的 asr.model_dir）
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// 转写语言（透传 audiocpp；缺省由模型自动识别），如 zh / en / ja
        #[arg(long)]
        language: Option<String>,
        /// 反向文本正则化（数字/标点），audiocpp 后端不支持、忽略此参数
        #[arg(long)]
        use_itn: Option<bool>,
    },
    /// 列出可用的麦克风输入设备
    Devices,
    /// 从模型库下载并安装 ASR 模型（缺省 Qwen3-ASR 0.6B，单 GGUF）
    InstallModel {
        /// 模型库 registry 条目 id（如 asr-qwen3-0.6b-audiocpp）
        #[arg(long)]
        registry_id: Option<String>,
        /// 已安装也强制重新下载
        #[arg(long)]
        force: bool,
    },
    /// 免提听写（录音 → 停止后整段送 audiocpp qwen3_asr 转写）
    Dictate {
        /// 模型目录（覆盖 settings.toml 的 asr.model_dir）
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// 指定输入设备名（包含匹配），默认系统默认麦克风
        #[arg(long)]
        device: Option<String>,
        /// 听写时长（秒），默认无限
        #[arg(long)]
        duration: Option<u64>,
        /// 转写语言（透传 audiocpp；缺省由模型自动识别），如 zh / en / ja
        #[arg(long)]
        language: Option<String>,
        /// 反向文本正则化（数字/标点），audiocpp 后端不支持、忽略此参数
        #[arg(long)]
        use_itn: Option<bool>,
    },
}

/// TTS 子命令
#[derive(Subcommand)]
pub enum TtsCmd {
    /// 把文本合成为 wav 文件
    Run {
        /// 要合成的文本
        #[arg(short, long)]
        text: String,
        /// 模型目录（覆盖 settings.toml 的 tts.model_dir）
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// 推理后端（audiocpp；残留 "sherpa" 由预检报错引导迁移）
        #[arg(long)]
        backend: Option<String>,
        /// audiocpp 引擎二进制路径（覆盖 locator 自动定位）
        #[arg(long)]
        engine_path: Option<PathBuf>,
        /// 语速，缺省 1.0
        #[arg(long)]
        speed: Option<f32>,
        /// 输出 wav 路径；缺省为用户数据目录下 tts/<时间戳>.wav
        #[arg(long)]
        output: Option<PathBuf>,
        /// 音色 id（模型包内置参考音色 / 自定义音色库 id）
        #[arg(long)]
        voice: Option<String>,
        /// 自定义参考音频 wav（配合 --reference-text 使用）
        #[arg(long)]
        reference_wav: Option<PathBuf>,
        /// 自定义参考音频的逐字转写文本
        #[arg(long)]
        reference_text: Option<String>,
    },
    /// 列出可用音色（模型包内置参考音色 + 自定义音色库）
    Voices {
        /// 模型目录（覆盖 settings.toml 的 tts.model_dir）
        #[arg(long)]
        model_dir: Option<PathBuf>,
    },
    /// 从模型库下载并安装 TTS 模型（缺省 Qwen3-TTS 0.6B，安装到用户模型目录）
    InstallModel {
        /// 模型库 registry 条目 id（如 tts-qwen3-17b-base-q8-audiocpp）
        #[arg(long)]
        registry_id: Option<String>,
        /// 已安装也强制重新下载
        #[arg(long)]
        force: bool,
    },
}

/// config 命令
fn cmd_config() -> Result<String, String> {
    let config = serde_json::json!({
        "version": VERSION,
        "debug": false,
        "logLevel": "info",
    });
    Ok(serde_json::to_string_pretty(&config).unwrap_or_default())
}

/// completion 命令
fn cmd_completion<W: std::io::Write>(shell: clap_complete::Shell, writer: &mut W) {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "audiofn", writer);
}

/// CLI 入口
pub async fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Some(Commands::Config) => {
            let output = cmd_config()?;
            println!("{output}");
            Ok(())
        }
        Some(Commands::Completion { shell }) => {
            cmd_completion(shell, &mut std::io::stdout());
            Ok(())
        }
        Some(Commands::Asr { cmd }) => cmd_asr(cmd).await,
        // TTS 含 audio.cpp 后端：reqwest blocking client 的 Drop 需在允许阻塞的
        // 上下文（否则 tokio panic），故 block_in_place 包住同步执行
        Some(Commands::Tts { cmd }) => tokio::task::block_in_place(|| cmd_tts(cmd)),
        None => unreachable!(),
    }
}

/// ASR 命令入口
async fn cmd_asr(cmd: AsrCmd) -> Result<(), String> {
    match cmd {
        AsrCmd::Transcribe {
            wav,
            model_dir,
            language,
            use_itn,
        } => {
            let mut cfg = asr_config(model_dir.as_ref())?;
            if language.is_some() {
                eprintln!("提示：--language 透传 audiocpp；缺省由模型自动识别语种。");
            }
            if use_itn.is_some() {
                eprintln!("提示：audiocpp 后端原生输出标点/规则化，--use-itn 已忽略。");
            }
            if language.is_some() {
                cfg.language = language;
            }
            if use_itn.is_some() {
                cfg.use_itn = use_itn;
            }
            let wav_path = wav
                .or_else(|| crate::asr::default_test_wav(&cfg.model_dir))
                .ok_or_else(|| {
                    format!(
                        "未指定 --wav 且 {} 下没有 test_wavs/*.wav 示例音频。\n若模型尚未安装，请先执行 {} 下载（安装后自带示例音频）。",
                        cfg.model_dir.display(),
                        crate::audiocpp::asr_families::QWEN3_ASR_06B.registry_hint,
                    )
                })?;
            crate::asr::run_offline(&cfg, &wav_path)
        }
        AsrCmd::Devices => {
            let devices = crate::audio::list_input_devices();
            if devices.is_empty() {
                println!("未找到任何输入设备。");
            } else {
                println!("可用输入设备:");
                for name in devices {
                    println!("  {name}");
                }
            }
            Ok(())
        }
        AsrCmd::InstallModel { registry_id, force } => {
            use crate::asr::config::DEFAULT_ASR_REGISTRY_ID;
            use crate::config::settings::get_models_dir;
            use crate::model_library::asset::{DownloadProgress, DownloadStage};
            use crate::model_library::{install_managed_model, registry};
            let id = registry_id.unwrap_or_else(|| DEFAULT_ASR_REGISTRY_ID.to_string());
            let model = registry::model_for_current_platform(&id)
                .ok_or_else(|| format!("未知的模型库条目（或当前平台不可用）: {id}"))?;
            if model.model_type != crate::model_library::registry::ModelType::Asr {
                return Err(format!("{id} 不是 ASR 模型条目"));
            }
            let dest = get_models_dir().join(&model.name);
            if !force && crate::asr::is_installed(&dest) {
                println!("模型已安装: {}", dest.display());
                return Ok(());
            }
            let mut progress = |p: DownloadProgress| {
                let stage = match p.stage {
                    DownloadStage::Downloading => "下载",
                    DownloadStage::Verifying => "校验",
                    DownloadStage::Done => "完成",
                };
                println!("[{stage}] {}", p.message);
            };
            let dest =
                install_managed_model(model, &mut progress, None).map_err(|e| e.to_string())?;
            println!("ASR 模型已就绪: {}", dest.display());
            Ok(())
        }
        AsrCmd::Dictate {
            model_dir,
            device,
            duration,
            language,
            use_itn,
        } => {
            use crate::asr::ConsoleAsrReaction;
            let mut cfg = asr_config(model_dir.as_ref())?;
            if cfg.model_type.is_streaming() {
                return Err(format!(
                    "当前模型类型 {} 不支持免提听写（离线模型专用）。请先安装并设为当前 SenseVoice/Whisper/Qwen3-ASR 模型。",
                    cfg.model_type.as_str()
                ));
            }
            if language.is_some() {
                eprintln!("提示：--language 透传 audiocpp；缺省由模型自动识别语种。");
            }
            if use_itn.is_some() {
                eprintln!("提示：audiocpp 后端原生输出标点/规则化，--use-itn 已忽略。");
            }
            if language.is_some() {
                cfg.language = language;
            }
            if use_itn.is_some() {
                cfg.use_itn = use_itn;
            }
            let stop = install_dictate_stop_signals();
            println!("开始录音（回车或 Ctrl-C 停止并转写）...");
            let mut reaction = ConsoleAsrReaction;
            crate::asr::dictate::run_dictate(
                &cfg,
                device.as_deref(),
                duration,
                &mut reaction,
                Some(&stop),
            )
        }
    }
}

/// 听写的 CLI 停止信号：回车（stdin 一行）或 Ctrl-C（tokio signal）都置位标志。
///
/// Ctrl-C 经 tokio signal 驱动接管——录音期间按下不再直接终止进程，而是走
/// 「停止录音 → 整段转写 → 输出文本」的正常收尾，转写完进程自然退出。
/// stdin EOF（管道 / 重定向输入）不置位，避免非交互场景刚启动就停；
/// 此时仍有 Ctrl-C 与 `--duration` 两个停止途径。
fn install_dictate_stop_signals() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let stop = Arc::new(AtomicBool::new(false));

    // 回车：阻塞读一行（线程内，不占用异步运行时）
    std::thread::spawn({
        let stop = stop.clone();
        move || {
            let mut line = String::new();
            match std::io::stdin().read_line(&mut line) {
                Ok(0) | Err(_) => {}
                Ok(_) => stop.store(true, Ordering::Relaxed),
            }
        }
    });
    // Ctrl-C：信号事件由运行时投递（`#[tokio::main]` 为多线程 runtime，
    // 录音循环阻塞在其中一个 worker 上时其余 worker 仍可处理信号任务）
    tokio::spawn({
        let stop = stop.clone();
        async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                stop.store(true, Ordering::Relaxed);
            }
        }
    });
    stop
}

/// 读取 settings 并解析 ASR 配置
fn asr_config(
    cli_model_dir: Option<&PathBuf>,
) -> Result<crate::asr::config::ResolvedAsrConfig, String> {
    let settings = crate::config::settings::load_settings()?;
    let asr_settings = settings.as_ref().and_then(|s| s.asr.clone());
    crate::asr::config::resolve(asr_settings.as_ref(), cli_model_dir.map(|p| p.as_path()))
}

/// TTS 命令入口（同步：内部无 await；经 `block_in_place` 调用，见 dispatch 注释）
fn cmd_tts(cmd: TtsCmd) -> Result<(), String> {
    match cmd {
        TtsCmd::Run {
            text,
            model_dir,
            backend,
            engine_path,
            speed,
            output,
            voice,
            reference_wav,
            reference_text,
        } => {
            let mut cfg = tts_config(model_dir.as_ref())?;
            apply_backend_override(&mut cfg, backend, engine_path)?;
            let engine = crate::tts::TtsEngine::new(cfg.clone())?;
            let speed = speed.unwrap_or(1.0);
            // 合成音色参数统一解析（见 tts::voice）
            let voice_params = crate::tts::voice::resolve_voice_params(
                &cfg,
                voice.as_deref(),
                reference_wav.as_deref(),
                reference_text.as_deref(),
            )?;
            let out_path = output.unwrap_or_else(crate::tts::default_output_path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建输出目录失败: {e}"))?;
            }
            engine.synthesize_to_wav(&text, speed, &voice_params, &out_path)?;
            println!("已合成: {}", out_path.display());
            Ok(())
        }
        TtsCmd::Voices { model_dir } => {
            let cfg = tts_config(model_dir.as_ref())?;
            let desc = crate::audiocpp::families::family_desc(cfg.model_type).ok_or_else(|| {
                format!("模型类型 {} 不支持 audiocpp 后端", cfg.model_type.as_str())
            })?;
            println!(
                "audiocpp 后端（{}）为参考音频克隆模型：用 --reference-wav/--reference-text 指定参考音频，或 --voice 选择音色库 id\n（Base 版无 auto voice 兜底，不可省略参考音色）",
                desc.family
            );
            let voices = crate::tts::voice::list_builtin_voices(&cfg.model_dir);
            let custom = crate::tts::voice_store::list_custom_voices();
            if !voices.is_empty() {
                println!("模型包内置音色:");
                for v in &voices {
                    println!("  {}  {}", v.id, v.name);
                }
            }
            if !custom.is_empty() {
                println!("自定义音色库:");
                for v in &custom {
                    println!("  {}  {}", v.id, v.name);
                }
            }
            if voices.is_empty() && custom.is_empty() {
                println!(
                    "未找到可用音色（请先安装模型/在音色库录制音色）。\n{}",
                    desc.registry_hint
                );
            }
            Ok(())
        }
        TtsCmd::InstallModel { registry_id, force } => {
            use crate::model_library::asset::{DownloadProgress, DownloadStage};
            use crate::model_library::{install_managed_model, registry};
            let id = registry_id
                .unwrap_or_else(|| crate::tts::config::DEFAULT_TTS_REGISTRY_ID.to_string());
            let model = registry::model_for_current_platform(&id)
                .ok_or_else(|| format!("未知的模型库条目（或当前平台不可用）: {id}"))?;
            if model.model_type != crate::model_library::registry::ModelType::Tts {
                return Err(format!("{id} 不是 TTS 模型条目"));
            }
            let dest = crate::config::settings::get_models_dir().join(&model.name);
            if !force && crate::tts::is_installed(&dest) {
                println!("模型已安装: {}", dest.display());
                return Ok(());
            }
            let mut progress = |p: DownloadProgress| {
                let stage = match p.stage {
                    DownloadStage::Downloading => "下载",
                    DownloadStage::Verifying => "校验",
                    DownloadStage::Done => "完成",
                };
                println!("[{stage}] {}", p.message);
            };
            let dest =
                install_managed_model(model, &mut progress, None).map_err(|e| e.to_string())?;
            println!("TTS 模型已就绪: {}", dest.display());
            Ok(())
        }
    }
}

/// CLI `--backend` / `--engine-path` 覆盖：校验合法后写入解析后的配置。
///
/// 一期裁剪后引擎只有 audiocpp 一条路径；老配置残留的 `--backend sherpa` 解析
/// 不报错（便于构造迁移提示），由 `tts::config::preflight` 明确报「已移除」。
fn apply_backend_override(
    cfg: &mut crate::tts::config::ResolvedTtsConfig,
    backend: Option<String>,
    engine_path: Option<PathBuf>,
) -> Result<(), String> {
    if let Some(v) = backend.as_deref() {
        cfg.backend = crate::tts::config::TtsBackendKind::parse_str(v)
            .ok_or_else(|| format!("未知 TTS 后端: {v}（支持 audiocpp）"))?;
    }
    if let Some(p) = engine_path {
        cfg.engine_path = Some(p);
    }
    Ok(())
}

/// 读取 settings 并解析 TTS 配置
fn tts_config(
    cli_model_dir: Option<&PathBuf>,
) -> Result<crate::tts::config::ResolvedTtsConfig, String> {
    let settings = crate::config::settings::load_settings()?;
    let tts_settings = settings.as_ref().and_then(|s| s.tts.clone());
    crate::tts::config::resolve(tts_settings.as_ref(), cli_model_dir.map(|p| p.as_path()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constant() {
        assert!(!VERSION.is_empty(), "VERSION should not be empty");
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "VERSION should be in semver format (X.Y.Z)");
        for part in &parts {
            assert!(!part.is_empty(), "semver part should not be empty");
            assert!(
                part.chars().all(|c| c.is_ascii_digit()),
                "semver part '{}' should be numeric",
                part
            );
        }
    }

    #[test]
    fn test_config_output() {
        let output = cmd_config().unwrap();
        let val: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(val["debug"], serde_json::Value::Bool(false));
        assert_eq!(
            val["logLevel"],
            serde_json::Value::String("info".to_string())
        );
        assert_eq!(val.as_object().unwrap().len(), 3);
    }

    #[test]
    fn test_config_contains_version() {
        let output = cmd_config().unwrap();
        assert!(output.contains(VERSION));
    }

    #[test]
    fn test_completion_bash() {
        let mut buf = Vec::new();
        cmd_completion(clap_complete::Shell::Bash, &mut buf);
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("complete -F"),
            "bash completion should contain complete -F"
        );
        for sub in &["config", "asr", "tts", "completion"] {
            assert!(
                output.contains(sub),
                "bash completion should contain subcommand {}",
                sub
            );
        }
        assert!(
            !output.contains("greet"),
            "bash completion should not mention removed greet"
        );
    }

    #[test]
    fn test_completion_zsh() {
        let mut buf = Vec::new();
        cmd_completion(clap_complete::Shell::Zsh, &mut buf);
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("#compdef"),
            "zsh completion should start with #compdef"
        );
        for sub in &["config", "asr", "tts", "completion"] {
            assert!(
                output.contains(sub),
                "zsh completion should contain subcommand {}",
                sub
            );
        }
    }

    #[test]
    fn test_completion_fish() {
        let mut buf = Vec::new();
        cmd_completion(clap_complete::Shell::Fish, &mut buf);
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("complete -c"),
            "fish completion should contain complete -c"
        );
        for sub in &["config", "asr", "tts", "completion"] {
            assert!(
                output.contains(sub),
                "fish completion should contain subcommand {}",
                sub
            );
        }
    }

    #[test]
    fn test_completion_powershell() {
        let mut buf = Vec::new();
        cmd_completion(clap_complete::Shell::PowerShell, &mut buf);
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("Register-ArgumentCompleter"),
            "powershell completion should register argument completer"
        );
        for sub in &["config", "asr", "tts", "completion"] {
            assert!(
                output.contains(sub),
                "powershell completion should contain subcommand {}",
                sub
            );
        }
    }

    #[test]
    fn test_completion_all_shells_have_all_subcommands() {
        let shells = [
            clap_complete::Shell::Bash,
            clap_complete::Shell::Zsh,
            clap_complete::Shell::Fish,
            clap_complete::Shell::PowerShell,
        ];
        for shell in shells {
            let mut buf = Vec::new();
            cmd_completion(shell, &mut buf);
            let output = String::from_utf8(buf).unwrap();
            for sub in &["config", "asr", "tts", "completion"] {
                assert!(
                    output.contains(sub),
                    "{:?} completion should contain subcommand {}",
                    shell,
                    sub
                );
            }
        }
    }

    /// `--help` 可见文案不得残留旧品牌与已删除的 greet 命令（CLI 面验收）。
    #[test]
    fn test_help_text_has_no_legacy_brand_or_greet() {
        // bin_name 必须显式固定为 audiofn：否则运行时会取 argv[0]（旧二进制文件名
        // zapmomo-cli）写进用法行，--help 又会露出旧品牌
        assert_eq!(
            Cli::command().get_bin_name(),
            Some("audiofn"),
            "显式 bin_name 缺失，--help 用法行会回退到旧二进制名"
        );
        let help = Cli::command().render_help().to_string();
        for banned in ["zapmomo", "ZapMomo", "greet"] {
            assert!(
                !help.to_lowercase().contains(&banned.to_lowercase()),
                "--help 不应包含 {banned}\n{help}"
            );
        }
        for expected in &["config", "asr", "tts"] {
            assert!(help.contains(expected), "--help 应包含 {expected}\n{help}");
        }

        // 子命令级帮助：asr 面应含 transcribe（原 asr test 已改名）。
        // `render_help` 需要 `&mut Command`，这里借 `--help` 的 DisplayHelp 错误取文案。
        let asr_help = match Cli::try_parse_from(["audiofn", "asr", "--help"]) {
            Err(e) => e.render().to_string(),
            Ok(_) => panic!("--help 应以 DisplayHelp 错误结束"),
        };
        for banned in ["zapmomo", "ZapMomo", "greet"] {
            assert!(
                !asr_help.to_lowercase().contains(&banned.to_lowercase()),
                "asr --help 不应包含 {banned}\n{asr_help}"
            );
        }
        for expected in &["transcribe", "dictate", "install-model", "devices"] {
            assert!(
                asr_help.contains(expected),
                "asr --help 应包含 {expected}\n{asr_help}"
            );
        }
    }

    #[test]
    fn test_cli_parse_config() {
        let cli = Cli::try_parse_from(["test", "config"]).unwrap();
        assert!(matches!(cli.command.unwrap(), Commands::Config));
    }

    #[test]
    fn test_cli_parse_asr_transcribe() {
        let cli = Cli::try_parse_from(["test", "asr", "transcribe", "--wav", "a.wav"]).unwrap();
        match cli.command.unwrap() {
            Commands::Asr { cmd } => assert!(matches!(cmd, AsrCmd::Transcribe { .. })),
            _ => panic!("Expected Asr command"),
        }
        // 旧子命令名 `asr test` 已移除
        assert!(Cli::try_parse_from(["test", "asr", "test"]).is_err());
    }

    #[test]
    fn test_cli_parse_asr_dictate() {
        let cli = Cli::try_parse_from([
            "test",
            "asr",
            "dictate",
            "--device",
            "内置麦克风",
            "--duration",
            "10",
            "--language",
            "zh",
            "--use-itn",
            "true",
        ])
        .unwrap();
        match cli.command.unwrap() {
            Commands::Asr { cmd } => assert!(matches!(
                cmd,
                AsrCmd::Dictate {
                    device: Some(d),
                    duration: Some(10),
                    language: Some(l),
                    use_itn: Some(true),
                    ..
                } if d == "内置麦克风" && l == "zh"
            )),
            _ => panic!("Expected Dictate command"),
        }
    }

    #[test]
    fn test_cli_parse_asr_transcribe_flags() {
        // 文件转写的语言透传与 ITN 开关（ITN 由 audiocpp 后端忽略）
        let cli = Cli::try_parse_from([
            "test",
            "asr",
            "transcribe",
            "--language",
            "zh",
            "--use-itn",
            "true",
        ])
        .unwrap();
        match cli.command.unwrap() {
            Commands::Asr { cmd } => assert!(matches!(
                cmd,
                AsrCmd::Transcribe {
                    language: Some(l),
                    use_itn: Some(true),
                    ..
                } if l == "zh"
            )),
            _ => panic!("Expected Asr command"),
        }
    }

    #[test]
    fn test_cli_parse_asr_devices() {
        let cli = Cli::try_parse_from(["test", "asr", "devices"]).unwrap();
        match cli.command.unwrap() {
            Commands::Asr { cmd } => assert!(matches!(cmd, AsrCmd::Devices)),
            _ => panic!("Expected Asr command"),
        }
    }

    #[test]
    fn test_cli_parse_asr_install_model() {
        // 缺省 registry id = audiocpp Qwen3-ASR
        let cli = Cli::try_parse_from(["test", "asr", "install-model", "--force"]).unwrap();
        match cli.command.unwrap() {
            Commands::Asr {
                cmd: AsrCmd::InstallModel { registry_id, force },
            } => {
                assert!(force);
                assert_eq!(registry_id, None);
            }
            _ => panic!("Expected InstallModel command"),
        }
        // 显式 registry id 透传
        let cli = Cli::try_parse_from([
            "test",
            "asr",
            "install-model",
            "--registry-id",
            "asr-qwen3-0.6b-audiocpp",
        ])
        .unwrap();
        match cli.command.unwrap() {
            Commands::Asr {
                cmd: AsrCmd::InstallModel { registry_id, .. },
            } => {
                assert_eq!(registry_id.as_deref(), Some("asr-qwen3-0.6b-audiocpp"));
            }
            _ => panic!("Expected InstallModel command"),
        }
    }

    #[test]
    fn test_cli_parse_tts_run() {
        let cli = Cli::try_parse_from(["test", "tts", "run", "--text", "你好", "--speed", "1.2"])
            .unwrap();
        match cli.command.unwrap() {
            Commands::Tts { cmd } => assert!(matches!(
                cmd,
                TtsCmd::Run {
                    text,
                    speed: Some(1.2),
                    voice: None,
                    ..
                } if text == "你好"
            )),
            _ => panic!("Expected Tts command"),
        }
    }

    #[test]
    fn test_cli_parse_tts_run_with_voice() {
        let cli = Cli::try_parse_from([
            "test", "tts", "run", "--text", "你好", "--voice", "leijun-1",
        ])
        .unwrap();
        match cli.command.unwrap() {
            Commands::Tts { cmd } => assert!(matches!(
                cmd,
                TtsCmd::Run {
                    voice: Some(id),
                    ..
                } if id == "leijun-1"
            )),
            _ => panic!("Expected Tts command"),
        }
    }

    #[test]
    fn test_cli_parse_tts_voices() {
        let cli = Cli::try_parse_from(["test", "tts", "voices"]).unwrap();
        match cli.command.unwrap() {
            Commands::Tts { cmd } => assert!(matches!(cmd, TtsCmd::Voices { .. })),
            _ => panic!("Expected Tts command"),
        }
    }

    #[test]
    fn test_cli_parse_tts_install_model() {
        let cli = Cli::try_parse_from(["test", "tts", "install-model", "--force"]).unwrap();
        match cli.command.unwrap() {
            Commands::Tts { cmd } => assert!(matches!(
                cmd,
                TtsCmd::InstallModel {
                    force: true,
                    registry_id: None,
                }
            )),
            _ => panic!("Expected InstallModel command"),
        }
        // 显式 registry id 透传
        let cli = Cli::try_parse_from([
            "test",
            "tts",
            "install-model",
            "--registry-id",
            "tts-qwen3-17b-base-q8-audiocpp",
        ])
        .unwrap();
        match cli.command.unwrap() {
            Commands::Tts {
                cmd: TtsCmd::InstallModel { registry_id, .. },
            } => {
                assert_eq!(
                    registry_id.as_deref(),
                    Some("tts-qwen3-17b-base-q8-audiocpp")
                );
            }
            _ => panic!("Expected InstallModel command"),
        }
    }

    #[test]
    fn test_apply_backend_override_valid_invalid_and_engine_path() {
        let mut cfg = crate::tts::config::ResolvedTtsConfig::default();

        // 非法后端报错（含支持列表）
        let err = apply_backend_override(&mut cfg, Some("vllm".to_string()), None).unwrap_err();
        assert!(err.contains("未知 TTS 后端"), "err: {err}");

        // audiocpp 显式覆盖生效（缺省同为 audiocpp，不改变 model_type）
        apply_backend_override(&mut cfg, Some("audiocpp".to_string()), None).unwrap();
        assert_eq!(cfg.backend, crate::tts::config::TtsBackendKind::Audiocpp);
        assert_eq!(cfg.model_type, crate::tts::config::TtsModelKind::Qwen3Tts06);

        // engine_path 透传
        let p = PathBuf::from("/opt/audiocpp_server");
        apply_backend_override(&mut cfg, None, Some(p.clone())).unwrap();
        assert_eq!(cfg.engine_path, Some(p));
    }
}
