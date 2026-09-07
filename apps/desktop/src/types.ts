// PacketBoat 桌面端类型定义 —— 由 tauri-specta 生成的 bindings.ts 统一提供。
// 这里基于 bindings 的 Serialize 变体派生旧命名别名，并做只读适配：
// 1) Vue `readonly()` 会产生 DeepReadonly，可变数组（string[]）无法承载，
//    故 urls/items 以 readonly 数组声明（可变 → 只读天然兼容）。
// 2) 序列化/反序列化 union（*_Serialize | *_Deserialize）会让可选字段
//    泄漏到读取侧，桌面端展示统一使用 Serialize 变体。
import type { Item, Settings_Serialize, StatePayload_Serialize } from './bindings'

export type {
  SourceType,
  Item,
  HostState,
  Settings,
  StatePayload,
  TransferProgress,
  AppError,
  ErrorNotice,
  SuccessNotice,
} from './bindings'

export type DesktopItem = Item

export type DesktopHostState = Omit<StatePayload_Serialize['host'], 'urls'> & {
  urls: readonly string[]
}

export type DesktopSettings = Settings_Serialize

export type DesktopState = {
  host: DesktopHostState
  settings: DesktopSettings
  items: readonly DesktopItem[]
  version: string
}
