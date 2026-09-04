# 文档站首页 hero 落地页 —— 技术方案

日期：2026-08-21
参考：Tailwind UI「Simple centered」hero；现有 `docs/lib/layout.shared.tsx`（`baseOptions`）、`docs/app/docs/layout.tsx`（DocsLayout 模式）

## 1. 背景与现状

ZapMomo 是开源桌面 AI 伴侣（Tauri 2，三平台安装包发布到 GitHub Releases）。仓库内 `docs/` 是 Fumadocs 16 + Next.js 16 + Tailwind 4 的文档站（`next.config.mjs` 设 `output: 'export'` 静态导出，部署 Cloudflare Pages），但首页 `docs/app/page.tsx` 目前仅 `redirect('/docs')`，没有真正的 landing page。

- Fumadocs 16 **无内置 Hero 组件**；`HomeLayout`（`fumadocs-ui/layouts/home`）自带导航 / 搜索按钮 / 主题切换，props 复用 `BaseLayoutProps`，可直接传 `baseOptions()`。
- 下载直链已在根 `README.md`「下载桌面应用」维护（5 平台主链 + MSI/AppImage 可选），全部指向 `releases/latest/download/<file>`，无需登录 GitHub。
- `dark:` 变体由 fumadocs `base.css` 已定义的 `@variant dark (&:where(.dark, .dark *))` 提供（基于 class），**无需**在 `global.css` 重复定义。
- docs 站目前无任何测试框架（无 vitest/jest）；`vitest@^4.1.10` 已在 workspace `pnpm-lock.yaml`（`src-tauri/frontend` 引入）。

## 2. 目标与非目标

### 目标

- 首页渲染 centered hero：announcement 徽章 + 大标题 + 一句话卖点 + 下载区 + 上下模糊渐变 blob。
- 下载区：macOS / Windows / Linux 三个按钮横向平铺，各自下拉选择安装包。
- hero 下方：假浏览器窗口框内嵌应用截图（`/screenshots/home.png`）+ 简洁 footer。
- 复用 HomeLayout 导航，与 `/docs` 风格统一；明暗模式正确。

### 非目标

- 不做版本 / 渠道选择器（只认 latest）。
- 不做 i18n（全站 zh-CN）。
- 不改造 `/docs` 内部布局。
- 不引入 lucide-react 等新运行时依赖（图标全用内联 SVG）。

### 已确认的决策

| 决策点 | 结论 |
| --- | --- |
| 落点 | `docs/app/page.tsx` 直接包 `HomeLayout`，移除 redirect（无需 (home) 路由组；`/docs` 的 DocsLayout 不冲突） |
| 导航 | 复用 `baseOptions()`，首页覆盖 `nav.title` 为 Logo + 文字 |
| 下载入口 | 三平台按钮横向平铺，各自下拉；**无环境感知**（不做 UA 检测） |
| 版本号展示 | 不放进下载按钮；announcement「查看版本」跳 Releases 页查看 |
| 展开区 | React state + `button[aria-expanded]`；不用 `<details>`/`<dialog>`（参考代码的 Tailwind Plus 元素脚本 `command="show-modal"` 禁用） |
| 测试 | docs 包引入最小 vitest（node 环境，只测 `lib/**/*.test.ts`） |

## 3. 技术方案

### 3.1 文件与组件结构

```
docs/
├─ app/
│  └─ page.tsx                     # 改：redirect → metadata + <HomeLayout>
├─ components/home/
│  ├─ logo-mark.tsx                # Server：nav.title 用 <img /favicon.svg> + 文字
│  ├─ home-hero.tsx                # Server：徽章/h1/副标题/blob + 渲染 DownloadSection
│  ├─ download-section.tsx         # Client：UA 检测 + 主按钮 + 展开区（核心交互）
│  ├─ platform-icons.tsx           # 无状态：内联 SVG（Windows/Apple/Linux/Chevron/Download）
│  ├─ screenshot-section.tsx       # Server：假浏览器窗口框 + home.png
│  └─ home-footer.tsx              # Server：GitHub/文档/Releases/许可证
├─ lib/
│  ├─ downloads.ts                 # 纯 TS：PLATFORMS + detectPlatform() + URL
│  └─ downloads.test.ts            # vitest 单测
├─ package.json                    # + test script + vitest devDep
└─ vitest.config.ts                # node 环境，include lib/**/*.test.ts
```

### 3.2 UA 检测纯函数（`lib/downloads.ts`）

```ts
export function detectPlatform(input: {
  ua: string; platform?: string; arch?: string;
}): PlatformKey  // 'windows-x64' | 'macos-arm64' | 'macos-x64' | 'linux-x64' | 'unknown'
```

分支（优先级从上到下）：

| 优先级 | 条件 | 返回 |
| --- | --- | --- |
| 0 | UA 含 `android\|iphone\|ipad\|ipod` | `unknown` |
| 1 | `platform === 'windows'` | `windows-x64` |
| 2 | `platform === 'linux'` | `linux-x64` |
| 3 | macOS（`platform==='macos'\|'macintel'` 或 UA 含 `mac os x\|macintosh`）：`arch==='arm'`→arm64；`arch==='x86'`→x64；**arch 缺失默认 arm64** | macOS 分支 |
| 4 | 兜底 | `unknown` |

