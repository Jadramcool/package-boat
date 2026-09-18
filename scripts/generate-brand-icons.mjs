// Generate PacketBoat PB-05 app icons via sharp (SVG → PNG/ICO-sized set)
import { createRequire } from 'node:module'
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)
const sharedModules = process.env.MIMO_NODE_MODULES
  || 'C:/Program Files/Xiaomi MiMo/resources/runtimes/win32-x64/node_modules'
const sharp = require(path.join(sharedModules, 'sharp'))

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(__dirname, '..')
const svgPath = path.join(root, 'design/logos/svg/pb-05-app-icon.svg')
const brandSvg = path.join(root, 'docs/images/logos/packetboat-private-wake.svg')
const monoSvg = path.join(root, 'design/logos/svg/pb-05-private-wake.svg')

const iconsDir = path.join(root, 'crates/packetboat-desktop/icons')
const docsIcons = path.join(root, 'docs/images')
const logosDir = path.join(root, 'docs/images/logos')

await mkdir(iconsDir, { recursive: true })
await mkdir(docsIcons, { recursive: true })
await mkdir(logosDir, { recursive: true })

const svg = await readFile(svgPath)
const brand = await readFile(brandSvg)

async function png(buf, size, out) {
  await sharp(buf, { density: Math.max(72, Math.ceil((size / 100) * 300)) })
    .resize(size, size, { fit: 'contain', background: { r: 0, g: 0, b: 0, alpha: 0 } })
    .png()
    .toFile(out)
  console.log('ok', out)
}

// Transparent-ish brand mark on dark tile for Tauri
const tauriSvg = svg // already has ink background

await png(tauriSvg, 32, path.join(iconsDir, '32x32.png'))
await png(tauriSvg, 128, path.join(iconsDir, '128x128.png'))
await png(tauriSvg, 256, path.join(iconsDir, '128x128@2x.png'))
await png(tauriSvg, 512, path.join(iconsDir, 'app-icon.png'))
await png(tauriSvg, 512, path.join(docsIcons, 'app-icon.png'))
await png(brand, 512, path.join(logosDir, 'packetboat-private-wake-512.png'))
await png(brand, 1024, path.join(logosDir, 'packetboat-private-wake-1024.png'))

// ICO: multi-size via png-to-ico if available, else write 256 png as icon.ico fallback + use pngs
try {
  const { PNG } = require('pngjs')
  // build simple ICO with embedded PNGs (Vista+ style)
  async function makeIco(sizes, outPath) {
    const images = []
    for (const s of sizes) {
      const data = await sharp(tauriSvg, { density: Math.max(72, Math.ceil((s / 100) * 300)) })
        .resize(s, s)
        .png()
        .toBuffer()
      images.push({ size: s, data })
    }
    const headerSize = 6 + images.length * 16
    const header = Buffer.alloc(headerSize)
    header.writeUInt16LE(0, 0)
    header.writeUInt16LE(1, 2)
    header.writeUInt16LE(images.length, 4)
    let offset = headerSize
    images.forEach((img, i) => {
      const entry = 6 + i * 16
      header[entry] = img.size >= 256 ? 0 : img.size
      header[entry + 1] = img.size >= 256 ? 0 : img.size
      header[entry + 2] = 0
      header[entry + 3] = 0
      header.writeUInt16LE(1, entry + 4)
      header.writeUInt16LE(32, entry + 6)
      header.writeUInt32LE(img.data.length, entry + 8)
      header.writeUInt32LE(offset, entry + 12)
      offset += img.data.length
    })
    const parts = [header, ...images.map((i) => i.data)]
    await writeFile(outPath, Buffer.concat(parts))
    console.log('ok', outPath)
  }
  await makeIco([16, 24, 32, 48, 64, 128, 256], path.join(iconsDir, 'icon.ico'))
} catch (err) {
  console.error('ico skip', err.message)
  // fallback: write largest png bytes into ico with PNG payload wrapper manually without pngjs
  const data = await sharp(tauriSvg, { density: 300 }).resize(256, 256).png().toBuffer()
  const header = Buffer.alloc(22)
  header.writeUInt16LE(0, 0)
  header.writeUInt16LE(1, 2)
  header.writeUInt16LE(1, 4)
  header[6] = 0
  header[7] = 0
  header.writeUInt16LE(1, 10)
  header.writeUInt16LE(32, 12)
  header.writeUInt32LE(data.length, 14)
  header.writeUInt32LE(22, 18)
  await writeFile(path.join(iconsDir, 'icon.ico'), Buffer.concat([header, data]))
  console.log('ok-fallback', path.join(iconsDir, 'icon.ico'))
}

// Replace old AI logo placeholders with official mark copies
const official = path.join(logosDir, 'logo-E-private-wake.png')
await png(brand, 512, official)
console.log('done')
