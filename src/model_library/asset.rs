//! 模型资产下载基础设施（全项目共享，不隶属任何具体能力模块）。
//!
//! 模型元数据编译期嵌入（`include_str!`），运行时从用户目录
//! `~/.zapmomo/models/<name>` 安装/查找，供 asr / tts / model_library、
//! CLI（`asr/tts install-model`）及 GUI（下载按钮）复用。流程：
//! 下载 → sha256 校验 → 落位（裸文件原子改名 / 归档解压），幂等可重跑。
//!
//! 一期裁剪后清单只含 qwen3 三资产（单文件 GGUF），各能力的「默认装哪个模型、
//! 装到哪个目录」由各能力模块的 registry 常量解析（`asr::config::DEFAULT_ASR_REGISTRY_ID`
//! / `tts::config::DEFAULT_TTS_REGISTRY_ID`），本模块只提供通用下载/校验/落位。

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::config::settings::get_models_dir;

/// `models/manifest.json` 的顶层结构。
#[derive(Debug, Clone, Deserialize)]
pub struct ModelManifest {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    pub assets: Vec<ModelAsset>,
}

/// 单个模型资产。
#[derive(Debug, Clone, Deserialize)]
pub struct ModelAsset {
    pub name: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub version: String,
    /// 资产类型：`archive`（默认，tar.bz2 解压落位）或 `raw`（单文件直接落位）。
    #[serde(default)]
    pub kind: Option<String>,
    pub archive: String,
    pub source: String,
    pub sha256: String,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub license: String,
}

impl ModelAsset {
    /// 是否为「裸文件」资产（单文件下载，无需解压）。
    pub fn is_raw(&self) -> bool {
        self.kind.as_deref() == Some("raw")
    }
}

/// 编译期嵌入的清单 JSON（随仓库入库，打包后不依赖外部文件）。
const MANIFEST_JSON: &str = include_str!("../../models/manifest.json");

/// 解析一次并缓存。
fn manifest() -> &'static ModelManifest {
    static CACHE: OnceLock<ModelManifest> = OnceLock::new();
    CACHE.get_or_init(|| serde_json::from_str(MANIFEST_JSON).expect("内嵌模型清单无效"))
}

/// 按 role 查找模型资产（如 "asr-audiocpp-qwen3-06b"）。
pub fn asset_by_role(role: &str) -> Option<&'static ModelAsset> {
    manifest().assets.iter().find(|a| a.role == role)
}

/// 清单内全部资产 role（自洽校验 / 诊断用）。
pub fn manifest_roles() -> Vec<&'static str> {
    manifest().assets.iter().map(|a| a.role.as_str()).collect()
}

/// 用户模型根目录：`~/.zapmomo/models`
pub fn user_models_dir() -> PathBuf {
    get_models_dir()
}

/// 下载/安装阶段（CLI 打日志 / GUI 推事件共用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStage {
    Downloading,
    Verifying,
    Extracting,
    Done,
}

/// 下载进度回调载荷。
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub stage: DownloadStage,
    /// 下载阶段 0..=100；其它阶段为 `-1`（不确定进度）。
    pub percent: f64,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub message: String,
}

pub type ProgressFn<'a> = dyn FnMut(DownloadProgress) + Send + 'a;

/// 模型安装错误。
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),
    #[error("下载失败（重试后仍失败）: {0}")]
    Download(String),
    #[error("sha256 校验失败（期望 {expected}，实际 {actual}）")]
    Sha256Mismatch { expected: String, actual: String },
    #[error("解压失败: {0}")]
    Extract(String),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("磁盘空间不足：{0}")]
    InsufficientSpace(String),
    #[error("下载已取消")]
    Cancelled,
}

/// 目标目录是否已包含给定的一组文件。
pub fn has_required_files(dest_dir: &Path, required: &[&str]) -> bool {
    required.iter().all(|f| dest_dir.join(f).is_file())
}

