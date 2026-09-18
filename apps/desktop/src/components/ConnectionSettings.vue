<script setup lang="ts">
import { computed } from 'vue'
import { AlertTriangle, Network } from '@lucide/vue'
import QrcodeVue from 'qrcode.vue'
import SideCard from '@/components/SideCard.vue'
import type { LocalAddress } from '@/types'

const props = defineProps<{
  addresses: readonly LocalAddress[]
  activeAddress: LocalAddress | null
  accessUrl: string
  accessUrlIndex: number
  addressUnreachable: boolean
  hasAlternateAddresses: boolean
  autoPairEnabled: boolean
  /** 是否需要配对码访问（服务端设置）；关闭时「扫码自动进入」没有意义。 */
  pairingRequired: boolean
  /** 服务未启动时没有可编码的地址，此时显示占位提示而不是空二维码。 */
  running: boolean
  /** 二维码内容：免配对时为纯地址；需配对且开「扫码自动进入」时携带配对码。 */
  qrUrl: string
}>()

const emit = defineEmits<{
  selectAddress: [url: string]
  toggleAutoPair: [enabled: boolean]
  togglePairing: [enabled: boolean]
}>()

/** 地址下拉选项：可达性分组标注，避免用户在不可达地址上来回试。 */
const addressOptions = computed(() =>
  props.addresses.map((address, index) => ({
    address,
    index,
    label: address.tier === 0 ? '局域网' : address.tier === 1 ? '其他网络' : '虚拟链路',
    detail: describeAddress(address),
  })),
)

function describeAddress(address: LocalAddress): string {
  const parts: string[] = []
  if (address.interface) parts.push(address.interface)
  if (address.prefix_len != null) parts.push(`/${address.prefix_len}`)
  return parts.join(' ')
}

function handleAddressChange(event: Event) {
  emit('selectAddress', (event.target as HTMLSelectElement).value)
}

function handleAutoPairChange(event: Event) {
  emit('toggleAutoPair', (event.target as HTMLInputElement).checked)
}

function handlePairingChange(event: Event) {
  emit('togglePairing', (event.target as HTMLInputElement).checked)
}
</script>

<template>
  <!-- 二维码从「命令条按钮 + 浮层」改为侧边栏常驻（用户指令）：
       地址切换决定二维码编码什么、扫码开关决定带不带配对码——
       三者本就是「手机怎么连进来」这一件事，故合并为一张卡，二维码是主角。 -->
  <SideCard title="扫码接入">
    <template v-if="props.running && props.qrUrl">
      <div class="qr-frame" :class="{ warn: props.addressUnreachable }">
        <QrcodeVue :value="props.qrUrl" :size="128" level="M" render-as="svg" />
      </div>

      <!-- 手机连不上当前地址时，给出可执行的下一步而不是静默失败 -->
      <p v-if="props.addressUnreachable" class="warn-note" role="status">
        <AlertTriangle :size="13" aria-hidden="true" />
        <span>当前地址属于虚拟网卡 / 点对点链路，手机通常无法直连{{ props.hasAlternateAddresses ? '，请切换到标注「局域网」的地址' : '' }}。</span>
      </p>
    </template>
    <p v-else class="idle">
      启动服务后，这里会显示扫码入口，手机扫码即可进入接收页。
    </p>

    <div v-if="props.running" class="field">
      <label class="field-label" for="side-access-address">访问地址</label>
      <div class="picker" :class="{ warn: props.addressUnreachable }">
        <Network :size="14" aria-hidden="true" />
        <select
          id="side-access-address"
          :value="props.accessUrl"
          :disabled="!props.hasAlternateAddresses"
          @change="handleAddressChange"
        >
          <option v-for="option in addressOptions" :key="option.address.url" :value="option.address.url">
            {{ option.label }} · {{ option.address.ip }}<template v-if="option.detail"> ({{ option.detail }})</template>
          </option>
        </select>
      </div>
      <p class="note">
        当前 {{ props.accessUrlIndex + 1 }} / {{ props.addresses.length }}
        <template v-if="props.activeAddress?.interface"> · {{ props.activeAddress.interface }}</template>
      </p>
    </div>

    <!-- 安全开关（服务端设置）：关掉后配对码不再拦截任何设备，扫码自动进入随之失去意义 -->
    <label class="switch">
      <span class="switch-copy">
        <strong>需要配对码访问</strong>
        <small>{{ props.pairingRequired ? '手机需输入配对码或扫带码二维码' : '任何设备打开地址即可直接访问' }}</small>
      </span>
      <input
        type="checkbox"
        :checked="props.pairingRequired"
        aria-label="是否需要配对码访问"
        @change="handlePairingChange"
      >
      <i aria-hidden="true" />
    </label>

    <label v-if="props.pairingRequired" class="switch">
      <span class="switch-copy">
        <strong>扫码自动进入</strong>
        <small>{{ props.autoPairEnabled ? '二维码已携带配对码，扫码直接进入' : '扫码后需手动输入配对码' }}</small>
      </span>
      <input
        type="checkbox"
        :checked="props.autoPairEnabled"
        aria-label="扫码时自动携带访问码"
        @change="handleAutoPairChange"
      >
      <i aria-hidden="true" />
    </label>
  </SideCard>
