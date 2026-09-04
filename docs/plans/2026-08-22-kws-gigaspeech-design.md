# KWS GigaSpeech 英文模型接入设计

日期：2026-08-22
状态：已确认（用户拍板：只补 GigaSpeech；自定义唤醒词走 sentencepiece BPE 路径）

## 背景

sherpa-onnx 官方预训练 KWS 模型共 3 个，ZapMomo 已支持 zh-en（默认）与 wenetspeech（纯中文），
缺 `sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01`（纯英文，GigaSpeech XL 1 万小时，
建模单元为 BPE）。本次补齐该模型，使官方 KWS 全家桶完整。

## 包内实况（已下载核实，sha256 f170013b…，17626723 字节）

- 三件套文件名与 wenetspeech 完全同族：`encoder/decoder/joiner-epoch-12-avg-2-chunk-16-left-64.onnx`（另有 int8 变体）
- `tokens.txt`：500 个 BPE 子词（`▁HE`、`LL`、`O`…）
- `bpe.model`：sentencepiece BPE 模型（239KB），自定义唤醒词的编码依据
- 关键词文件：`test_wavs/test_keywords.txt`（与 wenetspeech 同名，探测链第二候选命中）；
  包根目录的 `keywords.txt` 含不在 tokens.txt 的 piece（`▁WO`），为旧脚本残留，不采用
- 关键词行无 `@显示词` 后缀（我们的 parse_keywords_file 已兼容）

## 方案

### 1. 接入层（复刻 wenetspeech 模式，零新探测逻辑）

- `models/manifest.json`：新增 role `wake-word-gigaspeech` 资产
- `models/model_registry.json`：新增 id `kws-zipformer-gigaspeech-3.3m`（languages `["en"]`）
- `src/kws/model.rs`：`KWS_GIGASPEECH_REQUIRED_FILES` = wenetspeech 同款三件套（复用常量）
  + `tokens.txt` + `test_wavs/test_keywords.txt` + `bpe.model`（6 件，bpe.model 纳入安装完整性）
- `src/model_library/registry.rs`：`required_files_for_role` 加 `"wake-word-gigaspeech"` arm；
  registry 计数测试 23 → 24
- `src/kws/config.rs` 探测逻辑零改动（epoch-12 chunk-16 与 `test_wavs/test_keywords.txt`
  均被现有规则自动命中），仅补 fake gigaspeech 目录的 resolve 测试

### 2. BPE 关键词编码路径（唯一新逻辑）

> **实施中方案变更**：原计划引入 sentencepiece crate（0.14/0.11），实测发现该 crate
> 全版本依赖 `sentencepiece-sys`（C++/cmake 构建），其静态 protobuf 与
> `sherpa-onnx-sys` 内嵌的 protobuf 重复符号（ODR 冲突），同链测试直接段错误。
> 且解析核实 `bpe.model` 的 `trainer_spec.model_type` 实为 **UNIGRAM**（1），并非 BPE
> （sherpa 沿用 icefall `lang_bpe_500` 命名）。故改为 `src/kws/bpe.rs` 内置最小实现：
> 解析 ModelProto（NORMAL pieces + 对数概率分）+ unigram Viterbi（最大化子词对数概率
> 之和，未知字符按 `min_score - 10` 回退并合并连续段）。零新依赖；输出向量与官方
> sentencepiece（C++ 0.11/0.14）实测 **8/8 一致**（`HEY MOMO` → `▁HE Y ▁MO MO` 等）。

- `src/kws/bpe.rs`：`load(path)` + `encode_phrase(model, phrase)`（trim + 大写化后
  Viterbi，词表以全大写为主，保证唤醒词大小写无关）
- `encode_custom_keywords` 探测 `tokens.txt` 同目录的 `bpe.model` 存在 → BPE 模式：
  - 已是合法 piece 序列（手写）→ 透传
  - 否则英文短语 → BPE 编码，`validate_tokens` 逐 piece 校验（沿用现有防崩溃约束）
  - 中文输入 → 明确报错「该模型仅支持英文唤醒词」
  - 显示词 `@HEY_MOMO`（`_` 连接，与现有约定一致）
- 探测方式与 en.phone 完全一致（文件存在性），zh-en/wenetspeech 不受影响
- 测试夹具：官方 `bpe.model`（239KB）提交至 `src/kws/testdata/`（`include_bytes!`
  写临时目录），许可归属记入 `models/THIRD_PARTY_NOTICES.md`

### 3. 前端与文档

- `useKwsModelSwitch.ts` 的 `KWS_PRESETS` 加 gigaspeech 条目（tagline「纯英文 · GigaSpeech 1 万小时」）
- `docs/content/docs/kws/model.mdx` 模型表格加一行；`configuration.mdx` 候选链说明补 gigaspeech

### 4. 测试

- Rust：registry 计数/role arm、config gigaspeech 布局探测、bpe.rs 编码与
  encode_custom_keywords BPE 模式（含中文报错、piece 校验失败）
- 夹具：官方 `bpe.model`（239KB）提交至 `src/kws/testdata/`，测试用
  `include_bytes!` 写临时目录（Apache-2.0，注明来源）
- 前端：现有用例基于 mock 列表，加预设不破坏；跑全量 vitest 确认

## 验收

- `cargo fmt --check && cargo clippy -- -D warnings && cargo test -- --test-threads=1`
- 前端 `tsc -b` + vitest 全绿
- registry/manifest 自洽测试通过（role 存在性校验自动覆盖新条目）

## 非目标

- chunk-8 低延迟变体、int8 量化变体的可选配置（留待后续）
- gigaspeech 包根目录旧格式 `keywords.txt` 的兼容
