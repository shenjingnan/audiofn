---
name: update-readme
description: 根据项目当前状态更新根目录 README.md（用户视角）与 CONTRIBUTING.md（开发视角）
---

# Update README Command

根据项目当前状态更新根目录 `README.md` 与 `CONTRIBUTING.md`。

## 文档分工

- **README.md 面向最终用户**：只保留用户可见内容（特性、下载安装、应用内使用说明、macOS 未签名处理、高级配置简介、许可）。
  **禁止出现开发命令**（`cargo run` / `cargo test` / `cargo fmt` / `pnpm tauri dev` 之类）、项目结构树、依赖表、发布流程。
- **CONTRIBUTING.md 面向贡献者**：环境搭建、CLI 命令参考（kws / asr / tts / llm / voice）、完整配置段参考、桌面应用开发、项目结构、依赖说明、发布流程、Git 工作流。

## 执行步骤

1. 读取 `Cargo.toml` 获取项目名称、版本号、描述、依赖等信息
2. 读取 `src/` 目录下的源码，了解项目当前导出的公开 API（结构体、函数、trait 等）
3. 读取当前的 `README.md` 与 `CONTRIBUTING.md`，对比现状识别需要更新的内容
4. 按需更新：
   - **README.md**：项目描述（与 `Cargo.toml` 的 description 一致）、用户特性列表、下载/上手步骤
   - **CONTRIBUTING.md**：技术栈（Rust 版本、关键依赖）、快速开始、CLI 命令（`cargo run -- <command>` 格式）、各功能配置段、项目结构（与实际目录一致）
5. 运行格式检查: `cargo fmt --check`

## 更新原则

- 只更新与实际代码/配置不符的内容，不重写整个文件
- 保留用户自定义的章节和内容
- 保持文档风格与现有内容一致
- 如果用户指定了特定章节，只更新指定部分

## 注意事项

- 不要添加与项目无关的具体业务描述
- 所有命令示例使用 `cargo`，且只写入 CONTRIBUTING.md
- 新增开发向内容（命令、构建、测试、发布）一律进 CONTRIBUTING.md，README 仅保留指向它的链接
