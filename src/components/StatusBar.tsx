import type { AppStatus } from "../types";

interface StatusBarProps {
  status: AppStatus;
  message?: string;
  progress?: number;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  status,
  message,
  progress,
}) => {
  const getStatusText = (): string => {
    switch (status) {
      case "recording":
        return "正在录制";
      case "playing":
        return "正在回放";
      default:
        return "空闲";
    }
  };

  const getStatusColor = (): string => {
    switch (status) {
      case "recording":
        return "text-red-500";
      case "playing":
        return "text-blue-500";
      default:
        return "text-gray-500";
    }
  };

  return (
    <div className="border-t border-gray-300 p-3 bg-gray-50">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className={`font-medium ${getStatusColor()}`}>
            {getStatusText()}
          </span>
          {message && <span className="text-sm text-gray-600">- {message}</span>}
        </div>
        {status === "playing" && progress !== undefined && (
          <div className="flex items-center gap-2">
            <div className="w-32 h-2 bg-gray-200 rounded-full overflow-hidden">
              <div
                className="h-full bg-blue-500 transition-all"
                style={{ width: `${progress}%` }}
              ></div>
            </div>
            <span className="text-sm text-gray-600">{progress.toFixed(1)}%</span>
          </div>
        )}
      </div>
    </div>
  );
};

