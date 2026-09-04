import { QueryClientProvider } from "@tanstack/react-query";
import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "@/components/layout/AppShell";
import { ToastProvider } from "@/components/ui/toast";
import { queryClient } from "@/lib/queryClient";
import { HomePage } from "@/pages/HomePage";
import { ModelsOverviewPage } from "@/pages/ModelsOverviewPage";
import { AsrPage } from "@/pages/models/AsrPage";
import { TtsPage } from "@/pages/models/TtsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { AppRuntimeProvider } from "@/providers/AppRuntimeProvider";
import { StorageGateProvider } from "@/providers/StorageGateProvider";

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      {/* ToastProvider 提到 Runtime 外层：runtime hooks 需要在其内部弹通知。 */}
      <ToastProvider>
        {/* StorageGate 在 Runtime 外：下载/导入 hooks 调 useStorageGate 做首次下载引导。 */}
        <StorageGateProvider>
          <AppRuntimeProvider>
            <Routes>
              <Route element={<AppShell />}>
                <Route index element={<Navigate to="/home" replace />} />
                <Route path="home" element={<HomePage />} />
                <Route path="models" element={<ModelsOverviewPage />} />
                <Route path="models/asr" element={<AsrPage />} />
                <Route path="models/tts" element={<TtsPage />} />
                <Route path="settings" element={<SettingsPage />} />
              </Route>
            </Routes>
          </AppRuntimeProvider>
        </StorageGateProvider>
      </ToastProvider>
    </QueryClientProvider>
  );
}
