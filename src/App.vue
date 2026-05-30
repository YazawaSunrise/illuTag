<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { Pushpin } from '@icon-park/vue-next'
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
import { useTagManagement } from './composables/useTagManagement'
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
const isWindowAlwaysOnTop = ref(false)
const isTitlebarHovered = ref(false)
const boardSpaceFocusMode = ref<'item' | 'canvas'>('item')
const importLibraryFolderPickerItemId = ref<number | null>(null)
const importLibraryFolderPickerResolve = ref<((folderId: number | null) => void) | null>(null)
const sidebarHoverCloseTimer = ref<number | null>(null)
const boardPointerUseMaxAgeMs = 5000
const isSettingsView = computed(() => viewMode.value === 'settings')
const sidebarPinnedEffective = computed(() => isSettingsView.value || sidebarPinned.value)
const sidebarOpen = computed(() => sidebarPinnedEffective.value || sidebarHoverOpen.value)
const rightSidebarOpen = computed(() => rightSidebarPinned.value || rightSidebarHoverOpen.value)
const isTitlebarPinned = computed(() => !isWindowMaximized.value || isTitlebarHovered.value)
const galleryScopeTransitionKey = computed(() => `gallery:${galleryScrollScopeKeyOf(activeUserFolderId.value)}`)
const workspaceTransitionKey = computed(() =>
  viewMode.value === 'gallery' ? galleryScopeTransitionKey.value : viewMode.value,
)

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
  randomGalleryVisitSerial,
  unclassifiedOnlyParentFolderId,
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
  parentFoldersWithUnclassifiedImages,
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
  showRandomImages,
  showFavoriteImages,
  showTrashImages,
  onUserFolderRowClick,
  toggleFolderUnclassifiedOnly,
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

const showGalleryUnclassifiedToggle = computed(() => {
  if (typeof activeUserFolderId.value !== 'number') return false
  if (!folderHasChildren(activeUserFolderId.value)) return false
  return parentFoldersWithUnclassifiedImages.value.has(activeUserFolderId.value)
})

const isGalleryUnclassifiedOnly = computed(() => {
  if (typeof activeUserFolderId.value !== 'number') return false
  return unclassifiedOnlyParentFolderId.value === activeUserFolderId.value
})

