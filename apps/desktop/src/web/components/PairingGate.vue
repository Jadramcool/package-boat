<script setup lang="ts">
import { RefreshCw, ShieldCheck } from '@lucide/vue'
import BrandMark from '@/components/BrandMark.vue'
import { computed, onUnmounted, shallowRef, watch } from 'vue'

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
const focused = shallowRef(false)
const cooldown = shallowRef(0)
let cooldownTimer: ReturnType<typeof setInterval> | undefined

const canSubmit = computed(() =>
  code.value.length === 6 && !props.loading && cooldown.value === 0)

function handleInput(event: Event): void {
  const input = event.target as HTMLInputElement
  code.value = input.value.replace(/\D/g, '').slice(0, 6)
  input.value = code.value
}

function submit(): void {
  if (canSubmit.value)
    emit('submit', code.value)
}

// 配对失败：清空输入并短暂冷却，避免原样重复提交；服务端另有按 IP 限速兜底
watch(() => props.error, (error) => {
  if (!error)
    return
  code.value = ''
  cooldown.value = 3
  clearInterval(cooldownTimer)
  cooldownTimer = setInterval(() => {
    cooldown.value = Math.max(0, cooldown.value - 1)
    if (cooldown.value === 0)
      clearInterval(cooldownTimer)
  }, 1000)
})

onUnmounted(() => clearInterval(cooldownTimer))
</script>

<template>
  <!-- 配对门：单屏只做一件事——输入配对码 -->
  <main class="pairing-page">
    <section class="gate" aria-labelledby="pairing-title">
      <div class="gate-logo" aria-hidden="true"><BrandMark :size="42" /></div>
      <h1 id="pairing-title">连接到 <em :title="props.serverName">{{ props.serverName }}</em></h1>
      <p class="gate-sub">
        <ShieldCheck :size="15" aria-hidden="true" />
        <span>输入主机屏幕上的六位配对码 · 文件只在当前局域网流动，不经云端</span>
      </p>

      <div v-if="props.offline" class="offline-panel" role="alert">
        <p>暂时无法连接主机，请确认服务仍在运行。</p>
        <button type="button" @click="emit('retry')">
          <RefreshCw :size="15" /> 重新连接
        </button>
      </div>

      <form v-else class="pairing-form" @submit.prevent="submit">
        <label for="pair-code" class="visually-hidden">六位配对码</label>
        <div class="otp" :class="{ 'has-error': Boolean(props.error) }" aria-hidden="false">
          <i
            v-for="n in 6"
            :key="n"
            :class="{
              fill: code.length >= n,
              cursor: code.length < 6 && (code.length === n - 1) && (focused || code.length > 0),
            }"
          ><span v-if="code.length >= n">{{ code[n - 1] }}</span></i>
          <input
            id="pair-code"
            class="otp-input"
            :value="code"
            type="text"
            inputmode="numeric"
            autocomplete="one-time-code"
            maxlength="6"
            placeholder=""
            autofocus
            aria-describedby="pair-code-hint"
            @input="handleInput"
            @focus="focused = true"
            @blur="focused = false"
          >
        </div>
        <p id="pair-code-hint" class="form-hint">配对码显示在运行 PacketBoat 的主机界面或终端中</p>
        <p v-if="props.error" class="form-error" role="alert">{{ props.error }}</p>
        <button class="connect-button" type="submit" :disabled="!canSubmit">
          <span>{{ props.loading ? '正在验票…' : cooldown > 0 ? `${cooldown}s 后可重试` : '进入投递站' }}</span>
          <span class="arrow" aria-hidden="true">→</span>
        </button>
      </form>

      <p class="gate-alt">
        连不上？确认手机与电脑在<b>同一 Wi-Fi</b>
        <span v-if="!props.offline" class="dot-sep" aria-hidden="true">·</span>
        <button v-if="!props.offline" type="button" class="retry-link" @click="emit('retry')">重新连接</button>
      </p>
    </section>
  </main>
</template>

<style scoped>
.pairing-page {
  min-height: 100dvh;
  padding: 24px 20px;
  display: grid;
  /* 轨道锁死为 minmax(0,1fr)：否则长设备名（nowrap）的 min-content 会沿
     em → h1 → .gate 逐级撑大 auto 轨道，整页横向溢出，em 的省略号永远不生效 */
  grid-template-columns: minmax(0, 1fr);
  place-items: center;
  background:
    radial-gradient(circle at 74% 7%, rgb(215 249 84 / 16%), transparent 24rem),
    var(--dock-0);
}

