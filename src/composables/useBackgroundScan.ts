import { ref } from 'vue'

type BackgroundScanStatus = {
  running: boolean
}

type StartupCleanupStatus = {
  running: boolean
  generation: number
}

type BackgroundScanProgress = {
  running: boolean
  phase: string
  scannedFolders: number
  totalFolders: number
  newImages: number
  updatedImages: number
  skippedImages: number
  removedMissingImages: number
  queuedImages: number
  taggedImages: number
  failedImages: number
  lastError?: string | null
  recentErrors?: string[] | null
}

type UseBackgroundScanOptions = {
  loadLibrary: () => Promise<void>
  formatError: (error: unknown) => string
  setErrorText: (value: string) => void
  autoScanOnStartupStorageKey?: string
  pollIntervalMs?: number
}

const defaultAutoScanOnStartupStorageKey = 'illutag.autoScanOnStartup'

export function useBackgroundScan(options: UseBackgroundScanOptions) {
  const autoScanOnStartupStorageKey =
    options.autoScanOnStartupStorageKey ?? defaultAutoScanOnStartupStorageKey
  const pollIntervalMs = options.pollIntervalMs ?? 1200

  const autoScanOnStartup = ref(false)
  const isBackgroundScanRunning = ref(false)
  const scanProgressText = ref('')
  const scanRecentErrors = ref<string[]>([])

  const scanProgressPollTimer = ref<number | null>(null)
  const scanProgressSignature = ref('')
  const scanLibraryRefreshInFlight = ref(false)
  const scanLibraryRefreshAt = ref(0)
  const startupCleanupObservedGeneration = ref(0)

  function initAutoScanOnStartupFromStorage() {
    autoScanOnStartup.value = localStorage.getItem(autoScanOnStartupStorageKey) === 'true'
  }

  function setAutoScanOnStartup(value: boolean) {
    autoScanOnStartup.value = value
    localStorage.setItem(autoScanOnStartupStorageKey, String(value))
  }

  async function startScanAllFolders() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const started = await invoke<boolean>('start_scan_all_folders_with_tagging_command')
      if (started) {
        isBackgroundScanRunning.value = true
        scanProgressText.value = '扫描任务已启动'
      } else {
        scanProgressText.value = '扫描任务已在后台运行，已排队下一轮扫描'
      }
      await refreshBackgroundScanStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function startStartupCleanup() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke<boolean>('start_startup_cleanup_command')
      await refreshStartupCleanupStatus()
    } catch {
      // startup cleanup is best-effort; keep silent to avoid noisy cold-start errors
    }
  }

  async function refreshBackgroundScanStatus() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const wasRunning = isBackgroundScanRunning.value
      const status = await invoke<BackgroundScanStatus>('background_scan_status_command')
      isBackgroundScanRunning.value = Boolean(status.running)

      const progress = await invoke<BackgroundScanProgress>('background_scan_progress_command')
      isBackgroundScanRunning.value = Boolean(progress.running)
      scanProgressText.value = buildScanProgressText(progress)
      scanRecentErrors.value = Array.isArray(progress.recentErrors)
        ? progress.recentErrors.filter(
            (item): item is string => typeof item === 'string' && item.trim().length > 0,
          )
        : []

      const signature = [
        progress.running ? '1' : '0',
        progress.phase,
        progress.scannedFolders,
        progress.totalFolders,
        progress.newImages,
        progress.updatedImages,
        progress.skippedImages,
        progress.removedMissingImages,
        progress.queuedImages,
        progress.taggedImages,
        progress.failedImages,
      ].join('|')
      const changed = signature !== scanProgressSignature.value
      scanProgressSignature.value = signature

      const now = Date.now()
      const becameIdle = wasRunning && !progress.running
      const refreshIntervalMs =
        progress.phase === 'collecting' ? 900 : progress.phase === 'tagging' ? 2400 : 1200
      const shouldLiveRefresh =
        progress.running && changed && now - scanLibraryRefreshAt.value >= refreshIntervalMs
      if ((becameIdle || shouldLiveRefresh) && !scanLibraryRefreshInFlight.value) {
        scanLibraryRefreshInFlight.value = true
        scanLibraryRefreshAt.value = now
        try {
          await options.loadLibrary()
        } finally {
          scanLibraryRefreshInFlight.value = false
        }
      }
      await refreshStartupCleanupStatus()
    } catch (error) {
      if (isBackgroundScanRunning.value) {
        options.setErrorText(options.formatError(error))
      }
    }
  }

  async function refreshStartupCleanupStatus() {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const status = await invoke<StartupCleanupStatus>('startup_cleanup_status_command')
      const previousGeneration = startupCleanupObservedGeneration.value
      startupCleanupObservedGeneration.value = status.generation
      const cleanupJustFinished = !status.running && status.generation > previousGeneration
      if (!cleanupJustFinished || scanLibraryRefreshInFlight.value) return

      scanLibraryRefreshInFlight.value = true
      scanLibraryRefreshAt.value = Date.now()
      try {
        await options.loadLibrary()
      } finally {
        scanLibraryRefreshInFlight.value = false
      }
    } catch {
      // ignore startup cleanup polling failure to avoid disturbing normal scan progress flow
    }
  }

  function startBackgroundScanPolling() {
    stopBackgroundScanPolling()
    scanProgressPollTimer.value = window.setInterval(() => {
      void refreshBackgroundScanStatus()
    }, pollIntervalMs)
  }

  function stopBackgroundScanPolling() {
    if (scanProgressPollTimer.value === null) return
    window.clearInterval(scanProgressPollTimer.value)
    scanProgressPollTimer.value = null
  }

  function startAutoScanIfEnabled() {
    if (autoScanOnStartup.value) {
      void startScanAllFolders()
    }
  }

  function buildScanProgressText(progress: BackgroundScanProgress) {
    const phaseLabel = scanPhaseLabel(progress.phase)
    const folders = `${progress.scannedFolders}/${progress.totalFolders}`
    const tagged = `${progress.taggedImages}/${progress.queuedImages}`
    const base = `${phaseLabel}｜文件夹 ${folders}｜新增 ${progress.newImages}｜更新 ${progress.updatedImages}｜跳过 ${progress.skippedImages}｜清理 ${progress.removedMissingImages}｜打标 ${tagged}｜失败 ${progress.failedImages}`
    if (progress.lastError) {
      return `${base}｜错误：${progress.lastError}`
    }
    if (!progress.running && progress.phase === 'idle') {
      return `上次扫描完成｜${base}`
    }
    return base
  }

  function scanPhaseLabel(phase: string) {
    switch ((phase || '').toLowerCase()) {
      case 'collecting':
        return '扫描中'
      case 'tagging':
        return '打标中'
      case 'idle':
        return '空闲'
      default:
        return phase || '未知状态'
    }
  }

  return {
    autoScanOnStartup,
    isBackgroundScanRunning,
    scanProgressText,
    scanRecentErrors,
    initAutoScanOnStartupFromStorage,
    setAutoScanOnStartup,
    startScanAllFolders,
    startStartupCleanup,
    refreshBackgroundScanStatus,
    startBackgroundScanPolling,
    stopBackgroundScanPolling,
    startAutoScanIfEnabled,
  }
}
