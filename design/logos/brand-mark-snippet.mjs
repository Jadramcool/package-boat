// Shared brand mark SVG snippets for PacketBoat PB-05 私域航迹
export const brandMarkSvg = (opts = {}) => {
  const {
    size = 20,
    color = 'currentColor',
    uid = `bm${Math.random().toString(36).slice(2, 7)}`,
  } = opts
  return `<svg width="${size}" height="${size}" viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true" style="display:block;flex:none;color:${color}">
  <circle cx="50" cy="52" r="26" stroke="currentColor" stroke-width="1.6" stroke-dasharray="4 3.5" opacity="0.28"/>
  <path d="M28 64 C38 64 42 38 50 38 C58 38 62 64 72 64" stroke="currentColor" stroke-width="4.5" stroke-linecap="round"/>
  <path d="M34 74 C41 74 45 50 50 50 C55 50 59 74 66 74" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" opacity="0.35"/>
  <circle cx="28" cy="64" r="6.2" fill="currentColor"/>
  <circle cx="72" cy="64" r="4.8" fill="currentColor"/>
  <path d="M50 28 L56 38 L50 48 L44 38 Z" fill="currentColor"/>
</svg>`
}
