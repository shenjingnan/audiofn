import { ModelSummary } from "@/components/models/ModelSummary";
import { SetupGuideAlert } from "@/components/models/SetupGuideAlert";

/** 模型概览页：引导卡 + 模型摘要。 */
export function ModelsOverviewPage() {
  return (
    <div className="flex flex-col gap-3">
      <header>
        <h1 className="text-2xl font-semibold tracking-tight text-text-primary">模型与能力</h1>
      </header>
      <SetupGuideAlert />
      <ModelSummary />
    </div>
  );
}