.gate { width: min(430px, 100%); display: flex; flex-direction: column; align-items: center; text-align: center; }
.gate-logo {
  width: 76px; height: 76px; margin-bottom: 22px;
  display: grid; place-items: center;
  border: 2px solid var(--acid); border-radius: 20px;
  background: var(--ink); color: var(--acid);
  box-shadow: 0 12px 28px rgb(31 39 28 / 22%);
}
.gate h1 { margin: 0; max-width: 100%; display: flex; flex-wrap: wrap; justify-content: center; align-items: baseline; gap: 4px 8px; font: 720 27px/1.2 var(--font-display); letter-spacing: -.01em; }
.gate h1 em { min-width: 0; max-width: 100%; overflow: hidden; font-style: normal; white-space: nowrap; text-overflow: ellipsis; color: var(--acid-deep); background: var(--acid-wash); padding: 1px 9px; border-radius: 7px; }
/* 多行说明：图标对齐首行，不用 flex 居中（会把盾牌钉在两行文字中间） */
.gate-sub { display: flex; align-items: flex-start; justify-content: center; gap: 7px; margin: 13px 0 0; color: var(--muted); font-size: 13px; line-height: 1.6; }
.gate-sub svg { flex: none; display: block; margin-top: 0.28em; color: var(--ok); }

.pairing-form { width: 100%; margin-top: 30px; display: flex; flex-direction: column; }
.otp { position: relative; display: flex; justify-content: center; gap: 9px; }
.otp i {
  position: relative; width: 46px; height: 58px;
  display: grid; place-items: center;
  border: 1.5px solid var(--line-strong); border-radius: 12px;
  background: #fff; font-style: normal;
  transition: border-color 150ms ease, box-shadow 150ms ease, transform 150ms ease;
}
.otp i span { font: 700 26px/1 var(--font-mono); }
.otp i.fill { border-color: var(--ink); box-shadow: 0 2px 0 var(--ink); }
.otp i.cursor { border-color: var(--info); box-shadow: 0 0 0 3px rgb(47 111 211 / 16%); }
.otp i.cursor::after { content: ''; position: absolute; width: 2px; height: 26px; background: var(--info); animation: blink 1.1s steps(1) infinite; }
.otp.has-error i { border-color: var(--signal); }
@keyframes blink { 50% { opacity: 0; } }
.otp-input {
  position: absolute; inset: 0; width: 100%;
  border: 0; outline: none; background: transparent;
  color: transparent; caret-color: transparent;
  font: 700 26px/1 var(--font-mono); letter-spacing: 1.9rem; text-align: center;
}

.form-hint { margin: 14px 0 0; color: var(--muted); font-size: 12px; line-height: 1.5; }
.form-error { margin: 9px 0 0; color: var(--signal-deep); font-size: 12.5px; line-height: 1.5; }

.connect-button {
  width: 100%; height: 52px; margin-top: 22px;
  display: flex; align-items: center; justify-content: space-between;
  padding: 0 20px;
  border: 1px solid var(--ink); border-radius: 12px;
  background: var(--acid); color: var(--ink);
  cursor: pointer; font: 720 15px/1 var(--font-label); letter-spacing: .02em;
  transition: background 150ms ease, transform 150ms ease, box-shadow 150ms ease, opacity 150ms ease;
  box-shadow: 0 3px 0 var(--ink);
}
.connect-button:not(:disabled):hover { background: var(--acid-hi); transform: translateY(-1px); box-shadow: 0 4px 0 var(--ink); }
.connect-button:not(:disabled):active { transform: translateY(2px); box-shadow: 0 0 0 var(--ink); }
.connect-button:disabled { opacity: .4; cursor: not-allowed; box-shadow: none; }
.arrow { font-family: var(--font-display); font-size: 17px; }

.offline-panel {
  width: 100%; margin-top: 30px; padding: 18px 20px;
  border: 1px solid var(--signal); border-radius: 12px;
  background: var(--signal-soft); color: var(--signal-deep); text-align: left;
}
.offline-panel p { margin: 0 0 12px; font-size: 13px; line-height: 1.6; }
.offline-panel button { display: inline-flex; align-items: center; gap: 7px; padding: 0; border: 0; background: transparent; color: var(--signal-deep); cursor: pointer; font: 700 13px/1.2 var(--font-label); }
.offline-panel button svg { flex: none; display: block; }
.offline-panel button:hover { text-decoration: underline; }

.gate-alt { display: flex; flex-wrap: wrap; align-items: center; justify-content: center; gap: 6px; margin: 26px 0 0; color: var(--muted); font-size: 12px; }
.gate-alt b { color: var(--ink); }
.dot-sep { color: var(--line-strong); }
.retry-link { padding: 0; border: 0; background: transparent; color: var(--info); cursor: pointer; font: 600 12px/1 var(--font-sans); }
.retry-link:hover { text-decoration: underline; }

@media (max-width: 400px) {
  .otp { gap: 6px; }
  .otp i { width: 42px; height: 54px; }
  .gate h1 { font-size: 23px; }
}
@media (prefers-reduced-motion: reduce) {
  .otp i.cursor::after { animation: none; }
}
</style>
