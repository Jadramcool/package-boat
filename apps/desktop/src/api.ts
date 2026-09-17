// Tauri 命令桥：封装 tauri-specta 生成的 commands，前端组件不直接接触 tauri API。
// 命令签名与 Rust `commands.rs` 由 bindings.ts 同步，新增命令需在 Rust 侧
// `collect_commands!` 注册后重跑 `cargo test -p packetboat-desktop` 重新导出。
import { commands, events, type AppError, type Item, type TransferProgress } from './bindings'
import type { DesktopSettings, DesktopState } from './types'

/** 旧命名别名（组件层仍使用 DesktopItem）。 */
export type { DesktopState } from './types'
export type DesktopItem = Item

/** 把 tauri-specta 的 typedError 结果解包为旧式「成功返回值 / 失败抛错」语义。 */
async function unwrap<T>(result: Promise<{ status: 'ok'; data: T } | { status: 'error'; error: AppError }>): Promise<T> {
  const resolved = await result
  if (resolved.status === 'ok')
    return resolved.data
  const message = typeof resolved.error === 'string' ? resolved.error : '操作失败，请稍后重试'
  throw new Error(message)
}

export async function getState(): Promise<DesktopState> {
  return commands.getState()
}

export async function addLinkedFiles(paths: string[]): Promise<DesktopItem[]> {
  return unwrap(commands.addLinkedFiles(paths))
}

export async function chooseLinkedFiles(): Promise<DesktopItem[]> {
  return unwrap(commands.chooseLinkedFiles())
}

export async function unshare(id: string): Promise<void> {
  await unwrap(commands.unshare(id))
}

export async function clearSharedFiles(): Promise<number> {
  return unwrap(commands.clearSharedFiles())
}

export async function chooseReceiveDirectory(): Promise<string> {
  return unwrap(commands.chooseReceiveDirectory())
}

export async function toggleServer(): Promise<void> {
  await unwrap(commands.toggleServer())
}

/** 更新「是否需要配对码访问」；后端写设置并在服务运行中时重启。 */
export async function setRequirePairing(enabled: boolean): Promise<DesktopSettings> {
  return unwrap(commands.setRequirePairing(enabled))
}

/** 更新「是否展示接收目录内既有文件」；便于把接收目录当共享文件夹用。 */
export async function setShareReceiveDir(enabled: boolean): Promise<DesktopSettings> {
  return unwrap(commands.setShareReceiveDir(enabled))
}

export async function revealItem(id: string): Promise<void> {
  await unwrap(commands.revealItem(id))
}

export async function getTransferProgress(): Promise<TransferProgress> {
  return commands.getTransferProgress()
}

export { events }
