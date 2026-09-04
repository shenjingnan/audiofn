import type { KwsConfigInfo } from "@/types/tauri";

/** 状态语义色：绿=监听中、灰=未监听/未下载、红=错误。 */
export type KwsStatusTone = "good" | "idle" | "error";

export const KWS_STATUS_COLOR: Record<KwsStatusTone, string> = {
  good: "text-emerald-600",
  idle: "text-text-muted",
  error: "text-red-600",
};

/** 从 model_dir 派生展示名：取 basename。空路径返回 null，不硬编码任何模型名。 */
export function modelNameFromDir(dir: string | null | undefined): string | null {
  if (!dir) return null;
  return dir.split(/[\\/]/).pop() ?? dir;
}

/** 默认（legacy 一键下载所装的）zh-en 模型目录名前缀（不同版本安装布局的日期后缀可能不同）。 */
const DEFAULT_KWS_DIR_PREFIX = "sherpa-onnx-kws-zipformer-zh-en";

/**
 * 当前模型目录是否为默认 zh-en 模型：决定模型缺失时展示 legacy「下载模型」
 * （固定下载 zh-en）还是「选择模型」弹窗（可下载/切换其他 KWS 模型）。
 */
export function isDefaultKwsModelDir(dir: string | null | undefined): boolean {
  const name = modelNameFromDir(dir);
  return !!name && name.startsWith(DEFAULT_KWS_DIR_PREFIX);
}

/**
 * KWS 状态语义色（判断顺序：错误 > 监听中 > 未监听 > 未下载模型）。
 * 标题栏开关旁的文字显示「启用/禁用 KWS」，圆点颜色用此 tone 反映真实监听状态。
 * `configError`（get_kws_config 失败）不进入此状态机，由调用方单独展示。
 */
export function kwsStatus(
  cfg: KwsConfigInfo | null,
  st: { isListening: boolean; error: string | null },
): { tone: KwsStatusTone } {
  if (st.error) return { tone: "error" };
  if (st.isListening) return { tone: "good" };
  if (cfg?.models_present) return { tone: "idle" };
  return { tone: "idle" };
}
