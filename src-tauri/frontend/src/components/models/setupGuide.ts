import type { RuntimeState } from "@/providers/RuntimeContext";

export type GuideCapability = "asr" | "tts";

export interface GuideIssue {
  capability: GuideCapability;
  /** 展示名，与模型摘要行名一致，如「语音识别（ASR）」。 */
  name: string;
  kind: "error" | "unconfigured";
  /** 对应配置页路由，如 /models/asr。 */
  href: string;
}

/** 能力展示顺序与名称（与 ModelSummary 行保持一致）。 */
const CAPABILITY_ORDER: ReadonlyArray<{ capability: GuideCapability; name: string; href: string }> =
  [
    { capability: "asr", name: "语音识别（ASR）", href: "/models/asr" },
    { capability: "tts", name: "语音合成（TTS）", href: "/models/tts" },
  ];

export type SetupGuideInput = Pick<RuntimeState, "asr" | "tts">;

/** 各能力的错误与未配置判定（config 层与运行层错误都算）。 */
function capabilityIssue(
  capability: GuideCapability,
  name: string,
  href: string,
  runtime: SetupGuideInput,
): GuideIssue | null {
  switch (capability) {
    case "asr": {
      const { config, dictate } = runtime.asr;
      if (config.error || dictate.error) return { capability, name, kind: "error", href };
      // config 未加载完成（null）时不判断未配置，避免首帧闪「未配置」卡。
      if (config.config && !config.config.models_present) {
        return { capability, name, kind: "unconfigured", href };
      }
      return null;
    }
    case "tts": {
      const tts = runtime.tts;
      if (tts.configError || tts.error) return { capability, name, kind: "error", href };
      // enabled=false 是用户主动关闭，不算问题；只看 models_present。
      if (tts.config && !tts.config.models_present) {
        return { capability, name, kind: "unconfigured", href };
      }
      return null;
    }
  }
}

/**
 * 推导引导卡问题列表：错误在前、未配置在后；组内按 asr→tts。
 * 每能力至多产出一条 issue（错误优先于未配置）。
 */
export function deriveSetupGuideIssues(runtime: SetupGuideInput): GuideIssue[] {
  const errors: GuideIssue[] = [];
  const unconfigured: GuideIssue[] = [];
  for (const { capability, name, href } of CAPABILITY_ORDER) {
    const issue = capabilityIssue(capability, name, href, runtime);
    if (!issue) continue;
    (issue.kind === "error" ? errors : unconfigured).push(issue);
  }
  return [...errors, ...unconfigured];
}
