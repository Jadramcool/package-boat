import { createRequire } from 'node:module'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

const require = createRequire(import.meta.url)
const { chromium } = require('C:/Users/jdm/AppData/Local/npm-cache/_npx/31e32ef8478fbf80/node_modules/playwright')

const htmlPath = process.argv[2]
const outPath = process.argv[3]
const browser = await chromium.launch({ headless: true, channel: 'msedge' })
try {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
    locale: 'zh-CN',
  })
  const page = await context.newPage()
  await page.goto(pathToFileURL(path.resolve(htmlPath)).href, { waitUntil: 'load' })
  await page.waitForTimeout(400)
  await page.screenshot({ path: outPath, fullPage: false })
  console.log('saved', outPath)
} finally {
  await browser.close()
}