/// 按指定资产安装（测试/多模型可复用）。`required_files` 用于幂等性判断。
///
/// 等价于 `install_asset_to_cancellable(..., None)`（不可取消）。
pub fn install_asset_to(
    asset: &ModelAsset,
    dest_dir: &Path,
    force: bool,
    on_progress: &mut ProgressFn,
    required_files: &[&str],
) -> Result<(), ModelError> {
    install_asset_to_cancellable(asset, dest_dir, force, on_progress, required_files, None)
}

/// 可取消版本的 [`install_asset_to`]。
///
/// `cancel` 为 `Some(&AtomicBool)` 时，下载读循环每轮检查；命中即清理临时文件并返回
/// [`ModelError::Cancelled`]。各阶段前也会再检查一次（下载/校验/解压/落位）。
pub fn install_asset_to_cancellable(
    asset: &ModelAsset,
    dest_dir: &Path,
    force: bool,
    on_progress: &mut ProgressFn,
    required_files: &[&str],
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(), ModelError> {
    if cancelled(cancel) {
        return Err(ModelError::Cancelled);
    }
    let parent = dest_dir
        .parent()
        .ok_or_else(|| ModelError::Extract("目标目录缺少父目录".to_string()))?;

    if !force && has_required_files(dest_dir, required_files) {
        on_progress(progress(DownloadStage::Done, 100.0, dest_dir, "模型已安装"));
        return Ok(());
    }

    std::fs::create_dir_all(parent)?;
    let tmp_archive = parent.join(format!(".{}.tmp", asset.archive));

    download_to(
        &asset.source,
        &tmp_archive,
        asset.size_bytes,
        on_progress,
        cancel,
    )?;
    if cancelled(cancel) {
        let _ = std::fs::remove_file(&tmp_archive);
        return Err(ModelError::Cancelled);
    }

    on_progress(progress(
        DownloadStage::Verifying,
        -1.0,
        dest_dir,
        "校验 sha256",
    ));
    verify_sha256(&tmp_archive, &asset.sha256)?;

    on_progress(progress(
        DownloadStage::Extracting,
        -1.0,
        dest_dir,
        "解压中",
    ));
    extract_and_place(&tmp_archive, dest_dir)?;

    on_progress(progress(
        DownloadStage::Done,
        100.0,
        dest_dir,
        "模型安装完成",
    ));
    Ok(())
}

/// 取消标志是否已置位。
fn cancelled(cancel: Option<&std::sync::atomic::AtomicBool>) -> bool {
    cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed))
}

/// 安装「裸文件」资产（单文件，无解压）到 `dest_path`。
///
/// 用于 audiocpp 单文件 GGUF 这类独立发布的模型文件。流程：
/// 幂等检查 → 下载到临时文件 → sha256 校验 → 原子落位（无解压阶段）。
/// 需要中途取消时用 [`install_raw_file_to_cancellable`]。
pub fn install_raw_file_to_cancellable(
    asset: &ModelAsset,
    dest_path: &Path,
    force: bool,
    on_progress: &mut ProgressFn,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(), ModelError> {
    if cancelled(cancel) {
        return Err(ModelError::Cancelled);
    }
    let parent = dest_path
        .parent()
        .ok_or_else(|| ModelError::Extract("目标文件缺少父目录".to_string()))?;

    if !force && dest_path.is_file() {
        on_progress(progress(
            DownloadStage::Done,
            100.0,
            dest_path,
            "模型已安装",
        ));
        return Ok(());
    }

    std::fs::create_dir_all(parent)?;
    // tmp 名只取 archive 的文件名部分：archive 可能含子目录相对路径
    // （如 `embeddings/alba.safetensors`），直接拼接会落到未创建的子目录里
    let file_stem = std::path::Path::new(&asset.archive)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| asset.archive.clone());
    let tmp = parent.join(format!(".{file_stem}.tmp"));

    download_to(&asset.source, &tmp, asset.size_bytes, on_progress, cancel)?;
    if cancelled(cancel) {
        let _ = std::fs::remove_file(&tmp);
        return Err(ModelError::Cancelled);
    }

    on_progress(progress(
        DownloadStage::Verifying,
        -1.0,
        dest_path,
        "校验 sha256",
    ));
    verify_sha256(&tmp, &asset.sha256)?;

    // 原子落位：目标已存在先移除（Windows 上 rename 覆盖文件可能失败）。
    if dest_path.exists() {
        std::fs::remove_file(dest_path)?;
    }
    std::fs::rename(&tmp, dest_path)?;

    on_progress(progress(
        DownloadStage::Done,
        100.0,
        dest_path,
        "模型安装完成",
    ));
    Ok(())
}

