<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import GalleryView from './components/GalleryView.vue'
import AppOverlayLayer from './components/AppOverlayLayer.vue'
import LeftSidebar from './components/LeftSidebar.vue'
import RightSidebar from './components/RightSidebar.vue'
import ReferenceBoardView from './components/ReferenceBoardView.vue'
import SettingsView from './components/SettingsView.vue'
import { useAppSettings } from './composables/useAppSettings'
import { useBackgroundScan } from './composables/useBackgroundScan'
import { useThumbnailGeneration } from './composables/useThumbnailGeneration'
import { useAtmosphereGeneration } from './composables/useAtmosphereGeneration'
import { useColorSignatureGeneration } from './composables/useColorSignatureGeneration'
import { useFolderManagement } from './composables/useFolderManagement'
import { useGalleryMasonry } from './composables/useGalleryMasonry'
import { useGallerySearch } from './composables/useGallerySearch'
import { useImageDragAndDrop } from './composables/useImageDragAndDrop'
import { usePreviewBoardDrag } from './composables/usePreviewBoardDrag'
import { useContextMenuState } from './composables/useContextMenuState'
import { useReferenceBoardManagement } from './composables/useReferenceBoardManagement'
import { useReferenceBoardInteraction } from './composables/useReferenceBoardInteraction'
import { useReferenceBoardClipboard } from './composables/useReferenceBoardClipboard'
import { useReferenceBoardHistory } from './composables/useReferenceBoardHistory'
import { useReferenceBoardViewport } from './composables/useReferenceBoardViewport'
import type { GalleryImage, GalleryLayoutItem } from './types/gallery'
import type {
  BoardWorldBounds,
  LibraryStore,
  ReferenceBoard,
  ReferenceBoardItem,
  ViewMode,
} from './types/app-state'

const expandedReferenceBoardFolderIdsStorageKey = 'illutag.expandedReferenceBoardFolderIds'
const previewReferenceBoardIdsStorageKey = 'illutag.previewReferenceBoardIds'
const autoScanOnStartupStorageKey = 'illutag.autoScanOnStartup'
const imageDragDelayMs = 120
const sidebarHoverOpen = ref(false)
const rightSidebarHoverOpen = ref(false)
const viewMode = ref<ViewMode>('gallery')
const library = ref<LibraryStore>({
  folders: [],
  images: [],
  userFolders: [],
  imageFolders: [],
  referenceBoardFolders: [],
  referenceBoards: [],
  referenceBoardItems: [],
})
const isLoading = ref(false)
const isPickingFolder = ref(false)
const isAddingFolder = ref(false)
const statusText = ref('还没有添加图库文件夹')
const errorText = ref('')
const folderPathInput = ref('')
const activeReferenceBoardId = ref<number | null>(null)
const isWindowMaximized = ref(false)
const isTitlebarHovered = ref(false)
const boardSpaceFocusMode = ref<'item' | 'canvas'>('item')
const importLibraryFolderPickerItemId = ref<number | null>(null)
const importLibraryFolderPickerResolve = ref<((folderId: number | null) => void) | null>(null)
const boardPointerUseMaxAgeMs = 5000
const isSettingsView = computed(() => viewMode.value === 'settings')
const sidebarPinnedEffective = computed(() => isSettingsView.value || sidebarPinned.value)
const sidebarOpen = computed(() => sidebarPinnedEffective.value || sidebarHoverOpen.value)
const rightSidebarOpen = computed(() => rightSidebarPinned.value || rightSidebarHoverOpen.value)
const isTitlebarPinned = computed(() => !isWindowMaximized.value || isTitlebarHovered.value)

const activeReferenceBoard = computed(() =>
  library.value.referenceBoards.find((board) => board.id === activeReferenceBoardId.value) ?? null,
)

const {
  activeBoardCanvasBounds,
  boundsOfReferenceBoardItem,
  mergeBoardBounds,
  ensureBoardCanvasBoundsFor,
  syncBoardCanvasBounds,
} = useReferenceBoardViewport<LibraryStore>({
  library,
  activeReferenceBoardId,
  activeReferenceBoard,
})

const activeReferenceBoardItems = computed(() => {
  if (activeReferenceBoardId.value === null) return []
  const imagesById = new Map(library.value.images.map((image) => [image.id, image]))
  return library.value.referenceBoardItems
    .filter((item) => item.boardId === activeReferenceBoardId.value)
    .map((item) => ({ item, image: imagesById.get(item.imageId) }))
    .filter((entry): entry is { item: ReferenceBoardItem; image: GalleryImage } => Boolean(entry.image))
})

const referenceBoardCanvasMenuStyle = computed(() => {
  if (!referenceBoardCanvasMenu.value) return {}
  return {
    left: `${referenceBoardCanvasMenu.value.x}px`,
    top: `${referenceBoardCanvasMenu.value.y}px`,
  }
})

const searchPanelStyle = computed<Record<string, string>>(() => ({
  '--search-reveal': searchRevealProgress.value.toString(),
  '--search-opacity': searchRevealProgress.value.toString(),
  '--search-translate-y': `${(1 - searchRevealProgress.value) * -10}%`,
}))

const {
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
} = useAppSettings()

const {
  expandedReferenceBoardFolderIds,
  dragExpandedReferenceBoardFolderIds,
  previewReferenceBoardIds,
  boardContextMenu,
  boardDraft,
  newBoardName,
  isComposingBoardName,
  renamingReferenceBoardFolderId,
  renamingReferenceBoardFolderName,
  isComposingReferenceBoardFolderRename,
  renamingReferenceBoardId,
  renamingReferenceBoardName,
  isComposingReferenceBoardRename,
  draggedReferenceBoardId,
  draggedReferenceBoardFolderId,
  referenceBoardDragOverKind,
  referenceBoardDragOverId,
  isReferencePreviewActive,
  referenceBoardPreviewBlocks,
  referenceBoardRows,
  boardContextMenuStyle,
  boardDraftStyle,
  toggleReferenceBoardFolderExpanded,
  expandReferenceBoardFolder,
  onReferenceBoardFolderRowClick,
  showReferenceBoard,
  closeBoardContextMenu,
  referenceBoardIdFromPoint,
  referenceBoardFolderIdFromPoint,
  isPointInsideRightSidebarArea,
  clearDragReferenceBoardFolderCollapseTimer,
  clearDragExpandedReferenceBoardFoldersNow,
  scheduleClearDragExpandedReferenceBoardFolders,
  keepDragExpandedReferenceBoardFolder,
  clearReferenceBoardDragState,
  startReferenceBoardFolderDrag,
  startReferenceBoardDrag,
  onReferenceBoardDragOverFolder,
  onReferenceBoardDragOverBoard,
  onReferenceBoardDragOverSpace,
  dropOnReferenceBoardFolder,
  dropOnReferenceBoard,
  dropOnReferenceBoardSpace,
  endReferenceBoardDrag,
  openBoardSpaceMenu,
  openReferenceBoardFolderMenu,
  openReferenceBoardMenu,
  toggleReferenceBoardPreview,
  removeReferenceBoardPreview,
  openBoardDraft,
  closeBoardDraft,
  setNewBoardName,
  setComposingBoardName,
  commitBoardDraft,
  cancelReferenceBoardFolderRename,
  startComposingReferenceBoardFolderRename,
  endComposingReferenceBoardFolderRename,
  setRenamingReferenceBoardFolderName,
  startReferenceBoardFolderRename,
  commitReferenceBoardFolderRename,
  onReferenceBoardFolderRenameEnter,
  deleteReferenceBoardFolder,
  cancelReferenceBoardRename,
  startComposingReferenceBoardRename,
  endComposingReferenceBoardRename,
  setRenamingReferenceBoardName,
  startReferenceBoardRename,
  commitReferenceBoardRename,
  onReferenceBoardRenameEnter,
  deleteReferenceBoard,
} = useReferenceBoardManagement<LibraryStore>({
  library,
  viewMode,
  activeReferenceBoardId,
  rightSidebarPinned,
  autoFixRightSidebarOnPreview,
  ensureBoardCanvasBoundsFor,
  convertFileSrc,
  setErrorText(value) {
    errorText.value = value
  },
  formatError,
})

