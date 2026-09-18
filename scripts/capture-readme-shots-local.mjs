/**
 * Build assets already assumed built. Launch screenshot server, capture README shots, kill server.
 */
import { createRequire } from 'node:module'
import { spawn } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)
const { chromium } = require('C:/Users/jdm/AppData/Local/npm-cache/_npx/31e32ef8478fbf80/node_modules/playwright')

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const exe = path.join(root, 'target/debug/examples/screenshot_server.exe')
const metaPath = path.join(root, 'target/screenshot-server.json')
const outDir = path.join(root, 'docs/images')

const child = spawn(exe, [], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] })
let stdout = ''
let stderr = ''
child.stdout.on('data', (d) => { stdout += d.toString() })
child.stderr.on('data', (d) => { stderr += d.toString() })

async function waitForMeta(timeoutMs = 15000) {
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    if (fs.existsSync(metaPath)) {
      try {
        const meta = JSON.parse(fs.readFileSync(metaPath, 'utf8'))
        if (meta.port && meta.code) return meta
      } catch {}
    }
    await new Promise((r) => setTimeout(r, 300))
  }
  throw new Error('meta timeout\n' + stdout + stderr)
}

async function waitForHttp(url, timeoutMs = 10000) {
  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    try {
      const res = await fetch(url)
      if (res.ok || res.status < 500) return true
    } catch {}
    await new Promise((r) => setTimeout(r, 300))
  }
  throw new Error('http timeout')
}

async function waitFor(page, fn, timeout = 8000) {
  const start = Date.now()
  while (Date.now() - start < timeout) {
    try {
      if (await fn()) return true
    } catch {}
    await page.waitForTimeout(200)
  }
  return false
}

async function pair(page, code) {
  const input = page.locator('#pair-code')
  await input.waitFor({ state: 'visible', timeout: 8000 })
  await input.click()
  await input.fill(code)
  await page.getByRole('button', { name: /进入投递站|连接|配对/i }).first().click()
}

let browser
try {
  const meta = await waitForMeta()
  const baseUrl = `http://127.0.0.1:${meta.port}`
  const code = String(meta.code)
  console.log('server', baseUrl, code)
  await waitForHttp(baseUrl)
  fs.mkdirSync(outDir, { recursive: true })

  browser = await chromium.launch({
    headless: true,
    channel: 'msedge',
    args: ['--force-device-scale-factor=2'],
  })

  const desktop = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
    locale: 'zh-CN',
  })
  const page = await desktop.newPage()
  await page.goto(baseUrl, { waitUntil: 'networkidle' })
  await page.waitForTimeout(600)
  await page.screenshot({ path: path.join(outDir, 'web-pairing.png') })

  await pair(page, code)
  await waitFor(page, async () => {
    const t = await page.locator('body').innerText()
    return /共享|投递|货舱|在线|文件架/.test(t)
  }, 10000)
  await page.waitForTimeout(1000)
  await page.screenshot({ path: path.join(outDir, 'web-desktop.png') })
  await page.mouse.wheel(0, 200)
  await page.waitForTimeout(300)
  await page.screenshot({ path: path.join(outDir, 'web-desktop-scrolled.png') })
  await desktop.close()

  const mobile = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 3,
    isMobile: true,
    hasTouch: true,
    locale: 'zh-CN',
    userAgent:
      'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1',
  })
  const mpage = await mobile.newPage()
  await mpage.goto(baseUrl, { waitUntil: 'networkidle' })
  await mpage.waitForTimeout(500)
  await mpage.screenshot({ path: path.join(outDir, 'mobile-pairing.png') })
  await pair(mpage, code)
  await waitFor(mpage, async () => {
    const t = await mpage.locator('body').innerText()
    return /共享|投递|货舱|在线|文件架/.test(t)
  }, 10000)
  await mpage.waitForTimeout(1000)
  await mpage.screenshot({ path: path.join(outDir, 'mobile-browser.png') })
  await mobile.close()

  console.log('OK screenshots written')
  for (const f of ['web-pairing.png', 'web-desktop.png', 'web-desktop-scrolled.png', 'mobile-pairing.png', 'mobile-browser.png', 'desktop-live.png', 'app-icon.png']) {
    const p = path.join(outDir, f)
    if (fs.existsSync(p)) console.log(f, fs.statSync(p).size)
  }
} finally {
  if (browser) await browser.close().catch(() => {})
  try { child.kill() } catch {}
}