fn progress(
    stage: DownloadStage,
    percent: f64,
    _dest_dir: &Path,
    message: &str,
) -> DownloadProgress {
    DownloadProgress {
        stage,
        percent,
        bytes_downloaded: 0,
        total_bytes: 0,
        message: message.to_string(),
    }
}

/// 流式下载到临时文件，带进度回调；中断后从断点续传，重试 5 次（指数退避）。
///
/// `cancel` 命中时立即返回 [`ModelError::Cancelled`]（不重试），并删除临时文件。
/// `pub(crate)`：模型库安装（model_library）复用同一流式核心。
pub(crate) fn download_to(
    url: &str,
    tmp_archive: &Path,
    manifest_total: u64,
    on_progress: &mut ProgressFn,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(), ModelError> {
    let mut last_err: Option<ModelError> = None;
    for attempt in 0..5 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(400 * (1 << attempt)));
        }
        match try_download_once(url, tmp_archive, manifest_total, on_progress, cancel) {
            Ok(()) => return Ok(()),
            Err(e) => {
                if matches!(e, ModelError::Cancelled) {
                    let _ = std::fs::remove_file(tmp_archive);
                    return Err(e);
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err.map_or_else(
        || ModelError::Download("未知错误".to_string()),
        |e| ModelError::Download(e.to_string()),
    ))
}

fn try_download_once(
    url: &str,
    tmp_archive: &Path,
    manifest_total: u64,
    on_progress: &mut ProgressFn,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(), ModelError> {
    try_download_once_core(url, None, tmp_archive, manifest_total, on_progress, cancel)
}

/// 下载用 HTTP Agent：
/// - TLS 根证书取系统证书存储（`PlatformVerifier`）：杀软 / 代理的 HTTPS 解密
///   证书只装在系统存储，默认打包根证书（WebPki）会报 UnknownIssuer 或被掐断；
/// - 读取 `ALL_PROXY` / `HTTPS_PROXY` / `HTTP_PROXY`（含 `NO_PROXY` 排除规则）
///   环境变量，代理用户无须 TUN 即可生效；未设置时直连。
fn download_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .proxy(ureq::Proxy::try_from_env())
        .build();
    ureq::Agent::new_with_config(config)
}

/// 解析 `Content-Range: bytes 100-200/345` 的总量（345）；`*/345` 或无效 → `None`。
fn content_range_total(v: &str) -> Option<u64> {
    v.rsplit('/').next()?.trim().parse().ok()
}

fn try_download_once_core(
    url: &str,
    token: Option<&str>,
    tmp_archive: &Path,
    manifest_total: u64,
    on_progress: &mut ProgressFn,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(), ModelError> {
    // 断点续传：上次中断留下的部分文件非空时，请求剩余区间（服务器以 206 应答）
    let resume_from = std::fs::metadata(tmp_archive).map_or(0, |m| m.len());
    let mut req = download_agent().get(url);
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    if resume_from > 0 {
        req = req.header("Range", &format!("bytes={resume_from}-"));
    }
    let resp = req.call().map_err(|e| {
        // 416 = 断点越界（远端内容已变化等）→ 丢弃部分文件，下次重试从 0 开始
        if matches!(&e, ureq::Error::StatusCode(416)) {
            let _ = std::fs::remove_file(tmp_archive);
        }
        ModelError::Http(e.to_string())
    })?;

    let content_length = resp
        .headers()
        .get("Content-Length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let range_total = resp
        .headers()
        .get("Content-Range")
        .and_then(|v| v.to_str().ok())
        .and_then(content_range_total);
    // 206 = 续传生效（追加写入）；200 = 服务器忽略 Range（覆盖重下）
    let resumed = resp.status() == 206;
    let total = if resumed {
        // Content-Range 总量最准；缺失时用「断点 + 剩余长度」；仍未知退回清单值
        range_total.or_else(|| content_length.map(|len| resume_from.saturating_add(len)))
    } else {
        content_length
    }
    .filter(|&t| t > 0)
    .unwrap_or(manifest_total);

    let mut reader = resp.into_body().into_reader();
    let mut file = if resumed {
        std::fs::OpenOptions::new().append(true).open(tmp_archive)?
    } else {
        std::fs::File::create(tmp_archive)?
    };
    let mut buf = [0u8; 64 * 1024];
    let mut done: u64 = if resumed { resume_from } else { 0 };
    loop {
        if cancelled(cancel) {
            let _ = std::fs::remove_file(tmp_archive);
            return Err(ModelError::Cancelled);
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        let percent = if total > 0 {
            ((done as f64 / total as f64) * 100.0).min(100.0)
        } else {
            -1.0
        };
        on_progress(DownloadProgress {
            stage: DownloadStage::Downloading,
            percent,
            bytes_downloaded: done,
            total_bytes: total,
            message: format!("下载中 {:.1}%", percent.max(0.0)),
        });
    }
    file.flush()?;
    Ok(())
}

/// 对临时压缩包整包计算 sha256 并比对；不匹配则删除损坏文件并报错。
/// `pub(crate)`：HF 下载文件完整性校验复用。
pub(crate) fn verify_sha256(path: &Path, expected: &str) -> Result<(), ModelError> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != expected {
        let _ = std::fs::remove_file(path);
        return Err(ModelError::Sha256Mismatch {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}

/// 解压 tar.bz2 到同父目录临时目录，再把顶层模型目录原子移到目标位置。
fn extract_and_place(tmp_archive: &Path, dest_dir: &Path) -> Result<(), ModelError> {
    let parent = dest_dir
        .parent()
        .ok_or_else(|| ModelError::Extract("目标目录缺少父目录".to_string()))?;
    let name = dest_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let tmp_extract = parent.join(format!(".{name}.extract"));
    std::fs::create_dir_all(&tmp_extract)?;

    let file = std::fs::File::open(tmp_archive)?;
    let bz = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(bz);
    archive
        .unpack(&tmp_extract)
        .map_err(|e| ModelError::Extract(e.to_string()))?;

    // 定位顶层模型目录：优先 <name>，否则退化为唯一的顶层项（兼容不同包内布局）。
    let src = tmp_extract.join(&name);
    let src = if src.is_dir() {
        src
    } else {
        let mut entries = std::fs::read_dir(&tmp_extract)?.filter_map(Result::ok);
        let top = entries
            .next()
            .map(|e| e.path())
            .ok_or_else(|| ModelError::Extract("压缩包内容为空".to_string()))?;
        if entries.next().is_some() {
            return Err(ModelError::Extract(
                "压缩包顶层存在多个目录，无法确定模型根目录".to_string(),
            ));
        }
        top
    };

    // 原子落位：目标已存在先移除（Windows 上 rename 覆盖目录会失败）。
    if dest_dir.exists() {
        std::fs::remove_dir_all(dest_dir)?;
    }
    std::fs::rename(&src, dest_dir)?;
    std::fs::remove_dir_all(&tmp_extract)?;
    let _ = std::fs::remove_file(tmp_archive);
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::net::TcpListener;

    /// 测试归档内的占位模型文件（形状自定，仅需与安装校验清单一致；
    /// 真实清单单源 audiocpp 族表，见 `registry::required_files_for_role`）。
    pub(crate) const TEST_MODEL_FILES: [&str; 2] = ["encoder.onnx", "tokens.txt"];

    pub(crate) fn mini_tarbz2(prefix: &str) -> Vec<u8> {
        tarbz2_with(prefix, &TEST_MODEL_FILES)
    }

    /// 归档内容可指定的变体（调用方按其完整性清单摆文件）。
    pub(crate) fn tarbz2_with(prefix: &str, files: &[&str]) -> Vec<u8> {
        use bzip2::Compression;
        use bzip2::write::BzEncoder;
        let mut bz = BzEncoder::new(Vec::new(), Compression::default());
        {
            let mut ar = tar::Builder::new(&mut bz);
            let base = format!("{prefix}/");
            let mut dir = tar::Header::new_gnu();
            dir.set_entry_type(tar::EntryType::Directory);
            dir.set_size(0);
            dir.set_mode(0o755);
            dir.set_username("test").unwrap();
            dir.set_groupname("test").unwrap();
            dir.set_cksum();
            ar.append_data(&mut dir, &base, std::io::empty()).unwrap();

            let mut f = |rel: &str, bytes: &[u8]| {
                let mut h = tar::Header::new_gnu();
                h.set_size(bytes.len() as u64);
                h.set_mode(0o644);
                h.set_username("test").unwrap();
                h.set_groupname("test").unwrap();
                h.set_cksum();
                ar.append_data(&mut h, format!("{base}{rel}"), bytes)
                    .unwrap();
            };
            for (i, name) in files.iter().enumerate() {
                f(name, format!("payload-{i}").as_bytes());
            }
            ar.finish().unwrap();
        }
        bz.finish().unwrap()
    }

    /// 起一个本地 HTTP 服务，每个连接都返回给定字节，返回请求 URL。
    pub(crate) fn serve_many(bytes: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let payload = std::sync::Arc::new(bytes);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut sock) = stream else { break };
                let payload = payload.clone();
                std::thread::spawn(move || {
                    let mut buf = [0u8; 1024];
                    let _ = sock.read(&mut buf);
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        payload.len()
                    );
                    let _ = sock.write_all(head.as_bytes());
                    let _ = sock.write_all(&payload);
                });
            }
        });
        format!("http://{addr}/model.tar.bz2")
    }

    fn test_asset(source: &str, sha256: &str, archive: &str) -> ModelAsset {
        ModelAsset {
            name: "test-model".to_string(),
            role: "test-role".to_string(),
            version: "test".to_string(),
            kind: None,
            archive: archive.to_string(),
            source: source.to_string(),
            sha256: sha256.to_string(),
            size_bytes: 0,
            license: "Apache-2.0".to_string(),
        }
    }

    pub(crate) fn sha256_hex(data: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(data))
    }

    /// 清单自洽：只含 qwen3 三资产，全部为裸单文件 GGUF，校验字段齐全。
    #[test]
    fn test_manifest_qwen3_assets() {
        assert_eq!(
            manifest_roles(),
            [
                "tts-audiocpp-qwen3-06b",
                "tts-audiocpp-qwen3-17b",
                "asr-audiocpp-qwen3-06b"
            ],
            "模型清单收敛为 qwen3 三资产（KWS/标点/sherpa TTS/声纹/omnivoice/voxcpm2 已移除）"
        );
        for role in manifest_roles() {
            let a = asset_by_role(role).unwrap();
            assert!(!a.name.is_empty());
            assert!(a.source.starts_with("http"), "{role} 需有下载源");
            assert_eq!(a.sha256.len(), 64, "{role} 需有 sha256");
            assert!(a.is_raw(), "{role} 应为裸单文件资产（audio.cpp GGUF）");
            assert_eq!(asset_by_role(role).unwrap().archive, a.archive);
        }
    }

    #[test]
    fn test_verify_sha256_ok_and_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("blob");
        std::fs::write(&p, b"hello").unwrap();
        assert!(verify_sha256(&p, &sha256_hex(b"hello")).is_ok());
        // 错误校验值：报错且删除损坏文件
        let p2 = dir.path().join("bad");
        std::fs::write(&p2, b"hello").unwrap();
        let err = verify_sha256(&p2, &"0".repeat(64)).unwrap_err();
        assert!(matches!(err, ModelError::Sha256Mismatch { .. }));
        assert!(!p2.exists());
    }

    #[test]
    fn test_extract_and_place_mini_archive() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("mini.tar.bz2");
        std::fs::write(&archive, mini_tarbz2("test-model")).unwrap();
        let dest = dir.path().join("test-model");
        extract_and_place(&archive, &dest).unwrap();
        assert!(has_required_files(&dest, &TEST_MODEL_FILES));
        assert!(!archive.exists());
        assert!(!dir.path().join(".test-model.extract").exists());
    }

    #[test]
    fn test_install_full_flow_via_local_server() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = mini_tarbz2("test-model");
        let url = serve_many(bytes.clone());
        let asset = test_asset(&url, &sha256_hex(&bytes), "mini.tar.bz2");

        let dest = dir.path().join("test-model");
        let mut stages = Vec::new();
        install_asset_to(
            &asset,
            &dest,
            false,
            &mut |p| stages.push(p.stage),
            &TEST_MODEL_FILES,
        )
        .unwrap();
        assert!(has_required_files(&dest, &TEST_MODEL_FILES));

        let expected = [
            DownloadStage::Downloading,
            DownloadStage::Verifying,
            DownloadStage::Extracting,
            DownloadStage::Done,
        ];
        assert_eq!(stages, expected);
    }

    #[test]
    fn test_install_idempotent_skips_when_installed() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("test-model");
        // 直接摆好必需文件，模拟已安装
        std::fs::create_dir_all(&dest).unwrap();
        for f in TEST_MODEL_FILES {
            std::fs::write(dest.join(f), b"x").unwrap();
        }

        let mut stages = Vec::new();
        install_asset_to(
            &test_asset(
                "http://127.0.0.1:1/none.tar.bz2",
                &"0".repeat(64),
                "mini.tar.bz2",
            ),
            &dest,
            false,
            &mut |p| stages.push(p.stage),
            &TEST_MODEL_FILES,
        )
        .unwrap();
        assert_eq!(stages, vec![DownloadStage::Done]);
    }

    #[test]
    fn test_install_raw_file_via_local_server() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"gguf-bytes".to_vec();
        let url = serve_many(bytes.clone());
        let mut asset = test_asset(&url, &sha256_hex(&bytes), "qwen3-test-q8_0.gguf");
        asset.kind = Some("raw".to_string());

        let dest = dir.path().join("qwen3-test-q8_0.gguf");
        let mut stages = Vec::new();
        install_raw_file_to_cancellable(&asset, &dest, false, &mut |p| stages.push(p.stage), None)
            .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), bytes);

        let expected = [
            DownloadStage::Downloading,
            DownloadStage::Verifying,
            DownloadStage::Done,
        ];
        assert_eq!(stages, expected);

        // 幂等：已装且非 force → 仅 Done
        let mut stages = Vec::new();
        install_raw_file_to_cancellable(&asset, &dest, false, &mut |p| stages.push(p.stage), None)
            .unwrap();
        assert_eq!(stages, vec![DownloadStage::Done]);
    }

    #[test]
    fn test_install_force_reinstalls() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = mini_tarbz2("test-model");
        let url = serve_many(bytes.clone());
        let asset = test_asset(&url, &sha256_hex(&bytes), "mini.tar.bz2");
        let dest = dir.path().join("test-model");

        // 先装好，再 force 重装 → 应重新走完整流程
        install_asset_to(&asset, &dest, false, &mut |_| {}, &TEST_MODEL_FILES).unwrap();
        let mut stages = Vec::new();
        install_asset_to(
            &asset,
            &dest,
            true,
            &mut |p| stages.push(p.stage),
            &TEST_MODEL_FILES,
        )
        .unwrap();
        assert!(has_required_files(&dest, &TEST_MODEL_FILES));
        assert!(stages.contains(&DownloadStage::Downloading));
    }

    #[test]
    fn test_install_sha256_mismatch_errors() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = mini_tarbz2("test-model");
        let url = serve_many(bytes);
        let asset = test_asset(&url, &"0".repeat(64), "mini.tar.bz2");
        let dest = dir.path().join("test-model");
        let err =
            install_asset_to(&asset, &dest, false, &mut |_| {}, &TEST_MODEL_FILES).unwrap_err();
        assert!(matches!(err, ModelError::Sha256Mismatch { .. }));
        assert!(!dest.exists());
    }

    /// 慢速分块下载服务器，便于在下载中途触发取消。
    fn serve_slow(step_ms: u64, payload: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let payload = std::sync::Arc::new(payload);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut sock) = stream else { break };
                let payload = payload.clone();
                std::thread::spawn(move || {
                    let mut buf = [0u8; 1024];
                    let _ = sock.read(&mut buf);
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        payload.len()
                    );
                    let _ = sock.write_all(head.as_bytes());
                    let mut sent = 0usize;
                    while sent < payload.len() {
                        let end = (sent + 8192).min(payload.len());
                        let _ = sock.write_all(&payload[sent..end]);
                        sent = end;
                        std::thread::sleep(std::time::Duration::from_millis(step_ms));
                    }
                });
            }
        });
        format!("http://{addr}/slow.tar.bz2")
    }

    #[test]
    fn test_install_cancel_cleans_tmp_and_can_redo() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let dir = tempfile::tempdir().unwrap();
        let payload = vec![0u8; 512 * 1024];
        let url = serve_slow(15, payload);
        let asset = test_asset(&url, &"0".repeat(64), "slow.tar.bz2");
        let dest = dir.path().join("test-model");
        let parent = dest.parent().unwrap();
        let tmp_path = parent.join(".slow.tar.bz2.tmp");

        // 收到第一个中间进度后立即取消（确定性，避免时序竞态）
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        let asset_for_thread = asset.clone();
        let dest_for_thread = dest.clone();
        let handle = std::thread::spawn(move || {
            let mut stages = Vec::new();
            let result = install_asset_to_cancellable(
                &asset_for_thread,
                &dest_for_thread,
                false,
                &mut |p| {
                    if p.percent > 0.0 && p.percent < 100.0 {
                        let _ = tx.send(p.percent);
                    }
                    stages.push(p.percent);
                },
                &TEST_MODEL_FILES,
                Some(&cancel_clone),
            );
            (result, stages)
        });

        // 等到确实观察到中间进度后才取消
        let _first = rx
            .recv_timeout(std::time::Duration::from_secs(15))
            .expect("慢速服务器应产生中间进度");
        cancel.store(true, Ordering::Relaxed);
        let (result, stages) = handle.join().unwrap();

        assert!(matches!(result, Err(ModelError::Cancelled)));
        // 确实观察到了中间进度
        assert!(stages.iter().any(|&p| p > 0.0 && p < 100.0));
        // 取消后临时文件与正式目录都被清理
        assert!(!tmp_path.exists());
        assert!(!dest.exists());

        // 取消后重新下载（cancel 复位）能正常开始并完成
        cancel.store(false, Ordering::Relaxed);
        let bytes = mini_tarbz2("test-model");
        let url2 = serve_many(bytes.clone());
        let asset2 = test_asset(&url2, &sha256_hex(&bytes), "mini.tar.bz2");
        let mut stages2 = Vec::new();
        install_asset_to_cancellable(
            &asset2,
            &dest,
            false,
            &mut |p| stages2.push(p.stage),
            &TEST_MODEL_FILES,
            Some(&cancel),
        )
        .unwrap();
        assert!(has_required_files(&dest, &TEST_MODEL_FILES));
    }

    /// 多资产总体进度聚合：各 asset 下载字节累计后总体百分比单调不减。
    #[test]
    fn test_aggregate_overall_percent_monotonic() {
        // 与 install 聚合公式一致：overall = (累计已完成 + 当前 asset 字节) / 总字节
        let total: u64 = 3000;
        let mut done: u64 = 0;
        let mut prev: f64 = -1.0;
        let mut monotonic = true;
        let mut last_overall: f64 = 0.0;
        for size in [1000u64, 1000, 1000] {
            for &step in &[0u64, 500, size] {
                let cur = done + step;
                let overall = ((cur as f64 / total as f64) * 100.0).min(100.0);
                if overall < prev {
                    monotonic = false;
                }
                prev = overall;
                last_overall = overall;
            }
            done += size;
        }
        assert!(monotonic, "总体进度不能倒退");
        assert!((last_overall - 100.0).abs() < 1e-9);
    }

    // ---- 断点续传 ----

    #[test]
    fn test_content_range_total() {
        assert_eq!(content_range_total("bytes 100-200/345"), Some(345));
        assert_eq!(content_range_total("bytes */345"), Some(345));
        assert_eq!(content_range_total("bytes */*"), None);
        assert_eq!(content_range_total("garbage"), None);
    }

    /// 手动网络探针：用真实下载 Agent 打模型清单源，验证系统证书 + 代理链路。
    /// 平时忽略（不联网）；诊断时运行 `cargo test -- --ignored probe_model_url`。
    #[test]
    #[ignore = "联网探针，诊断时手动运行"]
    fn probe_model_url() {
        let url = asset_by_role(manifest_roles()[0]).unwrap().source.clone();
        let range = "bytes=0-1023";
        let resp = download_agent()
            .get(&url)
            .header("Range", range)
            .call()
            .expect("请求失败（TLS/网络）");
        assert!(resp.status().is_success(), "HTTP {}", resp.status());
        let bytes = resp.into_body().read_to_vec().unwrap();
        assert_eq!(bytes.len(), 1024, "Range 请求应返回 1024 字节");
    }

    /// 支持 Range 的「弱网」服务器：每次连接只发送 max_bytes 字节就提前断开
    /// （响应头声明 Content-Length 但未发完即关闭，客户端读到 early EOF），
    /// 只有完整区间才正常收尾。用于驱动断点续传重试。
    fn serve_flaky_resumable(payload: Vec<u8>, max_bytes_per_conn: usize) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let payload = std::sync::Arc::new(payload);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut sock) = stream else { break };
                let payload = payload.clone();
                std::thread::spawn(move || {
                    // 读完请求头（可能跨多个 TCP 段）
                    let mut request: Vec<u8> = Vec::new();
                    let mut buf = [0u8; 4096];
                    loop {
                        let n = sock.read(&mut buf).unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        request.extend_from_slice(&buf[..n]);
                        if request.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    let text = String::from_utf8_lossy(&request);
                    let start = text
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("range: bytes=")
                                .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if start >= payload.len() {
                        let _ = sock.write_all(
                            b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                        return;
                    }
                    let end = (start + max_bytes_per_conn).min(payload.len());
                    let remaining = payload.len() - start;
                    if start == 0 {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            payload.len()
                        );
                        let _ = sock.write_all(head.as_bytes());
                    } else {
                        let head = format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{remaining}\r\nContent-Length: {remaining}\r\nConnection: close\r\n\r\n",
                            payload.len() - 1
                        );
                        let _ = sock.write_all(head.as_bytes());
                    }
                    let _ = sock.write_all(&payload[start..end]);
                    let _ = sock.flush();
                });
            }
        });
        format!("http://{addr}/flaky.tar.bz2")
    }

    #[test]
    fn test_download_resumes_across_disconnects() {
        let dir = tempfile::tempdir().unwrap();
        // 512KB、每连接最多 192KB：两次中断后第三次连接应完成
        let payload: Vec<u8> = (0..512 * 1024).map(|i| (i % 251) as u8).collect();
        let url = serve_flaky_resumable(payload.clone(), 192 * 1024);
        let tmp = dir.path().join(".flaky.tar.bz2.tmp");
        let mut last_percent = -1.0;
        download_to(
            &url,
            &tmp,
            payload.len() as u64,
            &mut |p| last_percent = p.percent,
            None,
        )
        .unwrap();
        assert_eq!(std::fs::read(&tmp).unwrap(), payload);
        assert_eq!(last_percent, 100.0);
    }
}