</template>

<style scoped>
/* 二维码框：白底是扫码对比度的硬要求；warn 时橙描边与地址框警示同源 */
.qr-frame { justify-self: center; width: fit-content; padding: 6px; border: 1px solid var(--line-strong); border-radius: 10px; background: #fff; }
.qr-frame.warn { border-color: var(--signal); }
.qr-frame :deep(svg) { display: block; width: 128px; height: 128px; }

.field { min-width: 0; }
.field-label { display: block; margin-bottom: 5px; color: var(--muted); font: 600 11.5px/1 var(--font-label); }
.picker { display: flex; align-items: center; gap: 7px; height: 34px; padding: 0 10px; border: 1px solid var(--line-strong); border-radius: 8px; background: #fff; }
.picker.warn { border-color: var(--signal); background: var(--signal-soft); }
.picker > svg { flex: none; display: block; color: var(--muted); }
.picker select { flex: 1; min-width: 0; height: 100%; border: 0; appearance: none; background: transparent; color: var(--ink); cursor: pointer; font: 550 11.5px/1.2 var(--font-mono); text-overflow: ellipsis; }
.picker select:disabled { cursor: default; color: var(--muted); }
.picker select:focus-visible { outline: 2px solid var(--info); outline-offset: -2px; }
.note { margin: 5px 0 0; color: var(--muted); font-size: 11px; line-height: 1.6; }
.idle { margin: 0; padding: 10px 11px; border: 1px dashed var(--line-strong); border-radius: 8px; background: var(--dock-0); color: var(--muted); font-size: 11.5px; line-height: 1.55; }
.warn-note { display: flex; align-items: flex-start; gap: 8px; margin: 0; padding: 8px 10px; border: 1px solid rgb(255 195 120 / 45%); border-radius: 8px; background: var(--signal-soft); color: var(--signal-deep); font-size: 11.5px; line-height: 1.5; }
.warn-note svg { flex: none; margin-top: 1px; }
.switch { display: flex; align-items: center; justify-content: space-between; gap: 10px; cursor: pointer; }
.switch-copy { min-width: 0; }
.switch-copy strong { display: block; color: var(--ink); font: 650 12.5px/1.3 var(--font-label); }
.switch-copy small { display: block; margin-top: 4px; color: var(--muted); font-size: 11px; line-height: 1.5; }
.switch input { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); }
.switch i { position: relative; width: 36px; height: 21px; flex: none; border-radius: 999px; background: var(--line-strong); transition: background 160ms ease; }
.switch i::after { content: ''; position: absolute; top: 2px; left: 2px; width: 17px; height: 17px; border-radius: 50%; background: #fff; transition: left 160ms cubic-bezier(.22, 1, .36, 1); }
/* 开关用 info 蓝：与「状态 / 开关」语义一致，acid 绿留给品牌行动 */
.switch input:checked + i { background: var(--info-lit); }
.switch input:checked + i::after { left: 17px; background: var(--ink); }
.switch input:focus-visible + i { outline: 3px solid color-mix(in srgb, var(--info-lit) 80%, transparent); outline-offset: 2px; }
</style>
