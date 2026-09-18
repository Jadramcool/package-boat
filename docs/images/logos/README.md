# PacketBoat 品牌资产（PB-05 私域航迹）

正式方案：**PB-05 Private Wake / 私域航迹**

隐喻：主机与浏览器在局域网内直达，数据不经云端——双节点 + 航迹弧 + 私域环。

## 色板

| 名称 | 色值 | 用途 |
|---|---|---|
| 深海墨 | `#1f271c` | 底板 / 正文 |
| 航速绿 | `#d7f954` | 品牌主色 / 强调 |
| 船坞白 | `#fbfcf7` | 深底上的标记色 |

## 文件

| 路径 | 说明 |
|---|---|
| `apps/desktop/src/assets/brand-mark.svg` | UI 用 currentColor 单色标 |
| `apps/desktop/src/components/BrandMark.vue` | 前端组件（桌面 + 浏览器共用） |
| `docs/images/logos/packetboat-private-wake.svg` | 正式品牌标（色板锁定） |
| `docs/images/logos/packetboat-private-wake-512.png` | 512px 位图 |
| `docs/images/logos/packetboat-private-wake-1024.png` | 1024px 位图 |
| `docs/images/app-icon.png` / `crates/packetboat-desktop/icons/*` | 应用图标 |
| `design/logos/svg/pb-05-app-icon.svg` | 小尺寸加粗版图标源文件 |

## 替换记录

`logo-A`…`logo-D` 为早期 AI 探索稿，已不再作为产品图标。正式采用见上表。

重新生成图标：

```powershell
node scripts/generate-brand-icons.mjs
```
