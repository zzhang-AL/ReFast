import type { PluginContext } from "../../../types";
import { isMathExpression } from "../../../utils/launcherUtils";

export default async function execute(context: PluginContext) {
  // 打开独立的计算稿纸窗口
  if (context.tauriApi) {
    await context.tauriApi.showCalculatorPadWindow();
    
    // 如果查询内容是数学表达式，发送事件传递表达式到计算稿纸窗口
    if (context.query && isMathExpression(context.query)) {
      // 延迟发送事件，确保窗口已创建并准备好接收事件
      setTimeout(async () => {
        try {
          const { emit } = await import("@tauri-apps/api/event");
          await emit("calculator-pad:set-expression", context.query);
        } catch (error) {
          console.error("Failed to send expression to calculator pad window:", error);
          // 如果第一次失败，再试一次
          setTimeout(async () => {
            try {
              const { emit } = await import("@tauri-apps/api/event");
              await emit("calculator-pad:set-expression", context.query);
            } catch (retryError) {
              console.error("Failed to send expression to calculator pad window (retry):", retryError);
            }
          }, 500);
        }
      }, 500);
    }
    
    // 关闭启动器
    await context.hideLauncher();
  }
}