const {
  activeUserFolderId,
  newFolderName,
  folderDraft,
  isComposingFolderName,
  dragExpandedFolderIds,
  folderContextMenu,
  renamingUserFolderId,
  renamingUserFolderName,
  isComposingUserFolderRename,
  folderPointerState,
  draggedFolderId,
  folderDragOverId,
  folderTree,
  dropFolderTree,
  contextMenuStyle,
  folderDraftStyle,
  folderScopedImages,
  deleteUserFolder,
  openCreateFolderDraft,
  closeCreateFolderDraft,
  commitFolderDraft,
  toggleFolderExpanded,
  expandFolder,
  openFolderSectionMenu,
  openFolderMenu,
  closeFolderContextMenu,
  showAllImages,
  showTrashImages,
  onUserFolderRowClick,
  startUserFolderRename,
  setRenamingUserFolderName,
  startComposingUserFolderRename,
  endComposingUserFolderRename,
  cancelUserFolderRename,
  commitUserFolderRename,
  onUserFolderRenameEnter,
  clearFolderPress,
  startFolderPointer,
  moveFolderPointer,
  finishFolderPointer,
  folderIdFromPoint,
  folderHasChildren,
  expandedDropFolderIdsFor,
  assignImageToFolder,
} = useFolderManagement<LibraryStore>({
  library,
  viewMode,
  activeReferenceBoardId,
  setErrorText(value) {
    errorText.value = value
  },
  formatError,
  updateStatus,
  closeBoardContextMenu,
  clamp,
})

const {
  dragState,
  lastImageDragEndedAt,
  clearImagePress,
  cancelImageDrag,
  startImagePress,
  moveImageDrag,
  finishImageDrag,
} = useImageDragAndDrop<LibraryStore>({
  library,
  imageDragDelayMs,
  dragExpandedFolderIds,
  folderIdFromPoint,
  folderHasChildren,
  expandedDropFolderIdsFor,
  assignImageToFolder,
  referenceBoardIdFromPoint,
  referenceBoardFolderIdFromPoint,
  isPointInsideRightSidebarArea,
  keepDragExpandedReferenceBoardFolder,
  clearDragExpandedReferenceBoardFoldersNow,
  clearDragReferenceBoardFolderCollapseTimer,
  scheduleClearDragExpandedReferenceBoardFolders,
  dragExpandedReferenceBoardFolderIds,
  addImageToReferenceBoard,
  expandReferenceBoardFolder,
  isPointInsideExternalImageSearchDropZone,
  setExternalImageSearchFromGalleryImage: setExternalImageSearchFromGalleryDrag,
  setErrorText(value) {
    errorText.value = value
  },
  formatError,
})

const {
  imageDetailContextMenu,
  galleryImageContextMenu,
  imageDetailContextMenuStyle,
  galleryImageContextMenuStyle,
  closeImageDetailContextMenu,
  closeGalleryImageContextMenu,
  openGalleryImageMenu: openGalleryImageMenuState,
  openImageDetailMenu: openImageDetailMenuState,
} = useContextMenuState()

const {
  boardScale,
  boardPan,
  selectedReferenceBoardItemId,
  referenceBoardCanvasMenu,
  lastBoardPointerWorld,
  closeReferenceBoardCanvasMenu,
  openReferenceBoardItemMenu,
  openReferenceBoardCanvasMenu,
  getReferenceBoardViewportMetrics,
  trackBoardPointer,
  zoomReferenceBoard,
  startBoardPan,
  moveBoardInteraction,
  finishBoardInteraction,
  startBoardItemMove,
  startBoardItemResize,
  startBoardItemRotate,
  clearBoardInteraction,
} = useReferenceBoardInteraction<LibraryStore>({
  library,
  activeReferenceBoard,
  viewMode,
  ensureBoardCanvasBoundsFor,
  closeImageDetailContextMenu,
  closeGalleryImageContextMenu,
  setErrorText(value) {
    errorText.value = value
  },
  formatError,
  clamp,
  onLayoutHistory(payload) {
    pushBoardHistory({
      kind: 'layout',
      boardId: payload.boardId,
      changes: [{ itemId: payload.itemId, before: payload.before, after: payload.after }],
      selectionBefore: payload.selectionBefore,
      selectionAfter: payload.selectionAfter,
    })
  },
})

const {
  searchZhInput,
  searchZhSelected,
  searchZhSuggestions,
  searchZhOpen,
  searchEnQuery,
  searchFileNameQuery,
  searchNaturalLanguageQuery,
  searchMode,
  externalImageSearchType,
  externalImageQueryUrl,
  externalImageQueryPreviewUrl,
  externalImageQueryLabel,
  searchConfidenceMin,
  searchConfidenceMax,
  searchRunning,
  searchError,
  isSearchFocused,
  isSearchPointerInside,
  searchRevealMode,
  searchRevealProgress,
  visibleImages,
  activeImageDetailId,
  activeImageDetail,
  groupedImageTags,
  setSearchPointerInside,
  setSearchFocus,
  setSearchViewportState,
  triggerSearchRevealByHotspot,
  hideSearchPanel: hideSearchPanelByState,
  setSearchZhInput,
  openSearchZhSuggestionPanel,
  closeSearchZhSuggestionPanelDeferred,
  selectSearchZhSuggestion,
  removeSearchZhSuggestion,
  setSearchEnQuery,
  setSearchFileNameQuery,
  setSearchNaturalLanguageQuery,
  setSearchMode,
  setExternalImageSearchType,
  setSearchConfidenceMin,
  setSearchConfidenceMax,
  executeGallerySearch,
  clearExternalImageSearch,
  clearAllSearchInputs,
  setExternalImageQueryUrl,
  pasteExternalImageSearchFromPasteEvent,
  setExternalImageSearchFromFile,
  setExternalImageSearchFromGalleryImage,
  selectExternalImageSearchFile,
  pasteExternalImageSearchFromClipboard,
  searchBySingleTag,
  closeImageDetail,
  openGalleryImageDetail,
} = useGallerySearch<LibraryStore>({
  library,
  folderScopedImages,
  activeUserFolderId,
  lastImageDragEndedAt,
  formatError,
  clamp,
  toFileSrc: convertFileSrc,
  pickExternalImagePath: pickExternalImageSearchFilePath,
  onBeforeRunSearch() {
    scrollGalleryToTop(galleryScrollScopeKeyOf(activeUserFolderId.value))
  },
  onOpenImageDetail() {
    imageDetailContextMenu.value = null
  },
  onCloseImageDetail() {
    imageDetailContextMenu.value = null
  },
})

const {
  renderedLayoutItems,
  masonryContentWidth,
  totalHeight,
  setGalleryElement,
  onGalleryScroll,
  onGalleryWheel,
  updateViewportSize,
  saveGalleryScrollPosition,
  restoreGalleryScrollPosition,
  scrollGalleryToTop,
} = useGalleryMasonry({
  visibleImages,
  convertFileSrc,
  clamp,
})

