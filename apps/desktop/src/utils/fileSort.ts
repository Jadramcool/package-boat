export type FileSortKey =
  | 'name'
  | 'size'
  | 'type'
  | 'source'
  | 'modified'
  | 'added'
  | 'downloads'

export type FileSortDirection = 'asc' | 'desc'

export interface FileSortState {
  key: FileSortKey
  direction: FileSortDirection
}

export const DEFAULT_FILE_SORT: FileSortState = {
  key: 'modified',
  direction: 'desc',
}

export const FILE_SORT_LABELS: Record<FileSortKey, string> = {
  name: '文件名',
  size: '大小',
  type: '类型',
  source: '来源',
  modified: '修改时间',
  added: '添加时间',
  downloads: '下载',
}

/** 可排序文件字段（桌面 Item / 网页 SharedFile 的公共子集）。 */
export interface SortableFileLike {
  name: string
  size: number
  source_type: string
  is_dir?: boolean
  modified?: string
  modified_at?: string
  added_at?: string
  download_count?: number
}

function extensionOf(name: string, isDir?: boolean): string {
  if (isDir)
    return 'folder'
  const index = name.lastIndexOf('.')
  return index > 0 ? name.slice(index + 1).toLowerCase() : ''
}

function sourceRank(source: string): number {
  if (source === 'linked')
    return 0
  if (source === 'received')
    return 1
  if (source === 'inbox')
    return 2
  return 3
}

function timeOf(file: SortableFileLike, key: 'modified' | 'added'): number {
  const raw = key === 'modified'
    ? (file.modified_at ?? file.modified)
    : (file.added_at ?? file.modified_at ?? file.modified)
  if (!raw)
    return 0
  const value = Date.parse(raw)
  return Number.isNaN(value) ? 0 : value
}

function compareStrings(a: string, b: string): number {
  return a.localeCompare(b, 'zh-Hans-CN', { numeric: true, sensitivity: 'base' })
}

export function compareFiles(
  a: SortableFileLike,
  b: SortableFileLike,
  key: FileSortKey,
  direction: FileSortDirection,
): number {
  const sign = direction === 'asc' ? 1 : -1
  let result = 0
  switch (key) {
    case 'name':
      result = compareStrings(a.name, b.name)
      break
    case 'size':
      result = a.size - b.size
      break
    case 'type': {
      result = compareStrings(extensionOf(a.name, a.is_dir), extensionOf(b.name, b.is_dir))
      if (result === 0)
        result = compareStrings(a.name, b.name)
      break
    }
    case 'source':
      result = sourceRank(a.source_type) - sourceRank(b.source_type)
      if (result === 0)
        result = compareStrings(a.name, b.name)
      break
    case 'modified':
      result = timeOf(a, 'modified') - timeOf(b, 'modified')
      break
    case 'added':
      result = timeOf(a, 'added') - timeOf(b, 'added')
      break
    case 'downloads':
      result = (a.download_count ?? 0) - (b.download_count ?? 0)
      break
  }
  if (result === 0)
    result = compareStrings(a.name, b.name)
  return result * sign
}

export function sortFiles<T extends SortableFileLike>(
  files: readonly T[],
  sort: FileSortState,
): T[] {
  return [...files].sort((a, b) => compareFiles(a, b, sort.key, sort.direction))
}

export function nextSortState(current: FileSortState, key: FileSortKey): FileSortState {
  if (current.key === key)
    return { key, direction: current.direction === 'asc' ? 'desc' : 'asc' }
  // 名称默认升序；数值/时间默认降序（更大/更新在前）
  const direction: FileSortDirection = key === 'name' || key === 'type' || key === 'source'
    ? 'asc'
    : 'desc'
  return { key, direction }
}
