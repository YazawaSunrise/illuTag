import { ref, watch } from 'vue'

type ThemeMode = 'light' | 'dark'
export type FolderImageDropMode = 'copy' | 'move'

type UseAppSettingsOptions = {
  sidebarPinnedStorageKey?: string
  autoHideTitlebarInWindowModeStorageKey?: string
  rightSidebarPinnedStorageKey?: string
  autoFixRightSidebarOnPreviewStorageKey?: string
  themeModeStorageKey?: string
  thumbnailCacheEnabledStorageKey?: string
  showGalleryScrollbarStorageKey?: string
  galleryScrollbarSidebarModeStorageKey?: string
}

const defaultKeys = {
  sidebarPinned: 'illutag.sidebarPinned',
  autoHideTitlebarInWindowMode: 'illutag.autoHideTitlebarInWindowMode',
  rightSidebarPinned: 'illutag.rightSidebarPinned',
  autoFixRightSidebarOnPreview: 'illutag.autoFixRightSidebarOnPreview',
  themeMode: 'illutag.themeMode',
  thumbnailCacheEnabled: 'illutag.thumbnailCacheEnabled',
  showGalleryScrollbar: 'illutag.showGalleryScrollbar',
  galleryScrollbarSidebarMode: 'illutag.galleryScrollbarSidebarMode',
  folderImageDropMode: 'illutag.folderImageDropMode',
}

export function useAppSettings(options: UseAppSettingsOptions = {}) {
  const sidebarPinnedStorageKey = options.sidebarPinnedStorageKey ?? defaultKeys.sidebarPinned
  const autoHideTitlebarInWindowModeStorageKey =
    options.autoHideTitlebarInWindowModeStorageKey ?? defaultKeys.autoHideTitlebarInWindowMode
  const rightSidebarPinnedStorageKey =
    options.rightSidebarPinnedStorageKey ?? defaultKeys.rightSidebarPinned
  const autoFixRightSidebarOnPreviewStorageKey =
    options.autoFixRightSidebarOnPreviewStorageKey ?? defaultKeys.autoFixRightSidebarOnPreview
  const themeModeStorageKey = options.themeModeStorageKey ?? defaultKeys.themeMode
  const thumbnailCacheEnabledStorageKey =
    options.thumbnailCacheEnabledStorageKey ?? defaultKeys.thumbnailCacheEnabled
  const showGalleryScrollbarStorageKey =
    options.showGalleryScrollbarStorageKey ?? defaultKeys.showGalleryScrollbar
  const galleryScrollbarSidebarModeStorageKey =
    options.galleryScrollbarSidebarModeStorageKey ?? defaultKeys.galleryScrollbarSidebarMode

  const sidebarPinned = ref(false)
  const autoHideTitlebarInWindowMode = ref(false)
  const rightSidebarPinned = ref(false)
  const autoFixRightSidebarOnPreview = ref(false)
  const themeMode = ref<ThemeMode>('light')
  const thumbnailCacheEnabled = ref(false)
  const showGalleryScrollbar = ref(false)
  const galleryScrollbarSidebarMode = ref<'hidden' | 'fixed'>('hidden')
  const folderImageDropMode = ref<FolderImageDropMode>('copy')

  function applyTheme(value: ThemeMode) {
    document.documentElement.dataset.theme = value
  }

  function initAppSettingsFromStorage() {
    sidebarPinned.value = localStorage.getItem(sidebarPinnedStorageKey) === 'true'
    autoHideTitlebarInWindowMode.value =
      localStorage.getItem(autoHideTitlebarInWindowModeStorageKey) === 'true'
    rightSidebarPinned.value = localStorage.getItem(rightSidebarPinnedStorageKey) === 'true'
    autoFixRightSidebarOnPreview.value =
      localStorage.getItem(autoFixRightSidebarOnPreviewStorageKey) === 'true'
    const storedTheme = localStorage.getItem(themeModeStorageKey)
    themeMode.value = storedTheme === 'dark' ? 'dark' : 'light'
    thumbnailCacheEnabled.value = localStorage.getItem(thumbnailCacheEnabledStorageKey) === 'true'
    showGalleryScrollbar.value = localStorage.getItem(showGalleryScrollbarStorageKey) === 'true'
    galleryScrollbarSidebarMode.value =
      localStorage.getItem(galleryScrollbarSidebarModeStorageKey) === 'fixed' ? 'fixed' : 'hidden'
    folderImageDropMode.value =
      localStorage.getItem(defaultKeys.folderImageDropMode) === 'move' ? 'move' : 'copy'
    applyTheme(themeMode.value)
  }

  function setSidebarPinned(value: boolean) {
    sidebarPinned.value = value
  }

  function setAutoHideTitlebarInWindowMode(value: boolean) {
    autoHideTitlebarInWindowMode.value = value
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

  function setShowGalleryScrollbar(value: boolean) {
    showGalleryScrollbar.value = value
  }

  function setGalleryScrollbarSidebarMode(value: 'hidden' | 'fixed') {
    galleryScrollbarSidebarMode.value = value
  }

  function setFolderImageDropMode(value: FolderImageDropMode) {
    folderImageDropMode.value = value
    localStorage.setItem(defaultKeys.folderImageDropMode, value)
  }

  watch(sidebarPinned, (value) => {
    localStorage.setItem(sidebarPinnedStorageKey, String(value))
  })

  watch(autoHideTitlebarInWindowMode, (value) => {
    localStorage.setItem(autoHideTitlebarInWindowModeStorageKey, String(value))
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

  watch(showGalleryScrollbar, (value) => {
    localStorage.setItem(showGalleryScrollbarStorageKey, String(value))
  })

  watch(galleryScrollbarSidebarMode, (value) => {
    localStorage.setItem(galleryScrollbarSidebarModeStorageKey, value)
  })

  return {
    sidebarPinned,
    autoHideTitlebarInWindowMode,
    rightSidebarPinned,
    autoFixRightSidebarOnPreview,
    themeMode,
    thumbnailCacheEnabled,
    showGalleryScrollbar,
    galleryScrollbarSidebarMode,
    folderImageDropMode,
    initAppSettingsFromStorage,
    setSidebarPinned,
    setAutoHideTitlebarInWindowMode,
    setRightSidebarPinned,
    setAutoFixRightSidebarOnPreview,
    setThemeMode,
    setThumbnailCacheEnabled,
    setShowGalleryScrollbar,
    setGalleryScrollbarSidebarMode,
    setFolderImageDropMode,
  }
}
