import { ref } from 'vue'

type ColorSignatureGenerationProgress = {
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

type UseColorSignatureGenerationOptions = {
  loadLibrary: () => Promise<void>
  formatError: (error: unknown) => string
  setErrorText: (value: string) => void
  pollIntervalMs?: number
}

export function useColorSignatureGeneration(options: UseColorSignatureGenerationOptions) {
  const pollIntervalMs = options.pollIntervalMs ?? 1200
  const libraryRefreshIntervalMs = 30_000

  const isColorSignatureGenerationRunning = ref(false)
  const isColorSignatureGenerationPaused = ref(false)
  const colorSignatureProgressText = ref('')
  const colorSignatureProgressPercent = ref(0)
  const colorSignatureRecentErrors = ref<string[]>([])

  const colorSignatureProgressPollTimer = ref<number | null>(null)
  const colorSignatureProgressSignature = ref('')
  const libraryRefreshInFlight = ref(false)
  const libraryRefreshAt = ref(0)

  async function startColorSignatureGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const started = await invoke<boolean>('start_color_signature_generation_command')
      colorSignatureProgressText.value = started
        ? '配色特征任务已启动'
        : '配色特征任务已在后台运行，已排队下一轮'
      await refreshColorSignatureGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function pauseColorSignatureGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('pause_color_signature_generation_command')
      await refreshColorSignatureGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function resumeColorSignatureGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('resume_color_signature_generation_command')
      await refreshColorSignatureGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function stopColorSignatureGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('stop_color_signature_generation_command')
      await refreshColorSignatureGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function rebuildColorSignatureCache() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('rebuild_color_signature_cache_command')
      await refreshColorSignatureGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function refreshColorSignatureGenerationStatus() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const wasRunning = isColorSignatureGenerationRunning.value
      const progress = await invoke<ColorSignatureGenerationProgress>('color_signature_generation_status_command')
      isColorSignatureGenerationRunning.value = Boolean(progress.running)
      isColorSignatureGenerationPaused.value = Boolean(progress.paused)
      colorSignatureProgressText.value = buildColorSignatureProgressText(progress)
      colorSignatureProgressPercent.value = progressPercent(progress)
      colorSignatureRecentErrors.value = Array.isArray(progress.recentErrors)
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
      const changed = signature !== colorSignatureProgressSignature.value
      colorSignatureProgressSignature.value = signature

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
      if (isColorSignatureGenerationRunning.value) {
        options.setErrorText(options.formatError(error))
      }
    }
  }

  function startColorSignatureGenerationPolling() {
    stopColorSignatureGenerationPolling()
    colorSignatureProgressPollTimer.value = window.setInterval(() => {
      void refreshColorSignatureGenerationStatus()
    }, pollIntervalMs)
  }

  function stopColorSignatureGenerationPolling() {
    if (colorSignatureProgressPollTimer.value === null) return
    window.clearInterval(colorSignatureProgressPollTimer.value)
    colorSignatureProgressPollTimer.value = null
  }

  function progressPercent(progress: ColorSignatureGenerationProgress) {
    const total = Math.max(0, Number(progress.totalCandidates) || 0)
    const processed = Math.max(0, Number(progress.processedImages) || 0)
    if (total <= 0) return progress.running ? 0 : 100
    return Math.max(0, Math.min(100, Math.round((processed / total) * 100)))
  }

  function buildColorSignatureProgressText(progress: ColorSignatureGenerationProgress) {
    const total = Math.max(0, Number(progress.totalCandidates) || 0)
    const processed = Math.max(0, Number(progress.processedImages) || 0)
    const generated = Math.max(0, Number(progress.generatedImages) || 0)
    const skipped = Math.max(0, Number(progress.skippedImages) || 0)
    const failed = Math.max(0, Number(progress.failedImages) || 0)
    const phaseLabel = colorSignaturePhaseLabel(progress.phase)
    const base = `${phaseLabel}：${processed}/${total}，生成 ${generated}，跳过 ${skipped}，失败 ${failed}`
    if (progress.lastError) {
      return `${base}，错误：${progress.lastError}`
    }
    if (!progress.running && progress.phase === 'idle') {
      return `上次配色特征处理完成：${base}`
    }
    return base
  }

  function colorSignaturePhaseLabel(phase: string) {
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
    isColorSignatureGenerationRunning,
    isColorSignatureGenerationPaused,
    colorSignatureProgressText,
    colorSignatureProgressPercent,
    colorSignatureRecentErrors,
    startColorSignatureGeneration,
    pauseColorSignatureGeneration,
    resumeColorSignatureGeneration,
    stopColorSignatureGeneration,
    rebuildColorSignatureCache,
    refreshColorSignatureGenerationStatus,
    startColorSignatureGenerationPolling,
    stopColorSignatureGenerationPolling,
  }
}