const {
  autoScanOnStartup,
  isBackgroundScanRunning,
  isBackgroundScanPaused,
  scanProgressText,
  scanRecentErrors,
  isNaturalLanguageScanRunning,
  isNaturalLanguageScanPaused,
  naturalLanguageScanProgressText,
  naturalLanguageScanRecentErrors,
  initAutoScanOnStartupFromStorage,
  setAutoScanOnStartup,
  startScanAllFolders,
  pauseScanAllFolders,
  resumeScanAllFolders,
  stopScanAllFolders,
  startNaturalLanguageScan,
  pauseNaturalLanguageScan,
  resumeNaturalLanguageScan,
  stopNaturalLanguageScan,
  startStartupCleanup,
  refreshBackgroundScanStatus,
  startBackgroundScanPolling,
  stopBackgroundScanPolling,
  startAutoScanIfEnabled,
} = useBackgroundScan({
  loadLibrary,
  formatError,
  setErrorText(value) {
    errorText.value = value
  },
  autoScanOnStartupStorageKey,
})

const {
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
  startThumbnailGenerationPolling,
  stopThumbnailGenerationPolling,
} = useThumbnailGeneration({
  loadLibrary,
  formatError,
  setErrorText(value) {
    errorText.value = value
  },
})

const {
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
  startAtmosphereGenerationPolling,
  stopAtmosphereGenerationPolling,
} = useAtmosphereGeneration({
  loadLibrary,
  formatError,
  setErrorText(value) {
    errorText.value = value
  },
})

const {
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
  startColorSignatureGenerationPolling,
  stopColorSignatureGenerationPolling,
} = useColorSignatureGeneration({
  loadLibrary,
  formatError,
  setErrorText(value) {
    errorText.value = value
  },
})

const {
  copyReferenceBoardItemToClipboard,
  pasteReferenceBoardContent,
  copyImageToSystemClipboard,
  buildClipboardCopyErrorText,
  clearInternalBoardCopyRefForItem,
  clearInternalBoardCopyRefForItems,
} = useReferenceBoardClipboard<LibraryStore>({
  library,
  activeReferenceBoard,
  selectedReferenceBoardItemId,
  boardPan,
  boardScale,
  lastBoardPointerWorld,
  boardPointerUseMaxAgeMs,
  closeReferenceBoardCanvasMenu,
  ensureBoardCanvasBoundsFor,
  getReferenceBoardViewportMetrics,
  setErrorText(value) {
    errorText.value = value
  },
  formatError,
})

const {
  pruneBoardHistory,
  pushBoardHistory,
  collectBoardLayoutMap,
  buildBoardHistoryChanges,
  undoReferenceBoardHistory,
  redoReferenceBoardHistory,
  removeReferenceBoardItem,
  removeReferenceBoardItemsWithHistory,
} = useReferenceBoardHistory<LibraryStore>({
  library,
  activeReferenceBoardId,
  selectedReferenceBoardItemId,
  ensureBoardCanvasBoundsFor,
  clearInternalBoardCopyRefForItems,
  closeReferenceBoardCanvasMenu,
  setErrorText(value) {
    errorText.value = value
  },
  formatError,
})

const {
  previewDragOverDeleteZone,
  previewBoardItemDrag,
  previewBoardDragIconKind,
  onPreviewReferenceThumbClick,
  startPreviewBoardItemDrag,
  startPreviewBoardItemPointerDrag,
  movePreviewBoardItemPointerDrag,
  finishPreviewBoardItemPointerDrag,
  onPreviewBoardItemDragOverPreview,
  onPreviewBoardItemDragOverBoard,
  dropPreviewBoardItem,
  endPreviewBoardItemDrag,
  onGalleryPreviewBoardItemDragOver,
  onGalleryPreviewBoardItemDrop,
} = usePreviewBoardDrag<LibraryStore>({
  library,
  selectedReferenceBoardItemId,
  closeBoardContextMenu,
  clearReferenceBoardDragState,
  ensureBoardCanvasBoundsFor,
  removeReferenceBoardItemsWithHistory,
  clearInternalBoardCopyRefForItem,
  showReferenceBoard,
  setErrorText(value) {
    errorText.value = value
  },
  formatError,
})

onMounted(async () => {
  initAppSettingsFromStorage()
  expandedReferenceBoardFolderIds.value = readStoredIdSet(expandedReferenceBoardFolderIdsStorageKey)
  previewReferenceBoardIds.value = readStoredIdSet(previewReferenceBoardIdsStorageKey)
  initAutoScanOnStartupFromStorage()
  await loadLibrary()
  handleWindowResize()
  window.addEventListener('resize', handleWindowResize)
  window.addEventListener('pointermove', moveImageDrag)
  window.addEventListener('pointermove', movePreviewBoardItemPointerDrag)
  window.addEventListener('pointermove', moveFolderPointer)
  window.addEventListener('pointermove', moveBoardInteraction)
  window.addEventListener('pointermove', trackBoardPointer)
  window.addEventListener('pointerup', finishImageDrag)
  window.addEventListener('pointerup', finishPreviewBoardItemPointerDrag)
  window.addEventListener('pointerup', finishFolderPointer)
  window.addEventListener('pointerup', finishBoardInteraction)
  window.addEventListener('click', closeFolderContextMenu)
  window.addEventListener('click', closeBoardContextMenu)
  window.addEventListener('click', closeReferenceBoardCanvasMenu)
  window.addEventListener('click', closeImageDetailContextMenu)
  window.addEventListener('click', closeGalleryImageContextMenu)
  window.addEventListener('keydown', handleGlobalKeydown)
  void refreshBackgroundScanStatus()
  startBackgroundScanPolling()
  startThumbnailGenerationPolling()
  startAtmosphereGenerationPolling()
  startColorSignatureGenerationPolling()
  void startStartupCleanup()
  startAutoScanIfEnabled()
  if (thumbnailCacheEnabled.value && !isBackgroundScanRunning.value) {
    void startThumbnailGeneration()
  }
})

onUnmounted(() => {
  stopBackgroundScanPolling()
  stopThumbnailGenerationPolling()
  stopAtmosphereGenerationPolling()
  stopColorSignatureGenerationPolling()
  clearDragReferenceBoardFolderCollapseTimer()
  window.removeEventListener('resize', handleWindowResize)
  window.removeEventListener('pointermove', moveImageDrag)
  window.removeEventListener('pointermove', movePreviewBoardItemPointerDrag)
  window.removeEventListener('pointermove', moveFolderPointer)
  window.removeEventListener('pointermove', moveBoardInteraction)
  window.removeEventListener('pointermove', trackBoardPointer)
  window.removeEventListener('pointerup', finishImageDrag)
  window.removeEventListener('pointerup', finishPreviewBoardItemPointerDrag)
  window.removeEventListener('pointerup', finishFolderPointer)
  window.removeEventListener('pointerup', finishBoardInteraction)
  window.removeEventListener('click', closeFolderContextMenu)
  window.removeEventListener('click', closeBoardContextMenu)
  window.removeEventListener('click', closeReferenceBoardCanvasMenu)
  window.removeEventListener('click', closeImageDetailContextMenu)
  window.removeEventListener('click', closeGalleryImageContextMenu)
  window.removeEventListener('keydown', handleGlobalKeydown)
  closeImportLibraryFolderPicker(null)
})

watch(sidebarPinned, async (value) => {
  if (value) sidebarHoverOpen.value = false
  await nextTick()
  updateViewportSize()
})

watch(rightSidebarPinned, async (value) => {
  if (value) rightSidebarHoverOpen.value = false
  await nextTick()
  updateViewportSize()
})

watch(expandedReferenceBoardFolderIds, (value) => {
  localStorage.setItem(
    expandedReferenceBoardFolderIdsStorageKey,
    JSON.stringify([...value].filter((id) => Number.isFinite(id))),
  )
})

watch(previewReferenceBoardIds, (value) => {
  localStorage.setItem(
    previewReferenceBoardIdsStorageKey,
    JSON.stringify([...value].filter((id) => Number.isFinite(id))),
  )
})

