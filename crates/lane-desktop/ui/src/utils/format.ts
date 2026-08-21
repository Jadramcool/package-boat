const units = ['B', 'KB', 'MB', 'GB', 'TB'] as const

export function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0)
    return '0 B'
  const unitIndex = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1)
  const scaled = value / 1024 ** unitIndex
  const digits = scaled >= 100 || unitIndex === 0 ? 0 : scaled >= 10 ? 1 : 2
  return `${scaled.toFixed(digits)} ${units[unitIndex]}`
}

export function formatDuration(value: number): string {
  if (!Number.isFinite(value) || value <= 0)
    return '0 秒'
  const seconds = Math.ceil(value)
  if (seconds < 60)
    return `${seconds} 秒`
  if (seconds < 3_600) {
    const minutes = Math.floor(seconds / 60)
    const remainder = seconds % 60
    return remainder > 0 ? `${minutes} 分 ${remainder} 秒` : `${minutes} 分`
  }
  const hours = Math.floor(seconds / 3_600)
  const minutes = Math.floor(seconds % 3_600 / 60)
  return minutes > 0 ? `${hours} 小时 ${minutes} 分` : `${hours} 小时`
}

export function formatDate(value: string): string {
  const date = new Date(value)
  const today = new Date()
  const sameDay = date.toDateString() === today.toDateString()
  const sameYear = date.getFullYear() === today.getFullYear()
  return new Intl.DateTimeFormat('zh-CN', sameDay
    ? { hour: '2-digit', minute: '2-digit' }
    : { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', ...(sameYear ? {} : { year: 'numeric' }) },
  ).format(date)
}

export function fileExtension(name: string): string {
  const index = name.lastIndexOf('.')
  return index > 0 ? name.slice(index + 1).toUpperCase().slice(0, 5) : 'FILE'
}
