<script setup lang="ts">
import { ArrowRight, RadioTower, RefreshCw, ShieldCheck } from '@lucide/vue'
import { computed, shallowRef } from 'vue'

const props = defineProps<{
  serverName: string
  loading: boolean
  offline: boolean
  error: string
}>()

const emit = defineEmits<{
  submit: [code: string]
  retry: []
}>()

const code = shallowRef('')
const canSubmit = computed(() => code.value.length === 6 && !props.loading)

function handleInput(event: Event): void {
  const input = event.target as HTMLInputElement
  code.value = input.value.replace(/\D/g, '').slice(0, 6)
  input.value = code.value
}

function submit(): void {
  if (canSubmit.value)
    emit('submit', code.value)
}
</script>

<template>
  <main class="pairing-page">
    <div class="grid-noise" aria-hidden="true" />
    <section class="pairing-card" aria-labelledby="pairing-title">
      <div class="pairing-intro">
        <div class="route-mark" aria-hidden="true">
          <span class="route-dot" />
          <span class="route-line" />
          <span class="route-dot destination" />
        </div>
        <p class="station-label">LANE / LOCAL DELIVERY</p>
        <h1 id="pairing-title">近在同一张<br><em>网络里。</em></h1>
        <p class="intro-copy">
          无需上传云端，也不用安装客户端。输入主机屏幕上的配对码，接入这条局域网传输通道。
        </p>
        <div class="privacy-note">
          <ShieldCheck :size="18" aria-hidden="true" />
          <span>数据留在本地网络<br><small>关闭主机程序即断开</small></span>
        </div>
      </div>

      <div class="ticket-panel">
        <div class="ticket-topline"><span>ACCESS TICKET</span><span>NO. 01—LAN</span></div>
        <div class="station-name">
          <RadioTower :size="18" aria-hidden="true" />
          <span>正在连接</span>
          <strong>{{ serverName }}</strong>
        </div>

        <div v-if="offline" class="offline-panel" role="alert">
          <p>暂时无法连接主机，请确认服务仍在运行。</p>
          <button type="button" @click="emit('retry')">
            <RefreshCw :size="15" /> 重新连接
          </button>
        </div>

        <form v-else class="pairing-form" @submit.prevent="submit">
          <label for="pair-code">六位配对码</label>
          <input
            id="pair-code"
            :value="code"
            class="code-input"
            type="text"
            inputmode="numeric"
            autocomplete="one-time-code"
            maxlength="6"
            placeholder="000000"
            autofocus
            aria-describedby="pair-code-hint"
            @input="handleInput"
          >
          <p id="pair-code-hint" class="form-hint">配对码显示在运行 LANE 的主机界面或终端中</p>
          <p v-if="error" class="form-error" role="alert">{{ error }}</p>
          <button class="connect-button" type="submit" :disabled="!canSubmit">
            <span>{{ loading ? '正在验票' : '进入文件站' }}</span>
            <ArrowRight :size="19" aria-hidden="true" />
          </button>
        </form>

        <div class="ticket-footer" aria-hidden="true">
          <span v-for="index in 28" :key="index" />
        </div>
      </div>
    </section>
  </main>
</template>

