import { createApp } from 'vue'
import './styles.css'

const scriptStartTs = typeof performance !== 'undefined' ? performance.now() : Date.now()
console.info(`[startup-prof] main.ts script_start_ms=${scriptStartTs.toFixed?.(1) ?? scriptStartTs}`)
console.info(`[startup-prof] main.ts env mode=${import.meta.env.MODE} dev=${import.meta.env.DEV}`)

const startupTs = typeof performance !== 'undefined' ? performance.now() : Date.now()
console.info(`[startup-prof] main.ts before_createApp_ms=${startupTs.toFixed?.(1) ?? startupTs}`)

async function bootstrap() {
  if (import.meta.env.DEV) {
    void import('./devStressTools').then(({ installDevStressTools }) => {
      installDevStressTools()
    })
  }

  const importStartTs = typeof performance !== 'undefined' ? performance.now() : Date.now()
  console.info(`[startup-prof] main.ts before_import_app_ms=${importStartTs.toFixed?.(1) ?? importStartTs}`)
  const appModule = await import('./App.vue')
  const importEndTs = typeof performance !== 'undefined' ? performance.now() : Date.now()
  console.info(
    `[startup-prof] main.ts after_import_app_ms=${importEndTs.toFixed?.(1) ?? importEndTs} app_import_ms=${(importEndTs - importStartTs).toFixed?.(1) ?? (importEndTs - importStartTs)}`,
  )
  createApp(appModule.default).mount('#app')
  const mountedTs = typeof performance !== 'undefined' ? performance.now() : Date.now()
  console.info(`[startup-prof] main.ts mount_end_ms=${mountedTs.toFixed?.(1) ?? mountedTs}`)
}

void bootstrap()
