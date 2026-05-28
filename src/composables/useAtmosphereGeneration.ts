import { ref } from 'vue'

type AtmosphereGenerationProgress = {
  running: boolean
  paused: boolean
  phase: string
  totalCandidates: number
  processedImages: number
  generatedImages: number
  skippedImages: number
  failedImages: number
  lastError?: string | null
  recentErrors?: string[] | null
}

type UseAtmosphereGenerationOptions = {
  loadLibrary: () => Promise<void>
  formatError: (error: unknown) => string
  setErrorText: (value: string) => void
  pollIntervalMs?: number
}

export function useAtmosphereGeneration(options: UseAtmosphereGenerationOptions) {
  const pollIntervalMs = options.pollIntervalMs ?? 1200
  const libraryRefreshIntervalMs = 30_000

  const isAtmosphereGenerationRunning = ref(false)
  const isAtmosphereGenerationPaused = ref(false)
  const atmosphereProgressText = ref('')
  const atmosphereProgressPercent = ref(0)
  const atmosphereRecentErrors = ref<string[]>([])

  const atmosphereProgressPollTimer = ref<number | null>(null)
  const atmosphereProgressSignature = ref('')
  const libraryRefreshInFlight = ref(false)
  const libraryRefreshAt = ref(0)

  async function startAtmosphereGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const started = await invoke<boolean>('start_atmosphere_generation_command')
      atmosphereProgressText.value = started ? '氛围特征任务已启动' : '氛围特征任务已在后台运行，已排队下一轮'
      await refreshAtmosphereGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function pauseAtmosphereGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('pause_atmosphere_generation_command')
      await refreshAtmosphereGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function resumeAtmosphereGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('resume_atmosphere_generation_command')
      await refreshAtmosphereGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function stopAtmosphereGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('stop_atmosphere_generation_command')
      await refreshAtmosphereGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function rebuildAtmosphereSignatureCache() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const started = await invoke<boolean>('rebuild_atmosphere_signature_cache_command')
      atmosphereProgressText.value = started ? '氛围特征重建任务已启动' : '氛围特征任务已在后台运行，已排队下一轮'
      await refreshAtmosphereGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function refreshAtmosphereGenerationStatus() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const wasRunning = isAtmosphereGenerationRunning.value
      const progress = await invoke<AtmosphereGenerationProgress>('atmosphere_generation_status_command')
      isAtmosphereGenerationRunning.value = Boolean(progress.running)
      isAtmosphereGenerationPaused.value = Boolean(progress.paused)
      atmosphereProgressText.value = buildAtmosphereProgressText(progress)
      atmosphereProgressPercent.value = progressPercent(progress)
      atmosphereRecentErrors.value = Array.isArray(progress.recentErrors)
        ? progress.recentErrors.filter((item): item is string => typeof item === 'string' && item.trim().length > 0)
        : []

      const signature = [
        progress.running ? '1' : '0',
        progress.paused ? '1' : '0',
        progress.phase,
        progress.totalCandidates,
        progress.processedImages,
        progress.generatedImages,
        progress.skippedImages,
        progress.failedImages,
      ].join('|')
      const changed = signature !== atmosphereProgressSignature.value
      atmosphereProgressSignature.value = signature

      const now = Date.now()
      const becameIdle = wasRunning && !progress.running
      const shouldLiveRefresh =
        progress.running &&
        changed &&
        !progress.paused &&
        now - libraryRefreshAt.value >= libraryRefreshIntervalMs
      if ((becameIdle || shouldLiveRefresh) && !libraryRefreshInFlight.value) {
        libraryRefreshInFlight.value = true
        libraryRefreshAt.value = now
        try {
          await options.loadLibrary()
        } finally {
          libraryRefreshInFlight.value = false
        }
      }
    } catch (error) {
      if (isAtmosphereGenerationRunning.value) {
        options.setErrorText(options.formatError(error))
      }
    }
  }

  function startAtmosphereGenerationPolling() {
    stopAtmosphereGenerationPolling()
    atmosphereProgressPollTimer.value = window.setInterval(() => {
      void refreshAtmosphereGenerationStatus()
    }, pollIntervalMs)
  }

  function stopAtmosphereGenerationPolling() {
    if (atmosphereProgressPollTimer.value === null) return
    window.clearInterval(atmosphereProgressPollTimer.value)
    atmosphereProgressPollTimer.value = null
  }

  function progressPercent(progress: AtmosphereGenerationProgress) {
    const total = Math.max(0, Number(progress.totalCandidates) || 0)
    const processed = Math.max(0, Number(progress.processedImages) || 0)
    if (total <= 0) return progress.running ? 0 : 100
    return Math.max(0, Math.min(100, Math.round((processed / total) * 100)))
  }

  function buildAtmosphereProgressText(progress: AtmosphereGenerationProgress) {
    const total = Math.max(0, Number(progress.totalCandidates) || 0)
    const processed = Math.max(0, Number(progress.processedImages) || 0)
    const generated = Math.max(0, Number(progress.generatedImages) || 0)
    const skipped = Math.max(0, Number(progress.skippedImages) || 0)
    const failed = Math.max(0, Number(progress.failedImages) || 0)
    const phaseLabel = atmospherePhaseLabel(progress.phase)
    const base = `${phaseLabel}：${processed}/${total}，生成 ${generated}，跳过 ${skipped}，失败 ${failed}`
    if (progress.lastError) {
      return `${base}，错误：${progress.lastError}`
    }
    if (!progress.running && progress.phase === 'idle') {
      return `上次氛围特征处理完成：${base}`
    }
    return base
  }

  function atmospherePhaseLabel(phase: string) {
    switch ((phase || '').toLowerCase()) {
      case 'queueing':
        return '排队中'
      case 'collecting':
        return '收集中'
      case 'generating':
        return '生成中'
      case 'paused':
        return '已暂停'
      case 'stopping':
        return '停止中'
      case 'idle':
        return '空闲'
      default:
        return phase || '未知状态'
    }
  }

  return {
    isAtmosphereGenerationRunning,
    isAtmosphereGenerationPaused,
    atmosphereProgressText,
    atmosphereProgressPercent,
    atmosphereRecentErrors,
    startAtmosphereGeneration,
    pauseAtmosphereGeneration,
    resumeAtmosphereGeneration,
    stopAtmosphereGeneration,
    rebuildAtmosphereSignatureCache,
    refreshAtmosphereGenerationStatus,
    startAtmosphereGenerationPolling,
    stopAtmosphereGenerationPolling,
  }
}
