import type { PluginContext } from "../../../types";

export default async function execute(context: PluginContext) {
  // 打开独立的翻译窗口
  if (context.tauriApi) {
    await context.tauriApi.showTranslationWindow();
    // 关闭启动器
    await context.hideLauncher();
  }
}

