/**
 * One-off README screenshot capture against a local PacketBoat demo server.
 * Usage: node scripts/capture-readme-shots.mjs <baseUrl> <pairCode> <outDir>
 */
import { createRequire } from 'node:module'
import path from 'node:path'
import fs from 'node:fs'
import { pathToFileURL } from 'node:url'

const require = createRequire(import.meta.url)
const { chromium } = require('C:/Users/jdm/AppData/Local/npm-cache/_npx/31e32ef8478fbf80/node_modules/playwright')

const baseUrl = process.argv[2] || 'http://127.0.0.1:18991'
const code = String(process.argv[3] || '000000')
const outDir = process.argv[4] || 'docs/images'
fs.mkdirSync(outDir, { recursive: true })

const save = (name) => path.join(outDir, name)

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

const browser = await chromium.launch({
  headless: true,
  channel: 'msedge',
  args: ['--force-device-scale-factor=2'],
})

try {
  // --- Desktop pairing gate ---
  const desktop = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
    locale: 'zh-CN',
  })
  const page = await desktop.newPage()
  await page.goto(baseUrl, { waitUntil: 'networkidle' })
  await page.waitForTimeout(500)
  await page.screenshot({ path: save('web-pairing.png'), fullPage: false })

  await pair(page, code)
  await waitFor(page, async () => {
    const t = await page.locator('body').innerText()
    return t.includes('共享') || t.includes('投递') || t.includes('货舱') || t.includes('在线') || t.includes('文件架')
  }, 10000)
  await page.waitForTimeout(1000)
  await page.screenshot({ path: save('web-desktop.png'), fullPage: false })

  // Scroll a bit to show upload queue area if present
  await page.mouse.wheel(0, 200)
  await page.waitForTimeout(300)
  await page.screenshot({ path: save('web-desktop-scrolled.png'), fullPage: false })
  await desktop.close()

  // --- Mobile browser ---
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
  await mpage.waitForTimeout(400)
  await mpage.screenshot({ path: save('mobile-pairing.png'), fullPage: false })

  await pair(mpage, code)
  await waitFor(mpage, async () => {
    const t = await mpage.locator('body').innerText()
    return t.includes('共享') || t.includes('投递') || t.includes('货舱') || t.includes('在线') || t.includes('文件架')
  }, 10000)
  await mpage.waitForTimeout(1000)
  await mpage.screenshot({ path: save('mobile-browser.png'), fullPage: false })
  await mobile.close()

  console.log('OK screenshots written to', path.resolve(outDir))
  for (const f of fs.readdirSync(outDir)) {
    if (f.endsWith('.png') || f.endsWith('.jpg')) {
      const p = path.join(outDir, f)
      console.log(f, fs.statSync(p).size)
    }
  }
} finally {
  await browser.close()
}