**macOS 判别关键结论**：唯一可靠区分 Apple Silicon / Intel 的是 `navigator.userAgentData?.architecture`（Chromium 系低熵属性）。UA 字符串里的 `Intel Mac OS X` **不可靠**（Apple Silicon 上的浏览器为兼容也上报 `Macintosh; Intel Mac OS X`），禁止据此判 Intel。arch 缺失（Safari/Firefox/隐私模式）默认 `arm64`，展开区醒目列出 `macos-x64` + 「不确定芯片？」提示。

### 3.3 下载区交互（`download-section.tsx`）

- **三个平台按钮横向平铺**（macOS / Windows / Linux），点击各自展开下拉菜单；同一时刻只展开一个，点击菜单外关闭（`fixed` backdrop）。
- 下拉选项按平台区分——macOS: Apple Silicon / Intel；Linux: DEB / RPM / AppImage；**Windows 无下拉，点击直接下载 EXE**。
- macOS 下拉底部含未签名 `xattr -cr` 提示 + 「不确定芯片？」提示。
- **无环境感知**：不做 UA 检测，三平台按钮始终平铺展示（`detectPlatform` 纯函数保留在 `lib/downloads.ts` 供未来复用，仍有单测覆盖）。
- 配色全部 `fd-*` 语义色。

### 3.4 Hero / 截图 / Footer 骨架

- **Hero**：`section.relative.isolate.overflow-hidden` + 上下两个 `absolute -z-10 transform-gpu overflow-hidden blur-3xl` blob；多边形 **内联 `style={{ clipPath: 'polygon(...)' }}`**（Tailwind 无 clip-path 工具类）；class `aspect-1155/678`、`w-[36.125rem]`、`rotate-[30deg]`、`bg-linear-to-tr from-[#ff80b5] to-[#9089fc]` + `dark:` 调色。正文：announcement 徽章（`hidden sm:flex`）+ `h1 text-4xl sm:text-6xl` + 副标题 + `<DownloadSection/>`（无次 CTA）。
- **截图区**：`max-w-5xl` 卡片，顶栏红黄绿圆点 + 地址栏，内嵌 `<img src="/screenshots/home.png" width={1600} height={1030}>`。
- **Footer**：`border-t border-fd-border`，左侧版权 + GPL-3.0，右侧链接（GitHub / `/docs` / Releases / `/docs/license`）。

## 4. 测试计划

- `docs/lib/downloads.test.ts`：
  - `detectPlatform` 分支矩阵：Windows / Linux UA；macOS + `arch:'arm'`→arm64、`'x86'`→x64、arch 缺失→arm64；UA 含 `Macintosh; Intel Mac OS X`（无 userAgentData）→arm64（验证不误判 Intel）；Android/iOS→unknown；空串→unknown。
  - 数据完整性：每个 Platform `files` 非空、fileName 唯一、URL 以 `RELEASE_BASE` 开头；文件名集合与 README 表格一致。
- CI：`.github/workflows/docs.yml` 在 `pnpm --filter zapmomo-docs build` 前插入 `pnpm --filter zapmomo-docs test`。

### 验收清单

1. `/` 渲染 hero（announcement/大标题/副标题/双 CTA/背景 blob 不溢出）。
2. 桌面 UA：主按钮显示「立即下载」+ 系统平台 logo；macOS arm64/x64 正确分流；按钮内无版本号/文件名。
3. 点「其他平台」展开全部平台行，每行可下载；再点收起；选中行可覆盖主按钮。
4. macOS 主按钮下方出现 `xattr` 未签名提示。
5. 手机 UA：主按钮导向 Releases，展开区仍可手动选。
6. 明暗切换后文本/卡片/CTA 自动换色，blob 用 `dark:` 微调正确。
7. `/docs` 及站内页不受影响；首页搜索可用（静态索引）。
8. `pnpm test` / `types:check` / `build` 全绿，CI docs.yml 增加 test 步骤。

## 5. 风险与注意点

1. **Hydration 不一致（最重要）**：检测必须在 `useEffect`，渲染阶段禁止读 `navigator`。
2. **`nav.title` 覆盖**：用展开合并 `nav={{ ...options.nav, title, url: '/' }}`，勿整块替换。
3. **`clip-path` 内联 style** + blob 容器 `-z-10 transform-gpu overflow-hidden`、外层 `relative isolate`，防被背景盖住 / 横向滚动。
4. **`next/image` 不可用**：静态导出下需 `images.unoptimized`，全站用 `<img>`。
5. **版本号不写死**：不内嵌版本徽章 / 不写死版本号，避免 release-plz 自动发版后过期；版本信息经 announcement「查看版本」跳 Releases 页。
6. **`logo.svg` 1.2MB**：nav 图标偏大，可接受；如需可后续压缩。
7. **不直接 import lucide-react**（pnpm 严格 node_modules 下 docs 拿不到传递依赖）；图标全走 `platform-icons.tsx` 内联 SVG。
