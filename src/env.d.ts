/// <reference types="vite/client" />

declare const __APP_VERSION__: string

declare global {
  interface Window {
    illuTagDevStress?: import('./devStressTools').IlluTagDevStressTools
  }
}

declare module '*.vue' {
  import type { DefineComponent } from 'vue'

  const component: DefineComponent<object, object, unknown>
  export default component
}
