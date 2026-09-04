import { CircleAlert, Download, FileAudio, FolderOpen, Repeat2, Settings2 } from "lucide-react";
import { useState } from "react";
import { DeviceSelect } from "@/components/DeviceSelect";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useRuntime } from "@/providers/RuntimeContext";
import { modelNameFromDir } from "./asrMeta";

interface AsrBasicConfigProps {
  /** 打开「选择识别模型」弹窗（由 AsrPage 持有弹窗状态） */
  onSwitchOpen: () => void;
  /** 打开「转写文件」弹窗（选择本地音频 → 整段转写；离线模型主入口，流式模型也可用） */
  onTranscribeOpen: () => void;
}

/**
 * 基础配置（macOS 设置行）：
 * 当前模型（名称 + 就绪/未下载 Badge + 切换模型 + 可展开完整路径）/ 麦克风来源（复用全局 DeviceSelect）+
 * 底部「选择模型 / 转写文件」操作按钮。
 *
 * 模型缺失统一走「选择模型」弹窗（下载/切换同一入口）；不再保留 legacy
 * 一键「下载模型」——旧 sherpa 双语目录与 registry 默认模型不同目录，
 * 一键下载装的是 Qwen3-ASR，装完当前模型仍是缺失态。
 * 收录模型为 raw 单 GGUF、不带示例音频，转写一律由用户选择音频文件。
 */
export function AsrBasicConfig({ onSwitchOpen, onTranscribeOpen }: AsrBasicConfigProps) {
  const {
    asr,
    devices: { error: devicesError },
  } = useRuntime();
  const { config, error } = asr.config;
  const { error: dictateError } = asr.dictate;
  const [showPath, setShowPath] = useState(false);

  const modelsPresent = config?.models_present ?? false;
  const modelPath = config?.model_dir ?? "";
  const modelName = modelNameFromDir(modelPath);

  return (
    <section className="overflow-hidden rounded-[16px] border border-panel-border bg-panel-background">
      <div className="px-3.5 py-2.5">
        <div className="flex items-center gap-2.5">
          <Settings2 className="h-4 w-4 shrink-0 text-text-secondary" />
          <div>
            <h2 className="text-base font-semibold text-text-primary">基础配置</h2>
            <p className="mt-0.5 text-xs text-text-muted">ASR 识别与输入设置</p>
          </div>
        </div>
      </div>

      {error && (
        <div className="space-y-2 px-3.5 pb-2">
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">
              读取配置失败：{error}
            </AlertDescription>
          </Alert>
        </div>
      )}

      {dictateError && (
        <div className="px-3.5 pb-2">
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{dictateError}</AlertDescription>
          </Alert>
        </div>
      )}

      {config && !config.models_present && (
        <div className="px-3.5 pb-2">
          <Alert variant="warning">
            <CircleAlert className="h-4 w-4" />
            <AlertTitle>模型文件缺失</AlertTitle>
            <AlertDescription className="whitespace-pre-wrap">
              {`当前模型文件缺失（${config.model_dir}）。点击下方「选择模型」下载 Qwen3-ASR，或换用已安装模型。`}
            </AlertDescription>
          </Alert>
        </div>
      )}

      <dl>
        <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
          <dt className="shrink-0 text-sm text-text-primary">当前模型</dt>
          <dd className="min-w-0">
            <span className="flex items-center justify-end gap-1.5">
              {modelPath && (
                <button
                  type="button"
                  aria-label={showPath ? "隐藏模型路径" : "查看模型路径"}
                  title={modelPath}
                  onClick={() => setShowPath((v) => !v)}
                  className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-accent hover:text-text-primary"
                >
                  <FolderOpen className="h-3.5 w-3.5" />
                </button>
              )}
              <span
                className="truncate text-sm font-medium text-text-primary"
                title={modelName ?? undefined}
              >
                {modelName ?? "未知模型"}
              </span>
              {config?.backend === "audiocpp" && (
                <Badge
                  variant="outline"
                  className="shrink-0 border-violet-500/20 bg-violet-500/10 text-violet-600"
                  title="由内置 audio.cpp 引擎（sidecar 进程）驱动"
                >
                  audio.cpp
                </Badge>
              )}
              <Badge
                variant="outline"
                className={
                  modelsPresent
                    ? "shrink-0 border-emerald-500/20 bg-emerald-500/10 text-emerald-600"
                    : "shrink-0 border-amber-500/20 bg-amber-500/10 text-amber-600"
                }
              >
                {modelsPresent ? "已就绪" : "未下载"}
              </Badge>
              <Button
                size="sm"
                variant="outline"
                className="shadow-none"
                onClick={onSwitchOpen}
                aria-label="切换识别模型"
              >
                <Repeat2 className="h-3.5 w-3.5" />
                切换模型
              </Button>
            </span>
            {showPath && modelPath && (
              <p className="mt-1 truncate font-mono text-xs text-text-muted" title={modelPath}>
                {modelPath}
              </p>
            )}
          </dd>
        </div>

        <div className="flex items-center justify-between gap-3.5 px-3.5 py-2.5">
          <div className="min-w-0">
            <dt className="text-sm text-text-primary">麦克风来源</dt>
            <dd className="mt-0.5 text-xs text-text-muted">用于语音识别的麦克风输入源</dd>
          </div>
          <DeviceSelect />
        </div>
      </dl>

      {devicesError && (
        <div className="px-3.5 pb-2">
          <Alert variant="destructive">
            <CircleAlert className="h-4 w-4" />
            <AlertDescription className="whitespace-pre-wrap">{devicesError}</AlertDescription>
          </Alert>
        </div>
      )}

      <div className="flex flex-wrap gap-2 border-t border-divider px-3.5 py-2.5">
        {!modelsPresent && (
          <Button onClick={onSwitchOpen}>
            <Download className="h-4 w-4" />
            选择模型
          </Button>
        )}
        <Button
          variant="secondary"
          className="shadow-none"
          disabled={!modelsPresent}
          onClick={onTranscribeOpen}
        >
          <FileAudio className="h-4 w-4" />
          转写文件
        </Button>
      </div>
    </section>
  );
}