watch(activeReferenceBoardId, () => {
  boardSpaceFocusMode.value = 'item'
  if (activeReferenceBoardId.value !== null) {
    ensureBoardCanvasBoundsFor(activeReferenceBoardId.value)
  }
})

watch(selectedReferenceBoardItemId, () => {
  boardSpaceFocusMode.value = 'item'
})

watch(
  () => library.value.referenceBoards.map((board) => board.id).join(','),
  () => {
    const exists = new Set(library.value.referenceBoards.map((board) => board.id))
    pruneBoardHistory(exists)
    const next = new Set([...previewReferenceBoardIds.value].filter((id) => exists.has(id)))
    if (next.size !== previewReferenceBoardIds.value.size) {
      previewReferenceBoardIds.value = next
    }
    syncBoardCanvasBounds(exists)
  },
)

watch([visibleImages, sidebarPinned], async () => {
  await nextTick()
  updateViewportSize()
})

watch(
  [viewMode, activeUserFolderId],
  async ([nextViewMode, nextFolderId], [prevViewMode, prevFolderId]) => {
    const prevScopeKey = galleryScrollScopeKeyOf(prevFolderId)
    const nextScopeKey = galleryScrollScopeKeyOf(nextFolderId)

    if (prevViewMode === 'gallery') {
      saveGalleryScrollPosition(prevScopeKey)
    }

    if (nextViewMode === 'gallery') {
      restoreGalleryScrollPosition(nextScopeKey)
      await nextTick()
      updateViewportSize()
    }
  },
)

watch(thumbnailCacheEnabled, (enabled) => {
  if (!enabled) return
  if (isBackgroundScanRunning.value) return
  void startThumbnailGeneration()
})

watch(isBackgroundScanRunning, (running, wasRunning) => {
  if (!wasRunning || running) return
  if (!thumbnailCacheEnabled.value) return
  void startThumbnailGeneration()
})

async function loadLibrary(options?: { silent?: boolean }) {
  const silent = Boolean(options?.silent)
  if (!silent) {
    isLoading.value = true
    errorText.value = ''
  }

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('list_library')
    for (const board of library.value.referenceBoards) {
      ensureBoardCanvasBoundsFor(board.id)
    }
    updateStatus()
  } catch (error) {
    if (!silent) {
      errorText.value = formatError(error)
    }
  } finally {
    if (!silent) {
      isLoading.value = false
    }
    await nextTick()
    updateViewportSize()
  }
}

async function pickFolder() {
  if (isPickingFolder.value) return
  isPickingFolder.value = true
  errorText.value = ''
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择图库文件夹',
    })

    if (typeof selected === 'string') {
      folderPathInput.value = selected
    }
  } finally {
    isPickingFolder.value = false
  }
}

async function pickExternalImageSearchFilePath() {
  const selected = await open({
    directory: false,
    multiple: false,
    title: '选择用于以图搜图的图片',
    filters: [
      {
        name: 'Images',
        extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'ico'],
      },
    ],
  })
  return typeof selected === 'string' ? selected : null
}

async function addFolder() {
  if (isAddingFolder.value) return
  errorText.value = ''

  if (folderPathInput.value.trim().length === 0) {
    errorText.value = '请输入图库文件夹路径'
    return
  }

  isAddingFolder.value = true
  statusText.value = '正在扫描图库...'

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('add_gallery_folder_command', {
      folderPath: folderPathInput.value.trim(),
    })
    viewMode.value = 'gallery'
    activeUserFolderId.value = 'all'
    folderPathInput.value = ''
    updateStatus()
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    isAddingFolder.value = false
    await nextTick()
    updateViewportSize()
  }
}

async function removeFolder(folderPath: string) {
  errorText.value = ''
  isLoading.value = true
  statusText.value = '正在移除文件夹索引...'

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('remove_gallery_folder_command', { folderPath })
    updateStatus()
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    isLoading.value = false
  }
}

function isEditableKeyboardTarget(event: KeyboardEvent) {
  const target = event.target as HTMLElement | null
  if (!target) return false
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return true
  return target.isContentEditable
}

function handleGlobalKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    closeFolderContextMenu()
    closeCreateFolderDraft()
    cancelUserFolderRename()
    closeBoardContextMenu()
    closeBoardDraft()
    cancelReferenceBoardFolderRename()
    cancelReferenceBoardRename()
    closeReferenceBoardCanvasMenu()
    closeImportLibraryFolderPicker(null)
    closeImageDetail()
    closeGalleryImageContextMenu()
    cancelImageDrag()
    clearFolderPress()

    folderPointerState.value = null
    draggedFolderId.value = null
    folderDragOverId.value = null
    clearBoardInteraction()
    return
  }

  if (event.isComposing || isEditableKeyboardTarget(event)) return

  if (viewMode.value === 'gallery' && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'v') {
    event.preventDefault()
    void pasteExternalImageSearchFromClipboard().then((ok) => {
      if (ok) void executeGallerySearch()
    })
    return
  }

  if (viewMode.value !== 'board' || !activeReferenceBoard.value) return

  if ((event.ctrlKey || event.metaKey) && !event.altKey) {
    const key = event.key.toLowerCase()
    if (key === 'z') {
      event.preventDefault()
      if (event.shiftKey) {
        void redoReferenceBoardHistory()
      } else {
        void undoReferenceBoardHistory()
      }
      return
    }
    if (key === 'y') {
      event.preventDefault()
      void redoReferenceBoardHistory()
      return
    }
  }

  if (event.code === 'Space' || event.key === ' ') {
    event.preventDefault()
    focusReferenceBoardBySpaceShortcut()
    return
  }

  if (event.key === 'Delete' && selectedReferenceBoardItemId.value !== null) {
    event.preventDefault()
    void removeReferenceBoardItem(selectedReferenceBoardItemId.value)
    return
  }

  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'c') {
    if (selectedReferenceBoardItemId.value !== null) {
      event.preventDefault()
      void copyReferenceBoardItemToClipboard(selectedReferenceBoardItemId.value)
    }
    return
  }

  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'v') {
    event.preventDefault()
    void pasteReferenceBoardContent()
  }
}

async function addImageToReferenceBoard(imageId: string, boardId: number) {
  const { invoke } = await import('@tauri-apps/api/core')
  library.value = await invoke<LibraryStore>('add_image_to_reference_board_command', {
    imageId,
    boardId,
  })
  ensureBoardCanvasBoundsFor(boardId)
}

function focusReferenceBoardBounds(bounds: BoardWorldBounds) {
  const viewport = getReferenceBoardViewportMetrics()
  if (!viewport) return false

  const width = Math.max(1, bounds.maxX - bounds.minX)
  const height = Math.max(1, bounds.maxY - bounds.minY)
  const nextScale = clamp(Math.min(viewport.width / width, viewport.height / height), 0.2, 4)
  const centerX = (bounds.minX + bounds.maxX) / 2
  const centerY = (bounds.minY + bounds.maxY) / 2
  boardScale.value = nextScale
  boardPan.value = {
    x: viewport.width / 2 - centerX * nextScale,
    y: viewport.height / 2 - centerY * nextScale,
  }
  return true
}

function focusActiveReferenceBoardCanvas() {
  if (!activeReferenceBoard.value) return false
  const items = library.value.referenceBoardItems.filter((item) => item.boardId === activeReferenceBoard.value?.id)
  if (items.length === 0) return false
  const bounds = mergeBoardBounds(items.map(boundsOfReferenceBoardItem))
  return focusReferenceBoardBounds(bounds)
}

function focusSelectedReferenceBoardItem() {
  const selectedId = selectedReferenceBoardItemId.value
  if (selectedId === null) return false
  const item = library.value.referenceBoardItems.find((entry) => entry.id === selectedId)
  if (!item) return false
  return focusReferenceBoardBounds(boundsOfReferenceBoardItem(item))
}