function toggleActiveFolderUnclassifiedOnly() {
  if (typeof activeUserFolderId.value !== 'number') return
  toggleFolderUnclassifiedOnly(activeUserFolderId.value)
}

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
  activeImageCustomTags,
  activeImageSupplementTags,
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
  addImageUserCustomTag,
  removeImageUserCustomTag,
  addImageUserSupplementTag,
  removeImageUserSupplementTag,
  suggestKnownAutoTagsForInput,
  findExactKnownAutoTag,
  closeImageDetail,
  openGalleryImageDetail,
} = useGallerySearch<LibraryStore>({
  library,
  folderScopedImages,
  activeUserFolderId,
  randomGalleryVisitSerial,
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

type KnownAutoTagSuggestion = {
  tagEn: string
  tagZh?: string | null
  imageCount: number
  isUserCustom?: boolean
}

const imageDetailCustomTagDraft = ref('')
const imageDetailCustomTagEditorOpen = ref(false)
const imageDetailCustomTagConflict = ref<{
  input: string
  tagEn: string
  tagZh: string | null
} | null>(null)
const imageDetailCustomTagExpandedFolderIds = ref<number[]>([])
const imageDetailCustomTagSelectedExistingTags = ref<string[]>([])
const imageDetailSupplementPickerOpen = ref(false)
const imageDetailSupplementQuery = ref('')
const imageDetailSupplementSuggestions = ref<KnownAutoTagSuggestion[]>([])
const imageDetailSupplementSuggestLoading = ref(false)
const imageDetailSupplementSuggestTimer = ref<number | null>(null)
const imageDetailSupplementSuggestRequestToken = ref(0)

const favoriteVisibleImageIds = computed(() =>
  visibleImages.value.filter((image) => image.isFavorite).map((image) => image.id),
)

const activeTagManagerFolder = computed(() =>
  tagManagerFolders.value.find((folder) => folder.id === activeTagManagerFolderId.value) ?? null,
)

const tagManagerDraggingTagText = ref<string | null>(null)
const tagManagerDragOverFolderId = ref<number | null>(null)
const tagManagerTagContextMenu = ref<{ tagText: string; x: number; y: number } | null>(null)

const activeTagManagerFolderNameForHint = computed(() => activeTagManagerFolder.value?.name ?? '')

const activeTagManagerFolderTags = computed(() => activeTagManagerFolder.value?.tags ?? [])
const tagManagerTagContextMenuStyle = computed(() => {
  if (!tagManagerTagContextMenu.value) return {}
  return {
    left: `${tagManagerTagContextMenu.value.x}px`,
    top: `${tagManagerTagContextMenu.value.y}px`,
  }
})

function startTagManagerTagDrag(tagText: string, event: DragEvent) {
  const normalized = tagText.trim()
  if (!normalized) return
  tagManagerDraggingTagText.value = normalized
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
    event.dataTransfer.setData('text/plain', normalized)
  }
}

function endTagManagerTagDrag() {
  tagManagerDraggingTagText.value = null
  tagManagerDragOverFolderId.value = null
}

function closeTagManagerTagContextMenu() {
  tagManagerTagContextMenu.value = null
}

function openTagManagerTagContextMenu(tagText: string, event: MouseEvent) {
  const normalized = tagText.trim()
  if (!normalized) return
  event.preventDefault()
  event.stopPropagation()
  tagManagerTagContextMenu.value = {
    tagText: normalized,
    x: event.clientX,
    y: event.clientY,
  }
}

async function deleteTagManagerTagFromContextMenu() {
  const current = tagManagerTagContextMenu.value
  if (!current) return
  closeTagManagerTagContextMenu()
  await deleteTagManagerTag(current.tagText)
}

function onTagManagerFolderDragOver(folderId: number, event: DragEvent) {
  if (!tagManagerDraggingTagText.value) return
  event.preventDefault()
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'
  tagManagerDragOverFolderId.value = folderId
}

function onTagManagerFolderDragLeave(folderId: number) {
  if (tagManagerDragOverFolderId.value === folderId) {
    tagManagerDragOverFolderId.value = null
  }
}

async function onTagManagerFolderDrop(folderId: number, event: DragEvent) {
  event.preventDefault()
  const text = event.dataTransfer?.getData('text/plain')?.trim() ?? ''
  const tagText = text || tagManagerDraggingTagText.value || ''
  tagManagerDragOverFolderId.value = null
  if (!tagText) return
  await assignTagToFolder(tagText, folderId)
  tagManagerDraggingTagText.value = null
}

const {
  galleryEl,
  galleryScrollTop,
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

const galleryScrollableHeight = computed(() => {
  const element = galleryEl.value
  if (!element) return 0
  return Math.max(0, element.scrollHeight - element.clientHeight)
})

const galleryScrollProgress = computed(() => {
  const maxScroll = galleryScrollableHeight.value
  if (maxScroll <= 0) return 0
  return clamp(galleryScrollTop.value / maxScroll, 0, 1)
})

const showGalleryScrollProgress = computed(() => viewMode.value === 'gallery' && galleryScrollableHeight.value > 1)

const {
  autoScanOnStartup,
  isBackgroundScanRunning,
  isBackgroundScanPaused,
  scanProgressText,
  scanRecentErrors,
  lastBackgroundScanProgress,
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
  refreshNaturalLanguageScanStatus,
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
  refreshThumbnailGenerationStatus,
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
  refreshAtmosphereGenerationStatus,
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
  refreshColorSignatureGenerationStatus,
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

const {
  tagManagerOpen,
  isTagManagerLoading,
  tagManagerFolders,
  activeTagManagerFolderId,
  tagManagerUnclassifiedTags,
  newTagManagerFolderName,
  newTagManagerTagText,
  openTagManager,
  closeTagManager,
  reloadTagManagementState,
  createTagManagerFolder,
  createTagManagerTag,
  deleteTagManagerTag,
  assignTagToFolder,
  unassignTag,
  deleteTagManagerFolder,
} = useTagManagement({
  formatError,
  setErrorText(value) {
    errorText.value = value
  },
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
  window.addEventListener('click', closeTagManagerTagContextMenu)
  window.addEventListener('keydown', handleGlobalKeydown)
  void refreshBackgroundScanStatus()
  void refreshWindowAlwaysOnTop()
  startBackgroundScanPolling()
  startThumbnailGenerationPolling()
  startAtmosphereGenerationPolling()
  startColorSignatureGenerationPolling()
  void startStartupCleanup()
  void runStartupAutoScanPipeline()
  if (thumbnailCacheEnabled.value && !isBackgroundScanRunning.value && !autoScanOnStartup.value) {
    void startThumbnailGeneration()
  }
})

const startupAutoScanPipelineRunning = ref(false)

function sleep(ms: number) {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, ms)
  })
}

async function waitUntilIdle(refresh: () => Promise<void>, isRunning: () => boolean) {
  await refresh()
  while (isRunning()) {
    await sleep(700)
    await refresh()
  }
}

async function runStartupAutoScanPipeline() {
  if (startupAutoScanPipelineRunning.value) return
  startupAutoScanPipelineRunning.value = true
  try {
    const started = await startAutoScanIfEnabled()
    if (!started) return

    await waitUntilIdle(refreshBackgroundScanStatus, () => isBackgroundScanRunning.value)
    const newImages = Math.max(0, Number(lastBackgroundScanProgress.value?.newImages ?? 0))
    if (newImages <= 0) return

    await startThumbnailGeneration()
    await waitUntilIdle(refreshThumbnailGenerationStatus, () => isThumbnailGenerationRunning.value)

    await startNaturalLanguageScan()
    await waitUntilIdle(refreshNaturalLanguageScanStatus, () => isNaturalLanguageScanRunning.value)

    await startAtmosphereGeneration()
    await waitUntilIdle(refreshAtmosphereGenerationStatus, () => isAtmosphereGenerationRunning.value)

    await startColorSignatureGeneration()
    await waitUntilIdle(refreshColorSignatureGenerationStatus, () => isColorSignatureGenerationRunning.value)

    await startScanAllFolders()
  } finally {
    startupAutoScanPipelineRunning.value = false
  }
}

