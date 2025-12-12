/**
 * 平台检测与跨平台文案辅助。
 * <p>
 * 说明：项目目前未引入 `@tauri-apps/plugin-os`，这里使用 User-Agent 做“展示层”判断，
 * 仅用于 UI 文案与按钮显示（不参与安全/权限相关逻辑）。
 * </p>
 */
export type Platform = "macos" | "windows" | "linux" | "unknown";

export function detectPlatform(): Platform {
  const ua = (navigator.userAgent || "").toLowerCase();
  if (ua.includes("windows")) return "windows";
  if (ua.includes("macintosh") || ua.includes("mac os x") || ua.includes("macos")) return "macos";
  if (ua.includes("linux")) return "linux";
  return "unknown";
}

export function getFileIndexEngineLabel(platform: Platform): string {
  if (platform === "windows") return "Everything";
  if (platform === "macos") return "Spotlight（mdfind）";
  return "系统索引";
}

export function supportsEverythingInstallActions(platform: Platform): boolean {
  return platform === "windows";
}

