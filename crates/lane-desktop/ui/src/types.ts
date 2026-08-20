// LANE 桌面端类型定义 —— 与 Tauri 命令返回值一一对应（Rust 侧 serde snake_case）。
export type SourceType = 'linked' | 'received'

export interface DesktopItem {
  id: string
  name: string
  source_type: SourceType
  local_path: string
  size: number
  modified_at: string
  added_at: string
  available: boolean
  is_dir: boolean
}

export interface DesktopHostState {
  running: boolean
  urls: readonly string[]
  access_code: string
  error?: string
}

export interface DesktopSettings {
  device_name: string
  host: string
  port: number
  receive_dir: string
  max_upload_bytes: number
}

export interface DesktopState {
  host: DesktopHostState
  settings: DesktopSettings
  items: DesktopItem[]
  version: string
}

/** 传输进度（Rust 侧 ProgressTracker 快照，snake_case）。 */
export interface TransferProgress {
  active: boolean
  done: number
  total: number | null
}
