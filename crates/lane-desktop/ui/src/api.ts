// Tauri 命令桥：封装 invoke 调用与事件监听，前端组件不直接接触 tauri API。
import { invoke } from '@tauri-apps/api/core'
import type { DesktopItem, DesktopState, TransferProgress } from './types'

export const stateChangedEvent = 'desktop:state-changed'
export const errorEvent = 'desktop:error'
export const noticeEvent = 'desktop:notice'

export async function getState(): Promise<DesktopState> {
  return invoke<DesktopState>('get_state')
}

export async function addLinkedFiles(paths: string[]): Promise<DesktopItem[]> {
  return invoke<DesktopItem[]>('add_linked_files', { paths })
}

export async function chooseLinkedFiles(): Promise<DesktopItem[]> {
  return invoke<DesktopItem[]>('choose_linked_files')
}

export async function unshare(id: string): Promise<void> {
  return invoke('unshare', { id })
}

export async function chooseReceiveDirectory(): Promise<string> {
  return invoke<string>('choose_receive_directory')
}

export async function toggleServer(): Promise<void> {
  return invoke('toggle_server')
}

export async function revealItem(id: string): Promise<void> {
  return invoke('reveal_item', { id })
}

export async function getTransferProgress(): Promise<TransferProgress> {
  return invoke<TransferProgress>('get_transfer_progress')
}
