export type SourceType = 'linked' | 'received'

export interface ServiceInfo {
  device_name: string
  requires_auth: boolean
  max_upload_bytes: number
  version: string
}

export interface SessionStatus {
  authenticated: boolean
}

export interface SharedFile {
  id: string
  name: string
  size: number
  modified: string
  source_type: SourceType
  available: boolean
  is_dir: boolean
}

export interface FileListResponse {
  files: SharedFile[]
}

export interface UploadResponse {
  files: SharedFile[]
}

export interface ChunkUploadSession {
  id: string
  file_name: string
  file_size: number
  chunk_size: number
  total_chunks: number
  received_chunks: number[]
  uploaded_bytes: number
  completed: boolean
}

export interface UploadProgressSnapshot {
  uploadedBytes: number
  totalBytes: number
  progress: number
  resumed: boolean
}

export type UploadStatus = 'queued' | 'uploading' | 'paused' | 'complete' | 'error' | 'cancelled'

export interface UploadTask {
  id: string
  file: File
  progress: number
  uploadedBytes: number
  speedBytesPerSecond: number
  etaSeconds?: number
  status: UploadStatus
  error?: string
  sessionID?: string
  resumed?: boolean
}

export type TransferHistoryStatus = 'complete' | 'error' | 'cancelled'

export interface TransferHistoryEntry {
  id: string
  name: string
  size: number
  status: TransferHistoryStatus
  error?: string
  finishedAt: string
}
