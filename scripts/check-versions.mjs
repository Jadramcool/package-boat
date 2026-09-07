// 校验 Cargo.toml、tauri.conf.json 与前端 package.json 的版本号一致。
// 任何一处缺失或不一致都以非零码退出（CI 与本地 `pnpm run check:versions` 共用）。

import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')

const cargo = readFileSync(resolve(root, 'Cargo.toml'), 'utf8')
const workspaceSection = cargo.match(/\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/)?.[1] ?? ''
const cargoVersion = workspaceSection.match(/^version\s*=\s*"([^"]+)"/m)?.[1]

const tauri = JSON.parse(
  readFileSync(resolve(root, 'crates/lane-desktop/tauri.conf.json'), 'utf8'),
)
const ui = JSON.parse(readFileSync(resolve(root, 'apps/desktop/package.json'), 'utf8'))

const sources = [
  ['Cargo.toml [workspace.package]', cargoVersion],
  ['crates/lane-desktop/tauri.conf.json', tauri.version],
  ['apps/desktop/package.json', ui.version],
]

const missing = sources.filter(([, value]) => !value)
if (missing.length > 0) {
  console.error(`版本字段缺失: ${missing.map(([name]) => name).join(', ')}`)
  process.exit(1)
}

const unique = new Set(sources.map(([, value]) => value))
if (unique.size !== 1) {
  console.error(
    '版本不一致:\n' + sources.map(([name, value]) => `  ${name}: ${value}`).join('\n'),
  )
  process.exit(1)
}

console.log(`版本一致: ${[...unique][0]}`)
