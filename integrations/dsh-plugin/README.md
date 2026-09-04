# @zapmomo-ai/dsh-plugin

dsh（[DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)）→ ZapMomo 桌宠的**任务事件桥**。

> 面向用户的使用说明见[文档站：deepseek-harness 集成（dsh 桥）](../../docs/content/docs/desktop-app/dsh-bridge.mdx)；本篇是面向开发的实现与发布细节。

监听 dsh 的 `agent/status` 与 `session/event`（`turn/end`），把任务**开始 / 完成 / 失败 / 中断**翻译成语义化事件，POST 到 ZapMomo 的 loopback 桥（`POST /dsh/events`）。ZapMomo 桌宠据此以文字气泡 + 语音播报给用户反馈。

## 前置条件

- 已运行 **ZapMomo**（Tauri 应用，`[dsh]` 段默认启用）：ZapMomo 启动时在 `~/.zapmomo/runtime/dsh-bridge.json` 写桥端口与 Bearer token（权限 0600）
- 已安装 **deepseek-harness**（`dsh` CLI）

插件只在读到发现文件时才推送；ZapMomo 未运行时静默跳过（不报错、不影响 dsh）。

## 安装

```bash
# 从 npm（推荐）
dsh plugin --profile web add @zapmomo-ai/dsh-plugin

# 从源码（开发/调试，link 模式改动即时生效；<zapmomo> 换成你的仓库路径）
dsh plugin --profile web add link:<zapmomo>/integrations/dsh-plugin
```

装完因声明了 `dsh.bundle.patch`，插件**自动**进入 profile bundles，无需手动改 patch。重启 `dsh web` 生效。

## 行为

| dsh 事件 | 推送类型 | 备注 |
| --- | --- | --- |
| `agent/status` → `running` | `task-started` | title 取自会话最近一条 user/message（前 40 字符） |
| `turn/end` → `reason.kind = completed` | `task-finished` | |
| `turn/end` → `reason.kind = error` | `task-failed` | detail = `reason.error.message — code`（截 200） |
| `turn/end` → 其它（aborted/interrupted/max-tokens/blocked） | `task-interrupted` | |

推送纪律：每次发送前现读发现文件；POST 超时 1s、异常吞掉只打 debug——**绝不阻塞/影响 dsh 宿主**。

## 开发

```bash
# 本地调试用 link 模式安装（改动即时生效）
dsh plugin --profile web add link:<zapmomo>/integrations/dsh-plugin
```

字段路径按 dsh 源码核对（`session.id` / `agent.status` / `reason.error`），如有 dsh 版本升级导致漂移，以 `src/index.ts` 顶部「待实测核对」注释为准回读对齐。

## 发布

发布走 **npm Trusted Publishing（OIDC）**，由 `.github/workflows/npm-publish.yml` 自动完成，**不需要 NPM_TOKEN / OTP**。

### npm 侧前置（一次性）

在 npmjs.com 为 `@zapmomo-ai/dsh-plugin` 配置 Trusted Publisher，必须匹配：

| 字段 | 值 |
| --- | --- |
| Repository | `shenjingnan/zapmomo` |
| **Workflow filename** | `npm-publish.yml` |
| **Environment** | **留空** |

### 发新版本（tag 触发自动发布）

```bash
# 1. 递增版本
cd integrations/dsh-plugin && vim package.json    # version: 0.1.0 → 0.2.0

# 2. 提交 + 打 tag + 推送（workflow 收到 dsh-plugin-v* tag 即自动发布）
git add integrations/dsh-plugin/package.json
git commit -m "chore(dsh): bump version to 0.2.0"
git tag dsh-plugin-v0.2.0
git push origin dsh-plugin-v0.2.0
```

> 注意：tag 名必须 `dsh-plugin-v<版本>`（workflow 只匹配该前缀）。若 package.json 版本未变，`npm publish` 会被 npm 以 `E403 already published` 拒绝——正常保护，改对版本再推。

### 手动发布（临时/紧急）

GitHub → Actions → **npm Publish (dsh-plugin)** → **Run workflow**（不 push 任何东西）。

### 安全建议

Trusted Publisher 生效后，可在 npm 包设置里把发布方式设为 **「Require 2FA and disallow tokens」**——彻底禁掉 token 发布，只走 OIDC。