function focusReferenceBoardBySpaceShortcut() {
  if (boardSpaceFocusMode.value === 'item' && selectedReferenceBoardItemId.value !== null) {
    if (focusSelectedReferenceBoardItem()) {
      boardSpaceFocusMode.value = 'canvas'
      return
    }
  }
  if (focusActiveReferenceBoardCanvas()) {
    boardSpaceFocusMode.value = 'item'
  }
}

async function autoArrangeActiveReferenceBoard() {
  if (!activeReferenceBoard.value) return
  const boardId = activeReferenceBoard.value.id
  const beforeMap = collectBoardLayoutMap(boardId)
  const selectionBefore = selectedReferenceBoardItemId.value
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('auto_arrange_reference_board_command', {
      boardId,
    })
    ensureBoardCanvasBoundsFor(boardId)
    const afterMap = collectBoardLayoutMap(boardId)
    pushBoardHistory({
      kind: 'layout',
      boardId,
      changes: buildBoardHistoryChanges(beforeMap, afterMap),
      selectionBefore,
      selectionAfter: selectedReferenceBoardItemId.value,
    })
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    closeReferenceBoardCanvasMenu()
  }
}

async function importSelectedReferenceItemToLibrary(itemId: number) {
  if (!canImportReferenceBoardItemToLibrary(itemId)) {
    closeReferenceBoardCanvasMenu()
    return
  }

  closeReferenceBoardCanvasMenu()
  const folderId = await pickImportedLibraryFolderIdForImport(itemId)
  if (folderId === null) {
    return
  }

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('import_reference_board_item_to_library_command', {
      itemId,
      folderId,
    })
  } catch (error) {
    errorText.value = formatError(error)
  }
}

function canImportReferenceBoardItemToLibrary(itemId: number) {
  const boardItem = library.value.referenceBoardItems.find((item) => item.id === itemId)
  if (!boardItem) return false
  const image = library.value.images.find((item) => item.id === boardItem.imageId)
  if (!image) return false
  return image.source === 'reference'
}

function closeImportLibraryFolderPicker(selectedFolderId: number | null) {
  importLibraryFolderPickerItemId.value = null
  const resolve = importLibraryFolderPickerResolve.value
  importLibraryFolderPickerResolve.value = null
  resolve?.(selectedFolderId)
}

function openImportLibraryFolderPicker(itemId: number) {
  if (importLibraryFolderPickerResolve.value) {
    importLibraryFolderPickerResolve.value(null)
  }
  importLibraryFolderPickerItemId.value = itemId
  return new Promise<number | null>((resolve) => {
    importLibraryFolderPickerResolve.value = resolve
  })
}

async function pickImportedLibraryFolderIdForImport(itemId: number) {
  const folders = library.value.folders
  if (folders.length === 0) {
    errorText.value = '请先在设置中导入至少一个本地图库文件夹。'
    return null
  }
  if (folders.length === 1) {
    return folders[0].id
  }
  return openImportLibraryFolderPicker(itemId)
}

async function exportReferenceBoardItem(itemId: number) {
  const destination = await save({
    title: '导出参考板图片',
    defaultPath: `reference-item-${itemId}.png`,
  })
  if (!destination || Array.isArray(destination)) return

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('export_reference_board_item_command', {
      itemId,
      destination,
    })
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    closeReferenceBoardCanvasMenu()
  }
}

function updateStatus() {
  const imageCount = library.value.images.length
  const folderCount = library.value.folders.length
  statusText.value =
    imageCount > 0 ? `${imageCount} 张图片，来自 ${folderCount} 个图库文件夹` : '还没有添加图库文件夹'
}

function handleWindowResize() {
  updateViewportSize()
  void refreshWindowMaximized()
}

async function refreshWindowMaximized() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    isWindowMaximized.value = await invoke<boolean>('window_is_maximized_command')
  } catch {
    isWindowMaximized.value = false
  }
}

function showTitlebarByHotspot() {
  if (isWindowMaximized.value) isTitlebarHovered.value = true
}

function onTitlebarMouseEnter() {
  if (isWindowMaximized.value) isTitlebarHovered.value = true
}

function onTitlebarMouseLeave() {
  if (isWindowMaximized.value) isTitlebarHovered.value = false
}

async function minimizeWindow() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('window_minimize_command')
  } catch {}
}

async function toggleWindowMaximize() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const maximized = await invoke<boolean>('window_toggle_maximize_command')
    isWindowMaximized.value = maximized
    if (!maximized) {
      isTitlebarHovered.value = true
    } else if (!isTitlebarHovered.value) {
      isTitlebarHovered.value = false
    }
  } catch {}
}

async function startWindowDrag(event: PointerEvent) {
  if (event.button !== 0) return
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('window_start_dragging_command')
  } catch {}
}

async function closeWindow() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('window_close_command')
    return
  } catch {}

  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().close()
  } catch (error) {
    errorText.value = formatError(error)
  }
}

function openSidebarByHover() {
  if (isSettingsView.value) return
  if (!sidebarPinned.value) sidebarHoverOpen.value = true
}

function openRightSidebarByHover() {
  if (!rightSidebarPinned.value) rightSidebarHoverOpen.value = true
}

function closeSidebarByHover() {
  if (
    !sidebarPinnedEffective.value &&
    !folderDraft.value &&
    !isComposingFolderName.value &&
    renamingUserFolderId.value === null &&
    !isComposingUserFolderRename.value &&
    draggedFolderId.value === null
  ) {
    sidebarHoverOpen.value = false
    closeFolderContextMenu()
  }
}

function closeRightSidebarByHover() {
  if (previewBoardItemDrag.value) return
  if (
    !rightSidebarPinned.value &&
    !boardDraft.value &&
    !isComposingBoardName.value &&
    renamingReferenceBoardFolderId.value === null &&
    renamingReferenceBoardId.value === null &&
    !isComposingReferenceBoardFolderRename.value &&
    !isComposingReferenceBoardRename.value &&
    draggedReferenceBoardId.value === null &&
    draggedReferenceBoardFolderId.value === null
  ) {
    rightSidebarHoverOpen.value = false
    closeBoardContextMenu()
  }
}

function openSettings() {
  viewMode.value = 'settings'
  sidebarHoverOpen.value = false
}

function hideSearchPanel() {
  hideSearchPanelByState()
}

function setNewFolderName(value: string) {
  newFolderName.value = value
}

function setComposingFolderName(value: boolean) {
  isComposingFolderName.value = value
}

function openGalleryImageMenu(item: GalleryLayoutItem, event: MouseEvent) {
  openGalleryImageMenuState(item, event, closeReferenceBoardCanvasMenu)
}

function openImageDetailMenu(event: MouseEvent) {
  openImageDetailMenuState(event, Boolean(activeImageDetail.value), closeReferenceBoardCanvasMenu)
}

async function exportGalleryImage(imageId: string) {
  const image = library.value.images.find((entry) => entry.id === imageId)
  const destination = await save({
    title: '导出图片',
    defaultPath: image?.fileName ?? `${imageId}.png`,
  })
  if (!destination || Array.isArray(destination)) return

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('export_gallery_image_command', {
      imageId,
      destination,
    })
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    imageDetailContextMenu.value = null
    closeGalleryImageContextMenu()
  }
}

async function removeGalleryImageFromIndex(imageId: string) {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('remove_image_from_index_command', {
      imageId,
    })
    if (activeImageDetailId.value === imageId) {
      closeImageDetail()
    }
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    imageDetailContextMenu.value = null
    closeGalleryImageContextMenu()
  }
}

