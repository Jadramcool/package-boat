import { fileURLToPath, URL } from 'node:url'
import pluginVue from 'eslint-plugin-vue'
import { defineConfigWithVueTs, vueTsConfigs } from '@vue/eslint-config-typescript'

export default defineConfigWithVueTs(
  {
    ignores: ['dist/**', 'node_modules/**', 'src/bindings.ts'],
  },
  pluginVue.configs['flat/essential'],
  vueTsConfigs.recommended,
  {
    files: ['src/**/*.{ts,vue}'],
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: fileURLToPath(new URL('.', import.meta.url)),
      },
    },
  },
)