onUnmounted(() => {
  clearSidebarHoverCloseTimer()
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
  window.removeEventListener('click', closeTagManagerTagContextMenu)
  window.removeEventListener('keydown', handleGlobalKeydown)
  closeImportLibraryFolderPicker(null)
  resetImageDetailUserTagEditor()
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

watch(
  () => activeImageDetailId.value,
  () => {
    resetImageDetailUserTagEditor()
  },
)

watch(
  () => imageDetailSupplementQuery.value,
  () => {
    if (!imageDetailSupplementPickerOpen.value) return
    queueImageDetailSupplementSuggestions()
  },
)

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
    if (nextViewMode === 'gallery' && prevFolderId !== nextFolderId) {
      clearAllSearchInputs()
    }

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
  if (startupAutoScanPipelineRunning.value) return
  if (isBackgroundScanRunning.value) return
  void startThumbnailGeneration()
})

watch(isBackgroundScanRunning, (running, wasRunning) => {
  if (!wasRunning || running) return
  if (startupAutoScanPipelineRunning.value) return
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
    closeTagManager()
    endTagManagerTagDrag()
    closeTagManagerTagContextMenu()
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

async function flipReferenceBoardItemHorizontal(itemId: number) {
  await flipReferenceBoardItem(itemId, 'horizontal')
}

async function flipReferenceBoardItemVertical(itemId: number) {
  await flipReferenceBoardItem(itemId, 'vertical')
}

async function flipReferenceBoardItem(itemId: number, direction: 'horizontal' | 'vertical') {
  const item = library.value.referenceBoardItems.find((entry) => entry.id === itemId)
  if (!item) return
  const boardId = item.boardId
  const beforeMap = collectBoardLayoutMap(boardId)
  const selectionBefore = selectedReferenceBoardItemId.value

  const nextFlipX = direction === 'horizontal' ? !item.flipX : item.flipX
  const nextFlipY = direction === 'vertical' ? !item.flipY : item.flipY
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('update_reference_board_item_layout_command', {
      itemId: item.id,
      x: item.x,
      y: item.y,
      width: item.width,
      height: item.height,
      rotation: item.rotation,
      flipX: nextFlipX,
      flipY: nextFlipY,
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

async function refreshWindowAlwaysOnTop() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    isWindowAlwaysOnTop.value = await invoke<boolean>('window_is_always_on_top_command')
  } catch {
    isWindowAlwaysOnTop.value = false
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

async function toggleWindowAlwaysOnTop() {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    isWindowAlwaysOnTop.value = await invoke<boolean>('window_toggle_always_on_top_command')
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
  clearSidebarHoverCloseTimer()
  if (isSettingsView.value) return
  if (!sidebarPinned.value) sidebarHoverOpen.value = true
}

function openRightSidebarByHover() {
  if (!rightSidebarPinned.value) rightSidebarHoverOpen.value = true
}

function closeSidebarByHover() {
  clearSidebarHoverCloseTimer()
  sidebarHoverCloseTimer.value = window.setTimeout(() => {
    sidebarHoverCloseTimer.value = null
    if (
      !sidebarPinnedEffective.value &&
      !folderDraft.value &&
      !isComposingFolderName.value &&
      renamingUserFolderId.value === null &&
      !isComposingUserFolderRename.value &&
      draggedFolderId.value === null &&
      !isSidebarHoverSafeAreaActive()
    ) {
      sidebarHoverOpen.value = false
      closeFolderContextMenu()
    }
  }, 90)
}

function clearSidebarHoverCloseTimer() {
  if (sidebarHoverCloseTimer.value === null) return
  window.clearTimeout(sidebarHoverCloseTimer.value)
  sidebarHoverCloseTimer.value = null
}

function isSidebarHoverSafeAreaActive() {
  const sidebar = document.querySelector('.sidebar')
  if (sidebar instanceof HTMLElement && sidebar.matches(':hover')) {
    return true
  }
  const hotspot = document.querySelector('.sidebar-hotspot')
  if (hotspot instanceof HTMLElement && hotspot.matches(':hover')) {
    return true
  }
  return false
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
  closeTagManager()
  closeTagManagerTagContextMenu()
  viewMode.value = 'settings'
  sidebarHoverOpen.value = false
}

function openTagManagerPanel() {
  viewMode.value = 'gallery'
  void openTagManager()
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

async function toggleGalleryImageFavorite(imageId: string, favorite: boolean) {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('toggle_image_favorite_command', {
      imageId,
      favorite,
    })
  } catch (error) {
    errorText.value = formatError(error)
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

function resetImageDetailUserTagEditor() {
  imageDetailCustomTagDraft.value = ''
  imageDetailCustomTagEditorOpen.value = false
  imageDetailCustomTagConflict.value = null
  imageDetailCustomTagExpandedFolderIds.value = []
  imageDetailCustomTagSelectedExistingTags.value = []
  imageDetailSupplementPickerOpen.value = false
  imageDetailSupplementQuery.value = ''
  imageDetailSupplementSuggestions.value = []
  imageDetailSupplementSuggestLoading.value = false
  if (imageDetailSupplementSuggestTimer.value !== null) {
    window.clearTimeout(imageDetailSupplementSuggestTimer.value)
    imageDetailSupplementSuggestTimer.value = null
  }
}

function openImageDetailCustomTagEditor() {
  imageDetailCustomTagEditorOpen.value = true
  imageDetailCustomTagDraft.value = ''
  imageDetailCustomTagExpandedFolderIds.value = []
  imageDetailCustomTagSelectedExistingTags.value = []
  void reloadTagManagementState()
}

function cancelImageDetailCustomTagEditor() {
  imageDetailCustomTagEditorOpen.value = false
  imageDetailCustomTagDraft.value = ''
  imageDetailCustomTagExpandedFolderIds.value = []
  imageDetailCustomTagSelectedExistingTags.value = []
}

function isImageDetailTagFolderExpanded(folderId: number) {
  return imageDetailCustomTagExpandedFolderIds.value.includes(folderId)
}

function toggleImageDetailTagFolderExpanded(folderId: number) {
  if (isImageDetailTagFolderExpanded(folderId)) {
    imageDetailCustomTagExpandedFolderIds.value = imageDetailCustomTagExpandedFolderIds.value.filter((id) => id !== folderId)
    return
  }
  imageDetailCustomTagExpandedFolderIds.value = [...imageDetailCustomTagExpandedFolderIds.value, folderId]
}

function isImageDetailExistingTagSelected(tagText: string) {
  return imageDetailCustomTagSelectedExistingTags.value.includes(tagText)
}

function toggleImageDetailExistingTag(tagText: string) {
  const normalized = tagText.trim()
  if (!normalized) return
  if (isImageDetailExistingTagSelected(normalized)) {
    imageDetailCustomTagSelectedExistingTags.value = imageDetailCustomTagSelectedExistingTags.value.filter((tag) => tag !== normalized)
    return
  }
  imageDetailCustomTagSelectedExistingTags.value = [...imageDetailCustomTagSelectedExistingTags.value, normalized]
}

async function submitImageDetailCustomTagDraft() {
  const imageId = activeImageDetailId.value
  if (!imageId) return
  const input = imageDetailCustomTagDraft.value.trim()
  const selectedTags = Array.from(
    new Set(imageDetailCustomTagSelectedExistingTags.value.map((tag) => tag.trim()).filter((tag) => tag.length > 0)),
  )
  if (!input && selectedTags.length === 0) return
  try {
    const matched = input ? await findExactKnownAutoTag(input) : null
    if (input && matched) {
      imageDetailCustomTagConflict.value = {
        input,
        tagEn: matched.tagEn,
        tagZh: matched.tagZh ?? null,
      }
      imageDetailCustomTagEditorOpen.value = false
      return
    }
    const existingCustomTagSet = new Set(activeImageCustomTags.value.map((tag) => tag.tagText))
    if (input && !existingCustomTagSet.has(input)) {
      await addImageUserCustomTag(imageId, input)
      existingCustomTagSet.add(input)
    }
    for (const selectedTag of selectedTags) {
      if (existingCustomTagSet.has(selectedTag)) continue
      await addImageUserCustomTag(imageId, selectedTag)
      existingCustomTagSet.add(selectedTag)
    }
    imageDetailCustomTagDraft.value = ''
    imageDetailCustomTagEditorOpen.value = false
    imageDetailCustomTagSelectedExistingTags.value = []
    imageDetailCustomTagExpandedFolderIds.value = []
  } catch (error) {
    errorText.value = formatError(error)
  }
}

async function resolveImageDetailCustomTagConflict(mode: 'supplement' | 'custom') {
  const conflict = imageDetailCustomTagConflict.value
  const imageId = activeImageDetailId.value
  if (!conflict || !imageId) return
  try {
    if (mode === 'supplement') {
      await addImageUserSupplementTag(imageId, conflict.tagEn, conflict.tagZh)
    } else {
      const suffix = '（自定义）'
      const targetTag = conflict.input.endsWith(suffix) ? conflict.input : `${conflict.input}${suffix}`
      await addImageUserCustomTag(imageId, targetTag)
    }
    imageDetailCustomTagConflict.value = null
    imageDetailCustomTagDraft.value = ''
  } catch (error) {
    errorText.value = formatError(error)
  }
}

async function removeImageDetailCustomTag(tagText: string) {
  const imageId = activeImageDetailId.value
  if (!imageId) return
  try {
    await removeImageUserCustomTag(imageId, tagText)
  } catch (error) {
    errorText.value = formatError(error)
  }
}

async function removeImageDetailSupplementTag(tagEn: string) {
  const imageId = activeImageDetailId.value
  if (!imageId) return
  try {
    await removeImageUserSupplementTag(imageId, tagEn)
  } catch (error) {
    errorText.value = formatError(error)
  }
}

async function refreshImageDetailSupplementSuggestionsNow() {
  if (!imageDetailSupplementPickerOpen.value) return
  const keyword = imageDetailSupplementQuery.value.trim()
  if (!keyword) {
    imageDetailSupplementSuggestions.value = []
    imageDetailSupplementSuggestLoading.value = false
    return
  }
  const token = imageDetailSupplementSuggestRequestToken.value + 1
  imageDetailSupplementSuggestRequestToken.value = token
  imageDetailSupplementSuggestLoading.value = true
  try {
    const rows = await suggestKnownAutoTagsForInput(keyword, 60, { includeDictionary: true })
    if (token !== imageDetailSupplementSuggestRequestToken.value) return
    const existing = new Set(activeImageSupplementTags.value.map((item) => item.tagEn))
    imageDetailSupplementSuggestions.value = rows.filter((item) => !existing.has(item.tagEn))
  } catch (error) {
    if (token !== imageDetailSupplementSuggestRequestToken.value) return
    imageDetailSupplementSuggestions.value = []
    errorText.value = formatError(error)
  } finally {
    if (token === imageDetailSupplementSuggestRequestToken.value) {
      imageDetailSupplementSuggestLoading.value = false
    }
  }
}

function queueImageDetailSupplementSuggestions() {
  if (imageDetailSupplementSuggestTimer.value !== null) {
    window.clearTimeout(imageDetailSupplementSuggestTimer.value)
    imageDetailSupplementSuggestTimer.value = null
  }
  imageDetailSupplementSuggestTimer.value = window.setTimeout(() => {
    imageDetailSupplementSuggestTimer.value = null
    void refreshImageDetailSupplementSuggestionsNow()
  }, 140)
}

function openImageDetailSupplementPicker() {
  imageDetailSupplementPickerOpen.value = true
  imageDetailSupplementQuery.value = ''
  imageDetailSupplementSuggestions.value = []
}

function closeImageDetailSupplementPicker() {
  imageDetailSupplementPickerOpen.value = false
  imageDetailSupplementQuery.value = ''
  imageDetailSupplementSuggestions.value = []
  imageDetailSupplementSuggestLoading.value = false
}

async function addImageDetailSupplementTag(suggestion: KnownAutoTagSuggestion) {
  const imageId = activeImageDetailId.value
  if (!imageId) return
  try {
    await addImageUserSupplementTag(imageId, suggestion.tagEn, suggestion.tagZh ?? null)
    closeImageDetailSupplementPicker()
  } catch (error) {
    errorText.value = formatError(error)
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
  showRandomImages,
  showFavoriteImages,
  showTrashImages,
  openTagManager: openTagManagerPanel,
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
  toggleActiveFolderUnclassifiedOnly,
  scrollGalleryToCurrentTop,
  startImagePress,
  clearImagePress,
  toggleGalleryImageFavorite,
  openGalleryImageDetail,
  openGalleryImageMenu,
}

const overlayHandlers = {
  copyReferenceBoardItemToClipboard,
  canImportReferenceBoardItemToLibrary,
  importSelectedReferenceItemToLibrary,
  exportReferenceBoardItem,
  flipReferenceBoardItemHorizontal,
  flipReferenceBoardItemVertical,
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

function galleryScrollScopeKeyOf(folderId: number | 'all' | 'random' | 'favorites' | 'trash') {
  if (folderId === 'all') return 'all'
  if (folderId === 'random') return 'random'
  if (folderId === 'favorites') return 'favorites'
  if (folderId === 'trash') return 'trash'
  return `folder:${folderId}`
}

function scrollGalleryToCurrentTop() {
  scrollGalleryToTop(galleryScrollScopeKeyOf(activeUserFolderId.value))
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
        <button
          class="app-titlebar__button app-titlebar__button--win app-titlebar__button--pin"
          :class="{ 'is-active': isWindowAlwaysOnTop }"
          type="button"
          :aria-label="isWindowAlwaysOnTop ? '取消置顶' : '窗口置顶'"
          @click="toggleWindowAlwaysOnTop"
        >
          <Pushpin
            class="app-titlebar__button--pin-icon"
            theme="outline"
            :size="14"
            :fill="['currentColor']"
            aria-hidden="true"
          />
        </button>
        <button
          class="app-titlebar__button app-titlebar__button--win app-titlebar__button--minimize"
          type="button"
          aria-label="最小化"
          @click="minimizeWindow"
        >
          <span class="app-titlebar__button-icon" aria-hidden="true" />
        </button>
        <button
          class="app-titlebar__button app-titlebar__button--win"
          :class="isWindowMaximized ? 'app-titlebar__button--restore' : 'app-titlebar__button--maximize'"
          type="button"
          :aria-label="isWindowMaximized ? '还原' : '最大化'"
          @click="toggleWindowMaximize"
        >
          <span class="app-titlebar__button-icon" aria-hidden="true" />
        </button>
        <button
          class="app-titlebar__button app-titlebar__button--win app-titlebar__button--close"
          type="button"
          aria-label="关闭"
          @click="closeWindow"
        >
          <span class="app-titlebar__button-icon" aria-hidden="true" />
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
      :tag-manager-open="tagManagerOpen"
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
      <Transition name="workspace-switch" mode="out-in">
        <div :key="workspaceTransitionKey" class="workspace-view">
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
            :show-unclassified-toggle="showGalleryUnclassifiedToggle"
            :is-unclassified-only="isGalleryUnclassifiedOnly"
            :favorite-image-ids="favoriteVisibleImageIds"
            :layout-items="renderedLayoutItems"
            :total-height="totalHeight"
            :content-width="masonryContentWidth"
            :drag-state="dragState ? { imageId: dragState.imageId, x: dragState.x, y: dragState.y } : null"
            :handlers="galleryViewHandlers"
          />
        </div>
      </Transition>
      <div v-if="showGalleryScrollProgress" class="gallery-scroll-progress-indicator" aria-hidden="true">
        <div class="gallery-scroll-progress-indicator__track">
          <div
            class="gallery-scroll-progress-indicator__fill"
            :style="{ transform: `scaleY(${galleryScrollProgress})` }"
          />
        </div>
      </div>
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

    <div
      v-if="tagManagerOpen"
      class="tag-manager-layer"
      @click="
        closeTagManager();
        endTagManagerTagDrag();
        closeTagManagerTagContextMenu();
      "
    >
      <article class="tag-manager-modal" @click.stop>
        <header class="tag-manager-modal__header">
          <div class="tag-manager-modal__title">标签管理</div>
          <button
            class="tag-manager-modal__close"
            type="button"
            @click="
              closeTagManager();
              endTagManagerTagDrag();
              closeTagManagerTagContextMenu();
            "
          >
            ×
          </button>
        </header>
        <div class="tag-manager-modal__content">
          <aside class="tag-manager-modal__folders">
            <div class="tag-manager-modal__section-title">标签文件夹</div>
            <div class="tag-manager-modal__folder-list">
              <button
                v-for="folder in tagManagerFolders"
                :key="folder.id"
                type="button"
                class="tag-manager-modal__folder-item"
                :class="{
                  'is-active': activeTagManagerFolderId === folder.id,
                  'is-drop-target': tagManagerDragOverFolderId === folder.id,
                }"
                @click="activeTagManagerFolderId = folder.id"
                @dragover="onTagManagerFolderDragOver(folder.id, $event)"
                @dragleave="onTagManagerFolderDragLeave(folder.id)"
                @drop="onTagManagerFolderDrop(folder.id, $event)"
              >
                <span>{{ folder.name }}</span>
                <small>{{ folder.tags.length }}</small>
              </button>
            </div>
            <div class="tag-manager-modal__create-row">
              <input
                v-model.trim="newTagManagerFolderName"
                class="tag-manager-modal__create-input"
                type="text"
                placeholder="新建标签文件夹"
                @keydown.enter.prevent="createTagManagerFolder()"
              />
              <button type="button" class="secondary-button tag-manager-modal__action" @click="createTagManagerFolder()">
                新建
              </button>
            </div>
            <button
              type="button"
              class="danger-button tag-manager-modal__action"
              :disabled="activeTagManagerFolderId === null"
              @click="activeTagManagerFolderId !== null ? deleteTagManagerFolder(activeTagManagerFolderId) : null"
            >
              删除当前文件夹
            </button>
          </aside>
          <section class="tag-manager-modal__tags">
            <div class="tag-manager-modal__section-title">
              未分类标签
              <span v-if="activeTagManagerFolderNameForHint">（拖拽到左侧「{{ activeTagManagerFolderNameForHint }}」）</span>
            </div>
            <div class="tag-manager-modal__tag-list">
              <span
                v-for="tag in tagManagerUnclassifiedTags"
                :key="`unclassified:${tag}`"
                class="gallery-search__chip tag-manager-modal__tag-chip"
                draggable="true"
                @dragstart="startTagManagerTagDrag(tag, $event)"
                @dragend="endTagManagerTagDrag()"
                @contextmenu.prevent="openTagManagerTagContextMenu(tag, $event)"
              >
                <span class="gallery-search__chip-text">{{ tag }}</span>
              </span>
            </div>
            <div class="tag-manager-modal__create-row">
              <input
                v-model.trim="newTagManagerTagText"
                class="tag-manager-modal__create-input"
                type="text"
                placeholder="新建标签"
                @keydown.enter.prevent="createTagManagerTag()"
              />
              <button type="button" class="secondary-button tag-manager-modal__action" @click="createTagManagerTag()">
                新建标签
              </button>
            </div>
            <div class="tag-manager-modal__section-title">当前文件夹标签</div>
            <div class="tag-manager-modal__tag-list">
              <span
                v-for="tag in activeTagManagerFolderTags"
                :key="`folder-tag:${tag}`"
                class="gallery-search__chip tag-manager-modal__tag-chip"
                draggable="true"
                @dragstart="startTagManagerTagDrag(tag, $event)"
                @dragend="endTagManagerTagDrag()"
                @contextmenu.prevent="openTagManagerTagContextMenu(tag, $event)"
              >
                <span class="gallery-search__chip-text">{{ tag }}</span>
                <button type="button" class="gallery-search__chip-remove" @click.stop="unassignTag(tag)">×</button>
              </span>
              <p v-if="activeTagManagerFolderTags.length === 0" class="tag-manager-modal__placeholder">当前文件夹还没有标签</p>
            </div>
            <div v-if="isTagManagerLoading" class="tag-manager-modal__placeholder">处理中...</div>
          </section>
        </div>
        <div
          v-if="tagManagerTagContextMenu"
          class="context-menu"
          :style="tagManagerTagContextMenuStyle"
          @click.stop
          @contextmenu.prevent
        >
          <button class="is-danger" type="button" @click="deleteTagManagerTagFromContextMenu()">删除标签</button>
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
                  <section class="image-detail-modal__user-tags-section">
                    <h4>自定义标签</h4>
                    <div class="image-detail-modal__user-tags-box">
                      <div class="image-detail-modal__user-tags-scroll">
                        <button
                          v-if="activeImageCustomTags.length === 0"
                          type="button"
                          class="image-detail-modal__chip-plus"
                          @click="openImageDetailCustomTagEditor()"
                        >
                          +
                        </button>
                        <span
                          v-for="tag in activeImageCustomTags"
                          :key="`custom:${tag.tagText}`"
                          class="gallery-search__chip image-detail-modal__user-chip"
                        >
                          <span class="gallery-search__chip-text">{{ tag.tagText }}</span>
                          <button
                            type="button"
                            class="gallery-search__chip-remove"
                            @click.stop="removeImageDetailCustomTag(tag.tagText)"
                          >
                            ×
                          </button>
                        </span>
                        <button
                          v-if="activeImageCustomTags.length > 0"
                          type="button"
                          class="image-detail-modal__chip-plus"
                          @click="openImageDetailCustomTagEditor()"
                        >
                          +
                        </button>
                      </div>
                    </div>
                  </section>
                  <section class="image-detail-modal__user-tags-section">
                    <h4>补充自动标签</h4>
                    <div class="image-detail-modal__user-tags-box">
                      <div class="image-detail-modal__user-tags-scroll">
                        <button
                          v-if="activeImageSupplementTags.length === 0"
                          type="button"
                          class="image-detail-modal__chip-plus"
                          @click="openImageDetailSupplementPicker()"
                        >
                          +
                        </button>
                        <span
                          v-for="tag in activeImageSupplementTags"
                          :key="`supplement:${tag.tagEn}`"
                          class="gallery-search__chip image-detail-modal__user-chip"
                        >
                          <span class="gallery-search__chip-text">{{ tag.tagZh || tag.tagEn }}</span>
                          <button
                            type="button"
                            class="gallery-search__chip-remove"
                            @click.stop="removeImageDetailSupplementTag(tag.tagEn)"
                          >
                            ×
                          </button>
                        </span>
                        <button
                          v-if="activeImageSupplementTags.length > 0"
                          type="button"
                          class="image-detail-modal__chip-plus"
                          @click="openImageDetailSupplementPicker()"
                        >
                          +
                        </button>
                      </div>
                    </div>
                  </section>
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
          v-if="imageDetailCustomTagEditorOpen"
          class="image-detail-modal__dialog-layer"
          @click="cancelImageDetailCustomTagEditor()"
        >
          <article class="image-detail-modal__dialog" @click.stop>
            <h4>添加自定义标签</h4>
            <input
              v-model.trim="imageDetailCustomTagDraft"
              class="image-detail-modal__dialog-input"
              type="text"
              placeholder="创建标签"
              @keydown.enter.prevent="submitImageDetailCustomTagDraft()"
            />
            <section class="image-detail-modal__existing-tags-picker">
              <div class="image-detail-modal__existing-tags-title">从已有的标签中选择</div>
              <div
                v-if="tagManagerFolders.length > 0 || tagManagerUnclassifiedTags.length > 0"
                class="image-detail-modal__existing-folder-list"
              >
                <div
                  v-for="folder in tagManagerFolders"
                  :key="`detail-custom-folder:${folder.id}`"
                  class="image-detail-modal__existing-folder"
                >
                  <button
                    type="button"
                    class="image-detail-modal__existing-folder-toggle"
                    @click="toggleImageDetailTagFolderExpanded(folder.id)"
                  >
                    <span class="image-detail-modal__existing-folder-caret">
                      {{ isImageDetailTagFolderExpanded(folder.id) ? '▾' : '▸' }}
                    </span>
                    <span class="image-detail-modal__existing-folder-name">{{ folder.name }}</span>
                    <small>{{ folder.tags.length }}</small>
                  </button>
                  <div v-if="isImageDetailTagFolderExpanded(folder.id)" class="image-detail-modal__existing-tag-list">
                    <button
                      v-for="tagText in folder.tags"
                      :key="`detail-custom-folder-tag:${folder.id}:${tagText}`"
                      type="button"
                      class="gallery-search__chip image-detail-modal__existing-tag-chip"
                      :class="{ 'is-selected': isImageDetailExistingTagSelected(tagText) }"
                      @click="toggleImageDetailExistingTag(tagText)"
                    >
                      <span class="gallery-search__chip-text">{{ tagText }}</span>
                    </button>
                    <p v-if="folder.tags.length === 0" class="image-detail-modal__dialog-empty">该文件夹暂无标签</p>
                  </div>
                </div>
                <div v-if="tagManagerUnclassifiedTags.length > 0" class="image-detail-modal__existing-folder">
                  <button
                    type="button"
                    class="image-detail-modal__existing-folder-toggle"
                    @click="toggleImageDetailTagFolderExpanded(-1)"
                  >
                    <span class="image-detail-modal__existing-folder-caret">
                      {{ isImageDetailTagFolderExpanded(-1) ? '▾' : '▸' }}
                    </span>
                    <span class="image-detail-modal__existing-folder-name">未分类标签</span>
                    <small>{{ tagManagerUnclassifiedTags.length }}</small>
                  </button>
                  <div v-if="isImageDetailTagFolderExpanded(-1)" class="image-detail-modal__existing-tag-list">
                    <button
                      v-for="tagText in tagManagerUnclassifiedTags"
                      :key="`detail-custom-unclassified-tag:${tagText}`"
                      type="button"
                      class="gallery-search__chip image-detail-modal__existing-tag-chip"
                      :class="{ 'is-selected': isImageDetailExistingTagSelected(tagText) }"
                      @click="toggleImageDetailExistingTag(tagText)"
                    >
                      <span class="gallery-search__chip-text">{{ tagText }}</span>
                    </button>
                  </div>
                </div>
              </div>
              <p v-else class="image-detail-modal__dialog-empty">暂无可选标签</p>
            </section>
            <div class="image-detail-modal__dialog-actions">
              <button type="button" class="secondary-button" @click="cancelImageDetailCustomTagEditor()">取消</button>
              <button type="button" class="primary-button" @click="submitImageDetailCustomTagDraft()">添加</button>
            </div>
          </article>
        </div>

        <div
          v-if="imageDetailCustomTagConflict"
          class="image-detail-modal__dialog-layer"
          @click="imageDetailCustomTagConflict = null"
        >
          <article class="image-detail-modal__dialog" @click.stop>
            <h4>标签已存在</h4>
            <p class="image-detail-modal__dialog-text">
              已有此自动标签（{{ imageDetailCustomTagConflict.tagZh || imageDetailCustomTagConflict.tagEn }}），进行补充还是新建用户自定义标签？
            </p>
            <div class="image-detail-modal__dialog-actions">
              <button type="button" class="secondary-button" @click="imageDetailCustomTagConflict = null">取消</button>
              <button type="button" class="secondary-button" @click="resolveImageDetailCustomTagConflict('supplement')">
                补充
              </button>
              <button type="button" class="primary-button" @click="resolveImageDetailCustomTagConflict('custom')">
                自定义
              </button>
            </div>
          </article>
        </div>

        <div
          v-if="imageDetailSupplementPickerOpen"
          class="image-detail-modal__dialog-layer"
          @click="closeImageDetailSupplementPicker()"
        >
          <article class="image-detail-modal__dialog image-detail-modal__dialog--wide" @click.stop>
            <h4>补充自动标签</h4>
            <input
              v-model.trim="imageDetailSupplementQuery"
              class="image-detail-modal__dialog-input"
              type="text"
              placeholder="搜索已有自动标签"
            />
            <div class="image-detail-modal__dialog-list">
              <button
                v-for="item in imageDetailSupplementSuggestions"
                :key="`supplement-pick:${item.tagEn}`"
                type="button"
                class="image-detail-modal__dialog-option image-detail-modal__dialog-option--dual"
                @click="addImageDetailSupplementTag(item)"
              >
                <span class="image-detail-modal__dialog-option-main">{{ item.tagZh || item.tagEn }}</span>
                <span class="image-detail-modal__dialog-option-sub">{{ item.tagZh ? item.tagEn : '' }}</span>
              </button>
              <p
                v-if="!imageDetailSupplementSuggestLoading && imageDetailSupplementQuery && imageDetailSupplementSuggestions.length === 0"
                class="image-detail-modal__dialog-empty"
              >
                未找到可添加的标签
              </p>
              <p v-if="imageDetailSupplementSuggestLoading" class="image-detail-modal__dialog-empty">搜索中...</p>
            </div>
            <div class="image-detail-modal__dialog-actions">
              <button type="button" class="secondary-button" @click="closeImageDetailSupplementPicker()">关闭</button>
            </div>
          </article>
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
        v-if="
          activeUserFolderId === 'all' ||
          activeUserFolderId === 'random' ||
          activeUserFolderId === 'favorites'
        "
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
