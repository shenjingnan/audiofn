# 第三方模型声明

本目录下的模型文件由第三方提供，**不随代码分发**，由 CLI 的 `install-model` 命令
（GUI 模型库下载按钮同链路）按 `manifest.json` 清单按需下载。清单记录了来源 URL
与 sha256 校验和。

## qwen3-tts-12hz-0.6b-base-q8_0.gguf（audio.cpp TTS）

- **用途**: 文本转语音 + 零样本音色克隆（TTS，Qwen3-TTS 12Hz Base 0.6B，q8_0 量化，
  24kHz），由内置的 audio.cpp 引擎（sidecar 进程，Metal 后端）加载
- **来源**: https://huggingface.co/audio-cpp/audio.cpp-gguf/resolve/main/Qwen3-TTS-12Hz-0.6B-Base-GGUF/qwen3-tts-12hz-0.6b-base-q8_0.gguf
- **打包仓库**: https://huggingface.co/audio-cpp/audio.cpp-gguf（Qwen3-TTS-12Hz-0.6B-Base-GGUF）
- **上游模型**: https://huggingface.co/Qwen/Qwen3-TTS-12Hz-0.6B-Base
- **发布方**: audio-cpp（GGUF 打包）；模型源自 Qwen（阿里通义）
- **许可证**: Apache-2.0
- **sha256**: `771420bd20ff5f35407b4fa9cf9c5461e153800d3d772ef51c9febc0a520855d`

## qwen3-tts-12hz-1.7b-base-q8_0_v2.gguf（audio.cpp TTS）

- **用途**: 文本转语音 + 零样本音色克隆（TTS，Qwen3-TTS 12Hz Base 1.7B，q8_0 量化，
  `_v2` 打包，24kHz），质量优先变体，由内置的 audio.cpp 引擎（sidecar 进程，
  Metal 后端）加载
- **来源**: https://huggingface.co/audio-cpp/audio.cpp-gguf/resolve/main/Qwen3-TTS-12Hz-1.7B-Base-GGUF/qwen3-tts-12hz-1.7b-base-q8_0_v2.gguf
- **打包仓库**: https://huggingface.co/audio-cpp/audio.cpp-gguf（Qwen3-TTS-12Hz-1.7B-Base-GGUF）
- **上游模型**: https://huggingface.co/Qwen/Qwen3-TTS-12Hz-1.7B-Base
- **发布方**: audio-cpp（GGUF 打包）；模型源自 Qwen（阿里通义）
- **许可证**: Apache-2.0
- **sha256**: `b55e06c7890d43c208d15aed8b4ed3f18215f295e47d5960e061b15bff338ab0`

## qwen3-asr-0.6b-q8_0.gguf（audio.cpp ASR）

- **用途**: 离线语音转写（ASR，Qwen3-ASR 0.6B，q8_0 量化，29 语言自动识别，
  不支持热词），由内置的 audio.cpp 引擎（sidecar 进程，Metal 后端）加载
- **来源**: https://huggingface.co/audio-cpp/audio.cpp-gguf/resolve/main/Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf
- **打包仓库**: https://huggingface.co/audio-cpp/audio.cpp-gguf（Qwen3-ASR-0.6B-GGUF）
- **上游模型**: https://huggingface.co/Qwen/Qwen3-ASR-0.6B
- **发布方**: audio-cpp（GGUF 打包）；模型源自 Qwen（阿里通义）
- **许可证**: Apache-2.0
- **sha256**: `6c44ec2fb4cee513892d7863c1fcc3ea6b699ffa4d899b0ef4ab19956d9544f7`

## audiocpp_server（audio.cpp 引擎二进制，随安装包分发）

- **用途**: 语音推理第二后端（ggml 系 audio.cpp 的 HTTP server sidecar，裁剪构建
  仅含 Qwen3-TTS / Qwen3-ASR 模型族；编译参数见 `.github/workflows/release.yml`）
- **来源**: https://github.com/0xShug0/audio.cpp（版本 pin 见 release.yml 的 AUDIOCPP_REF）
- **发布方**: ShugoAI LLC（audio.cpp 项目）
- **许可证**: Apache-2.0（上游 LICENSE 版权声明 `Copyright 2026 ShugoAI LLC`）。
  随本项目以独立进程形式聚合分发，Apache-2.0 与 GPL-3.0 兼容；本文件即其
  NOTICE 性质的声明

## hound（Rust crate，wav 解码）

- **用途**: audio.cpp sidecar 返回的 wav 字节解码为 PCM 样本（纯 Rust 零传递依赖）
- **来源**: https://github.com/ruuda/hound（crates.io: https://crates.io/crates/hound）
- **发布方**: Ruud van Asseldonk
- **许可证**: Apache-2.0（crates.io 发布元数据与 crate 内 `license` 文件）
- **分发方式**: 编译期静态链接进本应用二进制，不以文件形式分发其副本
