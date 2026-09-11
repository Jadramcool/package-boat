import { cp, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import { dirname, extname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { gzipSync } from 'node:zlib'

const scriptDirectory = dirname(fileURLToPath(import.meta.url))
const uiDirectory = resolve(scriptDirectory, '..')
const sourceDirectory = resolve(uiDirectory, 'dist')
const targetDirectory = resolve(uiDirectory, '../../crates/packetboat-core/assets/dist')

await rm(targetDirectory, { recursive: true, force: true })
await mkdir(targetDirectory, { recursive: true })
await cp(sourceDirectory, targetDirectory, { recursive: true })

for (const relativePath of await assetFiles(targetDirectory)) {
  if (!['.css', '.js'].includes(extname(relativePath)))
    continue
  const path = resolve(targetDirectory, relativePath)
  const source = await readFile(path)
  const compressed = gzipSync(source, { level: 9, mtime: 0 })
  // gzip 头部第 10 字节是 OS 字段：Windows 的 zlib 写 0x0a、Linux 写 0x03，
  // 会让 CI 的嵌入资源新鲜度校验误报差异。统一固定为 0xff（unknown）。
  compressed[9] = 0xff
  await writeFile(`${path}.gz`, compressed)
}

console.log(`Synced browser assets to ${targetDirectory}`)

async function assetFiles(directory, prefix = '') {
  const files = []
  const entries = await readdir(directory, { withFileTypes: true })
  for (const entry of entries) {
    const relativePath = prefix ? `${prefix}/${entry.name}` : entry.name
    if (entry.isDirectory())
      files.push(...await assetFiles(resolve(directory, entry.name), relativePath))
    else
      files.push(relativePath)
  }
  return files
}
