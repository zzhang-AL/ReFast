export enum EventType {
  MouseMove = "MouseMove",
  MouseDown = "MouseDown",
  MouseUp = "MouseUp",
  MouseWheel = "MouseWheel",
  KeyDown = "KeyDown",
  KeyUp = "KeyUp",
}

export enum MouseButton {
  Left = "Left",
  Right = "Right",
  Middle = "Middle",
}

export interface RecordedEvent {
  event_type: EventType;
  x?: number;
  y?: number;
  time_offset_ms: number;
}

export interface RecordingMeta {
  file_path: string;
  file_name: string;
  duration_ms: number;
  event_count: number;
  created_at: string;
}

export type AppStatus = "idle" | "recording" | "playing";