async function restoreGalleryImageFromTrash(imageId: string) {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('restore_image_from_trash_command', {
      imageId,
    })
    if (activeImageDetailId.value === imageId) {
      closeImageDetail()
    }
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    imageDetailContextMenu.value = null
    closeGalleryImageContextMenu()
  }
}

async function removeGalleryImageFromFolder(imageId: string) {
  if (typeof activeUserFolderId.value !== 'number') return
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('remove_image_from_user_folder_command', {
      imageId,
      folderId: activeUserFolderId.value,
    })
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    imageDetailContextMenu.value = null
    closeGalleryImageContextMenu()
  }
}

async function copyGalleryImageToClipboard(imageId: string) {
  try {
    await copyImageToSystemClipboard(imageId)
  } catch (error) {
    errorText.value = buildClipboardCopyErrorText(error, '图库复制')
  } finally {
    imageDetailContextMenu.value = null
    closeGalleryImageContextMenu()
  }
}

function searchByTagFromImageDetail(tagEn: string, tagZh?: string | null) {
  searchBySingleTag(tagEn, tagZh ?? null)
  closeImageDetail()
}

function formatFileSize(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  let value = bytes
  let unitIndex = 0
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024
    unitIndex += 1
  }
  return `${value.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`
}

function formatTime(timestamp: number) {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return '--'
  return new Date(timestamp).toLocaleString('zh-CN')
}

function closeSidebarByToggle() {
  sidebarHoverOpen.value = false
}

const settingsViewHandlers = {
  setSidebarPinned,
  setThemeMode,
  setAutoFixRightSidebarOnPreview,
  setThumbnailCacheEnabled,
  startThumbnailGeneration,
  pauseThumbnailGeneration,
  resumeThumbnailGeneration,
  stopThumbnailGeneration,
  clearThumbnailCache,
  rebuildThumbnailCache,
  startAtmosphereGeneration,
  pauseAtmosphereGeneration,
  resumeAtmosphereGeneration,
  stopAtmosphereGeneration,
  rebuildAtmosphereSignatureCache,
  startColorSignatureGeneration,
  pauseColorSignatureGeneration,
  resumeColorSignatureGeneration,
  stopColorSignatureGeneration,
  rebuildColorSignatureCache,
  setAutoScanOnStartup,
  startScanAllFolders,
  pauseScanAllFolders,
  resumeScanAllFolders,
  stopScanAllFolders,
  startNaturalLanguageScan,
  pauseNaturalLanguageScan,
  resumeNaturalLanguageScan,
  stopNaturalLanguageScan,
  addFolder,
  pickFolder,
  setFolderPathInput(value: string) {
    folderPathInput.value = value
  },
  removeFolder,
}

const leftSidebarHandlers = {
  closeHover: closeSidebarByHover,
  closeByToggle: closeSidebarByToggle,
  showAllImages,
  showTrashImages,
  openFolderSectionMenu,
  openFolderMenu,
  startFolderPointer,
  onUserFolderRowClick,
  startUserFolderRename,
  toggleFolderExpanded,
  setRenamingUserFolderName,
  onUserFolderRenameEnter,
  cancelUserFolderRename,
  commitUserFolderRename,
  startComposingUserFolderRename,
  endComposingUserFolderRename,
  openSettings,
  openCreateFolderDraft,
  deleteUserFolder,
  commitFolderDraft,
  setNewFolderName,
  closeCreateFolderDraft,
  setComposingFolderName,
}

const rightSidebarHandlers = {
  closeHover: closeRightSidebarByHover,
  setRightSidebarPinned,
  toggleReferenceBoardPreview,
  removeReferenceBoardPreview,
  startPreviewBoardItemPointerDrag,
  startPreviewBoardItemDrag,
  onPreviewBoardItemDragOverPreview,
  onPreviewBoardItemDragOverBoard,
  dropPreviewBoardItem,
  endPreviewBoardItemDrag,
  startReferenceBoardFolderDrag,
  startReferenceBoardDrag,
  onReferenceBoardDragOverFolder,
  onReferenceBoardDragOverBoard,
  onReferenceBoardDragOverSpace,
  dropOnReferenceBoardFolder,
  dropOnReferenceBoard,
  dropOnReferenceBoardSpace,
  endReferenceBoardDrag,
  openBoardSpaceMenu,
  openReferenceBoardFolderMenu,
  onReferenceBoardFolderRowClick,
  toggleReferenceBoardFolderExpanded,
  showReferenceBoard,
  onPreviewReferenceThumbClick,
  openReferenceBoardMenu,
  openBoardDraft,
  startReferenceBoardFolderRename,
  setRenamingReferenceBoardFolderName,
  onReferenceBoardFolderRenameEnter,
  cancelReferenceBoardFolderRename,
  commitReferenceBoardFolderRename,
  startComposingReferenceBoardFolderRename,
  endComposingReferenceBoardFolderRename,
  renameReferenceBoardFolder: startReferenceBoardFolderRename,
  deleteReferenceBoardFolder,
  startReferenceBoardRename,
  setRenamingReferenceBoardName,
  onReferenceBoardRenameEnter,
  cancelReferenceBoardRename,
  commitReferenceBoardRename,
  startComposingReferenceBoardRename,
  endComposingReferenceBoardRename,
  renameReferenceBoard: startReferenceBoardRename,
  deleteReferenceBoard,
  commitBoardDraft,
  closeBoardDraft,
  setNewBoardName,
  setComposingBoardName,
}

const referenceBoardViewHandlers = {
  zoomReferenceBoard,
  startBoardPan,
  startBoardItemMove,
  startBoardItemResize,
  startBoardItemRotate,
  removeReferenceBoardItem,
  openReferenceBoardItemMenu,
  openReferenceBoardCanvasMenu,
  convertFileSrc,
}

const galleryViewHandlers = {
  setGalleryElement,
  onGalleryScroll,
  setSearchViewportState,
  triggerSearchRevealByHotspot,
  onGalleryWheel,
  onGalleryPreviewBoardItemDragOver,
  onGalleryPreviewBoardItemDrop,
  endPreviewBoardItemDrag,
  setSearchPointerInside,
  setSearchFocus,
  hideSearchPanel,
  setSearchZhInput,
  openSearchZhSuggestionPanel,
  closeSearchZhSuggestionPanelDeferred,
  selectSearchZhSuggestion,
  removeSearchZhSuggestion,
  setSearchEnQuery,
  setSearchFileNameQuery,
  setSearchNaturalLanguageQuery,
  setSearchMode,
  setExternalImageSearchType,
  setSearchConfidenceMin,
  setSearchConfidenceMax,
  executeGallerySearch,
  clearExternalImageSearch,
  clearAllSearchInputs,
  setExternalImageQueryUrl,
  pasteExternalImageSearchFromPasteEvent,
  setExternalImageSearchFromFile,
  selectExternalImageSearchFile,
  pasteExternalImageSearchFromClipboard,
  openSettings,
  startImagePress,
  clearImagePress,
  openGalleryImageDetail,
  openGalleryImageMenu,
}

const overlayHandlers = {
  copyReferenceBoardItemToClipboard,
  canImportReferenceBoardItemToLibrary,
  importSelectedReferenceItemToLibrary,
  exportReferenceBoardItem,
  removeReferenceBoardItem,
  pasteReferenceBoardContent,
  autoArrangeActiveReferenceBoard,
}

function readStoredIdSet(key: string) {
  const raw = localStorage.getItem(key)
  if (!raw) return new Set<number>()
  try {
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return new Set<number>()
    return new Set(
      parsed
        .map((value) => Number(value))
        .filter((value) => Number.isFinite(value) && value > 0),
    )
  } catch {
    return new Set<number>()
  }
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value))
}

function galleryScrollScopeKeyOf(folderId: number | 'all' | 'trash') {
  if (folderId === 'all') return 'all'
  if (folderId === 'trash') return 'trash'
  return `folder:${folderId}`
}