<style scoped>
.pairing-page { min-height: 100dvh; padding: 40px 20px; display: grid; place-items: center; position: relative; overflow: hidden; background: var(--ink); }
.grid-noise { position: absolute; inset: 0; opacity: .16; background-image: linear-gradient(rgb(255 255 255 / 20%) 1px, transparent 1px), linear-gradient(90deg, rgb(255 255 255 / 20%) 1px, transparent 1px); background-size: 48px 48px; mask-image: linear-gradient(to bottom right, #000, transparent 70%); }
.pairing-card { z-index: 1; position: relative; display: grid; grid-template-columns: 1.08fr .92fr; width: min(1040px, 100%); border: 1px solid rgb(255 255 255 / 20%); box-shadow: 0 30px 90px rgb(0 0 0 / 34%); }
.pairing-intro { min-height: 590px; padding: clamp(40px, 7vw, 78px); color: var(--paper); background: var(--ink); }
.route-mark { width: 128px; margin-bottom: 60px; display: flex; align-items: center; }
.route-dot { width: 13px; height: 13px; border: 2px solid var(--acid); border-radius: 50%; }
.route-dot.destination { background: var(--acid); }
.route-line { flex: 1; height: 1px; background: var(--acid); }
.station-label { margin: 0 0 20px; color: var(--acid); font: 650 11px/1 var(--font-label); letter-spacing: .2em; }
.pairing-intro h1 { margin: 0; font: 720 clamp(50px, 6vw, 76px)/.92 var(--font-display); letter-spacing: -.055em; }
.pairing-intro h1 em { color: var(--acid); font-style: normal; }
.intro-copy { max-width: 430px; margin: 28px 0 0; color: rgb(244 241 232 / 64%); font-size: 15px; line-height: 1.8; }
.privacy-note { margin-top: 74px; display: flex; align-items: flex-start; gap: 12px; color: var(--paper); font-size: 12px; line-height: 1.5; }
.privacy-note small { color: rgb(244 241 232 / 45%); }
.ticket-panel { min-width: 0; padding: 32px clamp(28px, 5vw, 54px) 48px; display: flex; flex-direction: column; position: relative; color: var(--ink); background: var(--paper); }
.ticket-topline { padding-bottom: 22px; display: flex; justify-content: space-between; border-bottom: 1px dashed var(--line-strong); color: var(--muted); font: 650 9px/1 var(--font-label); letter-spacing: .17em; }
.station-name { margin-top: 48px; display: grid; grid-template-columns: auto 1fr; gap: 4px 10px; }
.station-name svg { grid-row: 1 / 3; margin-top: 3px; color: var(--signal); }
.station-name span { color: var(--muted); font-size: 11px; }
.station-name strong { overflow: hidden; font: 680 18px/1.2 var(--font-display); text-overflow: ellipsis; white-space: nowrap; }
.pairing-form { margin-top: 50px; }
.pairing-form label { display: block; margin-bottom: 11px; font: 650 11px/1 var(--font-label); letter-spacing: .1em; }
.code-input { width: 100%; padding: 15px 4px 12px; border: 0; border-bottom: 3px solid var(--ink); outline: none; color: var(--ink); background: transparent; caret-color: var(--signal); font: 700 clamp(38px, 5vw, 56px)/1 var(--font-mono); letter-spacing: .22em; }
.code-input:focus { border-color: var(--signal); }
.code-input::placeholder { color: color-mix(in srgb, var(--ink) 12%, transparent); }
.form-hint, .form-error { margin: 10px 0 0; font-size: 11px; line-height: 1.5; }
.form-hint { color: var(--muted); }
.form-error { color: #a32d13; }
.connect-button { width: 100%; margin-top: 30px; padding: 16px 18px; display: flex; align-items: center; justify-content: space-between; border: 1px solid var(--ink); color: var(--paper); background: var(--ink); cursor: pointer; font: 650 14px/1 var(--font-display); transition: transform .15s, box-shadow .15s, background .15s; }
.connect-button:not(:disabled):hover { transform: translate(-3px, -3px); box-shadow: 5px 5px 0 var(--signal); }
.connect-button:disabled { opacity: .35; cursor: not-allowed; }
.offline-panel { margin-top: 72px; padding: 18px; border: 1px solid var(--signal); background: var(--signal-soft); }
.offline-panel p { margin: 0 0 14px; font-size: 13px; line-height: 1.6; }
.offline-panel button { padding: 0; display: inline-flex; align-items: center; gap: 7px; border: 0; color: var(--ink); background: transparent; cursor: pointer; font-weight: 700; }
.ticket-footer { margin-top: auto; padding-top: 36px; display: flex; justify-content: space-between; gap: 3px; }
.ticket-footer span { width: 3px; height: 28px; background: var(--ink); }
.ticket-footer span:nth-child(3n) { width: 1px; }
.ticket-footer span:nth-child(5n) { height: 20px; }
@media (max-width: 760px) {
  .pairing-page { padding: 0; place-items: start; }
  .pairing-card { min-height: 100dvh; grid-template-columns: 1fr; border: 0; }
  .pairing-intro { min-height: auto; padding: 42px 26px 36px; }
  .route-mark { margin-bottom: 38px; }
  .pairing-intro h1 { font-size: 52px; }
  .privacy-note { display: none; }
  .ticket-panel { min-height: 430px; padding: 28px 26px 38px; }
  .station-name { margin-top: 30px; }
  .pairing-form { margin-top: 34px; }
}
</style>
