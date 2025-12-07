/**
 * 窗口管理工具函数
 * 封装窗口大小调整等重复逻辑
 */

import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/window";

/**
 * 获取主容器元素
 */
export function getMainContainer(): HTMLElement | null {
  return document.querySelector('.bg-white') as HTMLElement | null;
}

/**
 * 窗口大小调整选项
 */
export interface WindowSizeAdjustOptions {
  windowWidth: number;
  clipboardUrlToOpen?: string | null;
  isMemoModalOpen?: boolean;
  maxHeight?: number;
  minHeight?: number;
  getContainer?: () => HTMLElement | null;
}

/**
 * 调整窗口大小
 */
export function adjustWindowSize(options: WindowSizeAdjustOptions): void {
  const {
    windowWidth,
    clipboardUrlToOpen = null,
    isMemoModalOpen = false,
    maxHeight,
    minHeight,
    getContainer = getMainContainer,
  } = options;

  const whiteContainer = getContainer();
  if (!whiteContainer || isMemoModalOpen) {
    return;
  }

  // 使用双重 requestAnimationFrame 确保 DOM 完全更新
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      const window = getCurrentWindow();
      let containerHeight = whiteContainer.scrollHeight;

      // 如果剪切板 URL 弹窗存在，取主界面和弹窗中更高的那个作为基准高度
      if (clipboardUrlToOpen) {
        const clipboardModal = document.querySelector('.clipboard-url-modal') as HTMLElement | null;
        if (clipboardModal) {
          const modalRect = clipboardModal.getBoundingClientRect();
          const modalHeightWithMargin = modalRect.height + 32;
          containerHeight = Math.max(containerHeight, modalHeightWithMargin);
        }
      }

      // 计算目标高度
      const MAX_HEIGHT = maxHeight ?? (clipboardUrlToOpen ? 720 : 600);
      const MIN_HEIGHT = minHeight ?? (clipboardUrlToOpen ? 260 : 200);
      const targetHeight = Math.max(MIN_HEIGHT, Math.min(containerHeight, MAX_HEIGHT));

      // 设置窗口大小
      window.setSize(new LogicalSize(windowWidth, targetHeight)).catch(console.error);
    });
  });
}

/**
 * 创建窗口大小调整函数（带延迟）
 */
export function createDelayedWindowSizeAdjuster(
  options: WindowSizeAdjustOptions & { delay?: number }
): () => void {
  const { delay = 100, ...adjustOptions } = options;
  
  return () => {
    setTimeout(() => {
      adjustWindowSize(adjustOptions);
    }, delay);
  };
}