function isPointInsideExternalImageSearchDropZone(x: number, y: number) {
  const node = document.elementFromPoint(x, y) as HTMLElement | null
  if (!node) return false
  return Boolean(node.closest('.gallery-search__lens-drop'))
}

async function setExternalImageSearchFromGalleryDrag(imageId: string) {
  return setExternalImageSearchFromGalleryImage(imageId)
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
</script>

<template>
  <div
    class="app-shell"
    :class="{
      'is-sidebar-pinned': sidebarPinnedEffective,
      'is-titlebar-pinned': isTitlebarPinned,
      'is-window-maximized': isWindowMaximized,
      'is-gallery-search-active': isSearchFocused || isSearchPointerInside,
      'theme-light': themeMode === 'light',
      'theme-dark': themeMode === 'dark',
    }"
  >
    <div class="app-titlebar-hotspot" @mouseenter="showTitlebarByHotspot" />
    <header
      v-if="isTitlebarPinned"
      class="app-titlebar"
      @mouseenter="onTitlebarMouseEnter"
      @mouseleave="onTitlebarMouseLeave"
    >
      <div class="app-titlebar__left">
        <span class="app-titlebar__brand">illuTag</span>
        <span class="app-titlebar__view">
          {{ viewMode === 'settings' ? '设置' : viewMode === 'board' ? '参考板' : '图库' }}
        </span>
      </div>
      <div
        class="app-titlebar__drag"
        data-tauri-drag-region
        @pointerdown="startWindowDrag"
        @dblclick="toggleWindowMaximize"
      />
      <div class="app-titlebar__right">
        <button class="app-titlebar__button app-titlebar__button--win" type="button" @click="minimizeWindow">
          —
        </button>
        <button class="app-titlebar__button app-titlebar__button--win" type="button" @click="toggleWindowMaximize">
          {{ isWindowMaximized ? '❐' : '□' }}
        </button>
        <button class="app-titlebar__button app-titlebar__button--win app-titlebar__button--close" type="button" @click="closeWindow">
          ×
        </button>
      </div>
    </header>
    <div v-if="!isSettingsView" class="sidebar-hotspot" @mouseenter="openSidebarByHover" />
    <div class="right-sidebar-hotspot" @mouseenter="openRightSidebarByHover" />

    <LeftSidebar
      :visible="sidebarOpen"
      :sidebar-pinned="sidebarPinnedEffective"
      :view-mode="viewMode"
      :active-user-folder-id="activeUserFolderId"
      :folder-tree="folderTree"
      :folder-drag-over-id="folderDragOverId"
      :dragged-folder-id="draggedFolderId"
      :renaming-user-folder-id="renamingUserFolderId"
      :renaming-user-folder-name="renamingUserFolderName"
      :folder-context-menu="folderContextMenu"
      :context-menu-style="contextMenuStyle"
      :folder-draft="folderDraft"
      :folder-draft-style="folderDraftStyle"
      :new-folder-name="newFolderName"
      :handlers="leftSidebarHandlers"
    />

    <RightSidebar
      :visible="rightSidebarOpen"
      :right-sidebar-pinned="rightSidebarPinned"
      :reference-board-rows="referenceBoardRows"
      :active-reference-board-id="activeReferenceBoardId"
      :preview-board-item-drag="previewBoardItemDrag"
      :gallery-image-drag-state="dragState ? { overBoardId: dragState.overBoardId } : null"
      :preview-reference-board-ids="[...previewReferenceBoardIds]"
      :reference-board-preview-blocks="referenceBoardPreviewBlocks"
      :dragged-reference-board-id="draggedReferenceBoardId"
      :dragged-reference-board-folder-id="draggedReferenceBoardFolderId"
      :reference-board-drag-over-kind="referenceBoardDragOverKind"
      :reference-board-drag-over-id="referenceBoardDragOverId"
      :board-context-menu="boardContextMenu"
      :board-context-menu-style="boardContextMenuStyle"
      :board-draft="boardDraft"
      :board-draft-style="boardDraftStyle"
      :new-board-name="newBoardName"
      :renaming-reference-board-folder-id="renamingReferenceBoardFolderId"
      :renaming-reference-board-folder-name="renamingReferenceBoardFolderName"
      :renaming-reference-board-id="renamingReferenceBoardId"
      :renaming-reference-board-name="renamingReferenceBoardName"
      :handlers="rightSidebarHandlers"
    />

    <main
      class="workspace"
      :class="{
        'is-titlebar-pinned': isTitlebarPinned,
        'is-settings-view': isSettingsView,
        'is-reference-preview-active': isReferencePreviewActive && rightSidebarPinned,
        'is-right-sidebar-fixed': rightSidebarPinned,
      }"
    >
      <SettingsView
        v-if="viewMode === 'settings'"
        :sidebar-pinned="sidebarPinned"
        :auto-fix-right-sidebar-on-preview="autoFixRightSidebarOnPreview"
        :thumbnail-cache-enabled="thumbnailCacheEnabled"
        :is-thumbnail-generation-running="isThumbnailGenerationRunning"
        :is-thumbnail-generation-paused="isThumbnailGenerationPaused"
        :thumbnail-progress-text="thumbnailProgressText"
        :thumbnail-progress-percent="thumbnailProgressPercent"
        :thumbnail-recent-errors="thumbnailRecentErrors"
        :is-atmosphere-generation-running="isAtmosphereGenerationRunning"
        :is-atmosphere-generation-paused="isAtmosphereGenerationPaused"
        :atmosphere-progress-text="atmosphereProgressText"
        :atmosphere-progress-percent="atmosphereProgressPercent"
        :atmosphere-recent-errors="atmosphereRecentErrors"
        :is-color-signature-generation-running="isColorSignatureGenerationRunning"
        :is-color-signature-generation-paused="isColorSignatureGenerationPaused"
        :color-signature-progress-text="colorSignatureProgressText"
        :color-signature-progress-percent="colorSignatureProgressPercent"
        :color-signature-recent-errors="colorSignatureRecentErrors"
        :auto-scan-on-startup="autoScanOnStartup"
        :is-background-scan-running="isBackgroundScanRunning"
        :is-background-scan-paused="isBackgroundScanPaused"
        :scan-progress-text="scanProgressText"
        :scan-recent-errors="scanRecentErrors"
        :is-natural-language-scan-running="isNaturalLanguageScanRunning"
        :is-natural-language-scan-paused="isNaturalLanguageScanPaused"
        :natural-language-scan-progress-text="naturalLanguageScanProgressText"
        :natural-language-scan-recent-errors="naturalLanguageScanRecentErrors"
        :theme-mode="themeMode"
        :folder-path-input="folderPathInput"
        :is-picking-folder="isPickingFolder"
        :is-adding-folder="isAddingFolder"
        :is-loading="isLoading"
        :error-text="errorText"
        :folders="library.folders"
        :handlers="settingsViewHandlers"
      />

      <ReferenceBoardView
        v-else-if="viewMode === 'board'"
        :active-reference-board="activeReferenceBoard"
        :active-reference-board-items="activeReferenceBoardItems"
        :board-pan="boardPan"
        :board-scale="boardScale"
        :board-canvas-bounds="activeBoardCanvasBounds"
        :selected-reference-board-item-id="selectedReferenceBoardItemId"
        :handlers="referenceBoardViewHandlers"
      />

      <GalleryView
        v-else
        :preview-drag-over-delete-zone="previewDragOverDeleteZone"
        :visible-images="visibleImages"
        :search-panel-style="searchPanelStyle"
        :search-reveal-mode="searchRevealMode"
        :is-search-focused="isSearchFocused"
        :search-zh-input="searchZhInput"
        :search-zh-selected="searchZhSelected"
        :search-zh-suggestions="searchZhSuggestions"
        :search-zh-open="searchZhOpen"
        :search-en-query="searchEnQuery"
        :search-file-name-query="searchFileNameQuery"
        :search-natural-language-query="searchNaturalLanguageQuery"
        :search-mode="searchMode"
        :external-image-search-type="externalImageSearchType"
        :external-image-query-url="externalImageQueryUrl"
        :external-image-query-preview-url="externalImageQueryPreviewUrl"
        :external-image-query-label="externalImageQueryLabel"
        :search-confidence-min="searchConfidenceMin"
        :search-confidence-max="searchConfidenceMax"
        :search-running="searchRunning"
        :search-error="searchError"
        :is-loading="isLoading"
        :layout-items="renderedLayoutItems"
        :total-height="totalHeight"
        :content-width="masonryContentWidth"
        :drag-state="dragState ? { imageId: dragState.imageId, x: dragState.x, y: dragState.y } : null"
        :handlers="galleryViewHandlers"
      />
    </main>

    <AppOverlayLayer
      :reference-board-canvas-menu="referenceBoardCanvasMenu"
      :reference-board-canvas-menu-style="referenceBoardCanvasMenuStyle"
      :drag-state="dragState"
      :drop-folder-tree="dropFolderTree"
      :handlers="overlayHandlers"
    />

    <div
      v-if="importLibraryFolderPickerItemId !== null"
      class="import-library-picker-layer"
      @click="closeImportLibraryFolderPicker(null)"
    >
      <article class="import-library-picker" @click.stop>
        <h3>加入图库</h3>
        <p>选择要保存到的本地图库文件夹</p>
        <div class="import-library-picker__list">
          <button
            v-for="folder in library.folders"
            :key="folder.id"
            type="button"
            class="import-library-picker__option"
            @click="closeImportLibraryFolderPicker(folder.id)"
          >
            {{ folder.path }}
          </button>
        </div>
        <div class="import-library-picker__actions">
          <button type="button" class="secondary-button" @click="closeImportLibraryFolderPicker(null)">取消</button>
        </div>
      </article>
    </div>

    <div v-if="activeImageDetail" class="image-detail-layer" @click="closeImageDetail()">
      <article class="image-detail-modal" @click.stop>
        <button class="image-detail-modal__close" type="button" @click="closeImageDetail()">×</button>
        <div class="image-detail-modal__scroll">
          <div class="image-detail-modal__main">
            <div class="image-detail-modal__media-column">
              <div class="image-detail-modal__media-sticky">
                <div class="image-detail-modal__media" @contextmenu="openImageDetailMenu($event)">
                  <img :src="convertFileSrc(activeImageDetail.path)" :alt="activeImageDetail.fileName" draggable="false" />
                </div>
              </div>
            </div>
            <aside class="image-detail-modal__meta">
              <div class="image-detail-modal__meta-scroll">
                <div class="image-detail-modal__meta-stack">
                  <div class="image-detail-modal__info-block">
                    <h3>{{ activeImageDetail.fileName }}</h3>
                    <div class="image-detail-modal__meta-list">
                      <div class="image-detail-modal__meta-row">
                        <span>尺寸</span>
                        <strong>{{ activeImageDetail.width }} × {{ activeImageDetail.height }}</strong>
                      </div>
                      <div class="image-detail-modal__meta-row">
                        <span>大小</span>
                        <strong>{{ formatFileSize(activeImageDetail.fileSize) }}</strong>
                      </div>
                      <div class="image-detail-modal__meta-row">
                        <span>修改时间</span>
                        <strong>{{ formatTime(activeImageDetail.modifiedAt) }}</strong>
                      </div>
                      <div class="image-detail-modal__meta-row">
                        <span>来源</span>
                        <strong>{{ activeImageDetail.source }}</strong>
                      </div>
                    </div>
                  </div>
                  <section class="image-detail-modal__tags">
                    <h4>自动标签</h4>
                    <div v-if="groupedImageTags.length > 0" class="image-detail-modal__tags-scroll">
                      <div
                        v-for="group in groupedImageTags"
                        :key="group.key"
                        class="image-detail-modal__tag-group"
                      >
                        <div class="image-detail-modal__tag-group-title">{{ group.label }}</div>
                        <div class="image-detail-modal__tag-list">
                          <div
                            v-for="tag in group.rows"
                            :key="`${group.key}:${tag.tagEn}`"
                            class="image-detail-modal__tag-item"
                            role="button"
                            tabindex="0"
                            @click="searchByTagFromImageDetail(tag.tagEn, tag.tagZh)"
                            @keydown.enter.prevent="searchByTagFromImageDetail(tag.tagEn, tag.tagZh)"
                            @keydown.space.prevent="searchByTagFromImageDetail(tag.tagEn, tag.tagZh)"
                          >
                            <div class="image-detail-modal__tag-main">{{ tag.tagZh || tag.tagEn }}</div>
                            <div class="image-detail-modal__tag-sub">{{ tag.tagZh ? tag.tagEn : '' }}</div>
                            <div class="image-detail-modal__tag-score">{{ tag.confidence.toFixed(3) }}</div>
                          </div>
                        </div>
                      </div>
                    </div>
                    <p v-else class="image-detail-modal__tags-empty">暂无自动标签</p>
                  </section>
                </div>
              </div>
            </aside>
          </div>
        </div>

        <div
          v-if="imageDetailContextMenu"
          class="context-menu"
          :style="imageDetailContextMenuStyle"
          @click.stop
          @contextmenu.prevent
        >
          <button type="button" @click="copyGalleryImageToClipboard(activeImageDetail.id)">复制</button>
          <button type="button" @click="exportGalleryImage(activeImageDetail.id)">导出到本地</button>
        </div>
      </article>
    </div>

    <div
      v-if="galleryImageContextMenu"
      class="context-menu"
      :style="galleryImageContextMenuStyle"
      @click.stop
      @contextmenu.prevent
    >
      <button
        v-if="activeUserFolderId === 'all'"
        class="is-danger"
        type="button"
        @click="removeGalleryImageFromIndex(galleryImageContextMenu.imageId)"
      >
        移入回收站
      </button>
      <button
        v-else-if="activeUserFolderId === 'trash'"
        type="button"
        @click="restoreGalleryImageFromTrash(galleryImageContextMenu.imageId)"
      >
        还原
      </button>
      <button
        v-else
        class="is-danger"
        type="button"
        @click="removeGalleryImageFromFolder(galleryImageContextMenu.imageId)"
      >
        从文件夹中移除
      </button>
      <button type="button" @click="copyGalleryImageToClipboard(galleryImageContextMenu.imageId)">复制</button>
      <button type="button" @click="exportGalleryImage(galleryImageContextMenu.imageId)">导出到本地</button>
    </div>

    <div
      v-if="previewBoardItemDrag"
      class="image-drag-preview"
      :style="{ left: `${previewBoardItemDrag.x}px`, top: `${previewBoardItemDrag.y}px` }"
    >
      <img :src="previewBoardItemDrag.thumbnailUrl" alt="" draggable="false" />
      <span
        v-if="previewBoardDragIconKind(previewBoardItemDrag) !== 'none'"
        class="image-drag-preview__copy-icon"
        :class="{
          'is-move': previewBoardDragIconKind(previewBoardItemDrag) === 'move',
          'is-delete': previewBoardDragIconKind(previewBoardItemDrag) === 'delete',
        }"
      >
        {{
          previewBoardDragIconKind(previewBoardItemDrag) === 'delete'
            ? '🗑'
            : previewBoardDragIconKind(previewBoardItemDrag) === 'move'
              ? '→'
              : '+'
        }}
      </span>
    </div>
  </div>
</template>
