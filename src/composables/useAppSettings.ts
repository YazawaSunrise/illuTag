import { ref, watch } from 'vue'

type ThemeMode = 'light' | 'dark'

type UseAppSettingsOptions = {
  sidebarPinnedStorageKey?: string
  rightSidebarPinnedStorageKey?: string
  autoFixRightSidebarOnPreviewStorageKey?: string
  themeModeStorageKey?: string
  thumbnailCacheEnabledStorageKey?: string
}

const defaultKeys = {
  sidebarPinned: 'illutag.sidebarPinned',
  rightSidebarPinned: 'illutag.rightSidebarPinned',
  autoFixRightSidebarOnPreview: 'illutag.autoFixRightSidebarOnPreview',
  themeMode: 'illutag.themeMode',
  thumbnailCacheEnabled: 'illutag.thumbnailCacheEnabled',
}

export function useAppSettings(options: UseAppSettingsOptions = {}) {
  const sidebarPinnedStorageKey = options.sidebarPinnedStorageKey ?? defaultKeys.sidebarPinned
  const rightSidebarPinnedStorageKey =
    options.rightSidebarPinnedStorageKey ?? defaultKeys.rightSidebarPinned
  const autoFixRightSidebarOnPreviewStorageKey =
    options.autoFixRightSidebarOnPreviewStorageKey ?? defaultKeys.autoFixRightSidebarOnPreview
  const themeModeStorageKey = options.themeModeStorageKey ?? defaultKeys.themeMode
  const thumbnailCacheEnabledStorageKey =
    options.thumbnailCacheEnabledStorageKey ?? defaultKeys.thumbnailCacheEnabled

  const sidebarPinned = ref(false)
  const rightSidebarPinned = ref(false)
  const autoFixRightSidebarOnPreview = ref(false)
  const themeMode = ref<ThemeMode>('light')
  const thumbnailCacheEnabled = ref(false)

  function applyTheme(value: ThemeMode) {
    document.documentElement.dataset.theme = value
  }

  function initAppSettingsFromStorage() {
    sidebarPinned.value = localStorage.getItem(sidebarPinnedStorageKey) === 'true'
    rightSidebarPinned.value = localStorage.getItem(rightSidebarPinnedStorageKey) === 'true'
    autoFixRightSidebarOnPreview.value =
      localStorage.getItem(autoFixRightSidebarOnPreviewStorageKey) === 'true'
    const storedTheme = localStorage.getItem(themeModeStorageKey)
    themeMode.value = storedTheme === 'dark' ? 'dark' : 'light'
    thumbnailCacheEnabled.value = localStorage.getItem(thumbnailCacheEnabledStorageKey) === 'true'
    applyTheme(themeMode.value)
  }

  function setSidebarPinned(value: boolean) {
    sidebarPinned.value = value
  }

  function setRightSidebarPinned(value: boolean) {
    rightSidebarPinned.value = value
  }

  function setAutoFixRightSidebarOnPreview(value: boolean) {
    autoFixRightSidebarOnPreview.value = value
  }

  function setThemeMode(value: ThemeMode) {
    themeMode.value = value
  }

  function setThumbnailCacheEnabled(value: boolean) {
    thumbnailCacheEnabled.value = value
  }

  watch(sidebarPinned, (value) => {
    localStorage.setItem(sidebarPinnedStorageKey, String(value))
  })

  watch(rightSidebarPinned, (value) => {
    localStorage.setItem(rightSidebarPinnedStorageKey, String(value))
  })

  watch(autoFixRightSidebarOnPreview, (value) => {
    localStorage.setItem(autoFixRightSidebarOnPreviewStorageKey, String(value))
  })

  watch(themeMode, (value) => {
    applyTheme(value)
    localStorage.setItem(themeModeStorageKey, value)
  })

  watch(thumbnailCacheEnabled, (value) => {
    localStorage.setItem(thumbnailCacheEnabledStorageKey, String(value))
  })

  return {
    sidebarPinned,
    rightSidebarPinned,
    autoFixRightSidebarOnPreview,
    themeMode,
    thumbnailCacheEnabled,
    initAppSettingsFromStorage,
    setSidebarPinned,
    setRightSidebarPinned,
    setAutoFixRightSidebarOnPreview,
    setThemeMode,
    setThumbnailCacheEnabled,
  }
}
