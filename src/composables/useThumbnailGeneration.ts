import { ref } from 'vue'

type ThumbnailGenerationProgress = {
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

type UseThumbnailGenerationOptions = {
  loadLibrary: () => Promise<void>
  formatError: (error: unknown) => string
  setErrorText: (value: string) => void
  pollIntervalMs?: number
}

export function useThumbnailGeneration(options: UseThumbnailGenerationOptions) {
  const pollIntervalMs = options.pollIntervalMs ?? 1200
  const libraryRefreshIntervalMs = 25_000

  const isThumbnailGenerationRunning = ref(false)
  const isThumbnailGenerationPaused = ref(false)
  const thumbnailProgressText = ref('')
  const thumbnailProgressPercent = ref(0)
  const thumbnailRecentErrors = ref<string[]>([])

  const thumbnailProgressPollTimer = ref<number | null>(null)
  const thumbnailProgressSignature = ref('')
  const libraryRefreshInFlight = ref(false)
  const libraryRefreshAt = ref(0)

  async function startThumbnailGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const started = await invoke<boolean>('start_thumbnail_generation_command')
      thumbnailProgressText.value = started ? '缩略图任务已启动' : '缩略图任务已在后台运行，已排队下一轮'
      await refreshThumbnailGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function pauseThumbnailGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('pause_thumbnail_generation_command')
      await refreshThumbnailGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function resumeThumbnailGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('resume_thumbnail_generation_command')
      await refreshThumbnailGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function stopThumbnailGeneration() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('stop_thumbnail_generation_command')
      await refreshThumbnailGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function clearThumbnailCache() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('clear_thumbnail_cache_command')
      await options.loadLibrary()
      await refreshThumbnailGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function rebuildThumbnailCache() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('rebuild_thumbnail_cache_command')
      await refreshThumbnailGenerationStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function refreshThumbnailGenerationStatus() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const wasRunning = isThumbnailGenerationRunning.value
      const progress = await invoke<ThumbnailGenerationProgress>('thumbnail_generation_status_command')
      isThumbnailGenerationRunning.value = Boolean(progress.running)
      isThumbnailGenerationPaused.value = Boolean(progress.paused)
      thumbnailProgressText.value = buildThumbnailProgressText(progress)
      thumbnailProgressPercent.value = progressPercent(progress)
      thumbnailRecentErrors.value = Array.isArray(progress.recentErrors)
        ? progress.recentErrors.filter(
            (item): item is string => typeof item === 'string' && item.trim().length > 0,
          )
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
      const changed = signature !== thumbnailProgressSignature.value
      thumbnailProgressSignature.value = signature

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
      if (isThumbnailGenerationRunning.value) {
        options.setErrorText(options.formatError(error))
      }
    }
  }

  function startThumbnailGenerationPolling() {
    stopThumbnailGenerationPolling()
    thumbnailProgressPollTimer.value = window.setInterval(() => {
      void refreshThumbnailGenerationStatus()
    }, pollIntervalMs)
  }

  function stopThumbnailGenerationPolling() {
    if (thumbnailProgressPollTimer.value === null) return
    window.clearInterval(thumbnailProgressPollTimer.value)
    thumbnailProgressPollTimer.value = null
  }

  function progressPercent(progress: ThumbnailGenerationProgress) {
    const total = Math.max(0, Number(progress.totalCandidates) || 0)
    const processed = Math.max(0, Number(progress.processedImages) || 0)
    if (total <= 0) return progress.running ? 0 : 100
    return Math.max(0, Math.min(100, Math.round((processed / total) * 100)))
  }

  function buildThumbnailProgressText(progress: ThumbnailGenerationProgress) {
    const total = Math.max(0, Number(progress.totalCandidates) || 0)
    const processed = Math.max(0, Number(progress.processedImages) || 0)
    const generated = Math.max(0, Number(progress.generatedImages) || 0)
    const skipped = Math.max(0, Number(progress.skippedImages) || 0)
    const failed = Math.max(0, Number(progress.failedImages) || 0)
    const phaseLabel = thumbnailPhaseLabel(progress.phase)
    const base = `${phaseLabel}｜${processed}/${total}｜新增 ${generated}｜跳过 ${skipped}｜失败 ${failed}`
    if (progress.lastError) {
      return `${base}｜错误：${progress.lastError}`
    }
    if (!progress.running && progress.phase === 'idle') {
      return `上次缩略图处理完成｜${base}`
    }
    return base
  }

  function thumbnailPhaseLabel(phase: string) {
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
    isThumbnailGenerationRunning,
    isThumbnailGenerationPaused,
    thumbnailProgressText,
    thumbnailProgressPercent,
    thumbnailRecentErrors,
    startThumbnailGeneration,
    pauseThumbnailGeneration,
    resumeThumbnailGeneration,
    stopThumbnailGeneration,
    clearThumbnailCache,
    rebuildThumbnailCache,
    refreshThumbnailGenerationStatus,
    startThumbnailGenerationPolling,
    stopThumbnailGenerationPolling,
  }
}
