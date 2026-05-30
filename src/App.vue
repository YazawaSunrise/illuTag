<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'
import { FolderClose, FolderOpen, Pushpin } from '@icon-park/vue-next'
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
const removeFolderConfirmPath = ref<string | null>(null)
const systemTrashMoveErrorMessage = ref<string | null>(null)
const activeReferenceBoardId = ref<number | null>(null)
const isWindowMaximized = ref(false)
const isWindowAlwaysOnTop = ref(false)
const isTitlebarHovered = ref(false)
const boardSpaceFocusMode = ref<'item' | 'canvas'>('item')
const importLibraryFolderPickerItemId = ref<number | null>(null)
const importLibraryFolderPickerResolve = ref<((folderId: number | null) => void) | null>(null)
const sidebarHoverCloseTimer = ref<number | null>(null)
const rightSidebarHoverCloseTimer = ref<number | null>(null)
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
const isGalleryBatchMode = ref(false)
const galleryBatchSelectedImageIds = ref<string[]>([])
type GalleryBatchActionItem = {
  key:
    | 'copy-folder'
    | 'move-folder'
    | 'add-tags'
    | 'remove-from-folder'
    | 'assign-folder'
    | 'favorite'
    | 'trash'
  label: string
}
type PendingBatchTag =
  | {
      id: string
      kind: 'custom'
      tagText: string
      label: string
      subLabel?: string
    }
  | {
      id: string
      kind: 'supplement'
      tagEn: string
      tagZh: string | null
      label: string
      subLabel: string
    }
const batchFolderPickerModal = ref<null | { mode: 'copy' | 'move' | 'assign'; title: string; confirmLabel: string }>(null)
const batchFolderPickerTargetId = ref<number | null>(null)
const batchTagModalOpen = ref(false)
const batchTagDraft = ref('')
const batchTagSuggestions = ref<KnownAutoTagSuggestion[]>([])
const batchTagPending = ref<PendingBatchTag[]>([])
const batchTagCustomConflict = ref<{
  input: string
  tagEn: string
  tagZh: string | null
} | null>(null)
const batchTagExpandedFolderIds = ref<number[]>([])
const batchTagSuggestTimer = ref<number | null>(null)
const batchTagSuggestLoading = ref(false)
const batchTagSuggestToken = ref(0)
type FolderRuleConditionDraft = {
  id: number
  logic: 'AND' | 'OR' | 'NOT'
  source: 'danbooru' | 'custom' | 'filename'
  keyword: string
}
type FolderRuleTagGroup = {
  key: string
  title: string
  tags: string[]
}
const folderRuleEditor = ref<{
  folderId: number
  folderName: string
  conditions: FolderRuleConditionDraft[]
} | null>(null)
const folderRuleSeed = ref(1)
const folderRuleDanbooruActiveConditionId = ref<number | null>(null)
const folderRuleDanbooruSuggestions = ref<KnownAutoTagSuggestion[]>([])
const folderRuleDanbooruSuggestLoading = ref(false)
const folderRuleDanbooruSuggestTimer = ref<number | null>(null)
const folderRuleDanbooruSuggestToken = ref(0)
const batchSelectedImageIds = computed(() => Array.from(new Set(galleryBatchSelectedImageIds.value)))
const isGalleryBatchAllSelected = computed(() => {
  if (!isGalleryBatchMode.value) return false
  const visibleIds = visibleImages.value.map((image) => image.id)
  if (visibleIds.length === 0) return false
  const selected = new Set(galleryBatchSelectedImageIds.value)
  return visibleIds.every((id) => selected.has(id))
})
const galleryBatchActionLabels = computed<GalleryBatchActionItem[]>(() => {
  if (typeof activeUserFolderId.value === 'number') {
    return [
      { key: 'copy-folder', label: '复制到其他文件夹' },
      { key: 'move-folder', label: '移动到其他文件夹' },
      { key: 'add-tags', label: '添加标签' },
      { key: 'remove-from-folder', label: '从文件夹中删除' },
    ]
  }
  return [
    { key: 'assign-folder', label: '归类到文件夹' },
    { key: 'add-tags', label: '添加标签' },
    { key: 'favorite', label: '归类到我喜爱的' },
    { key: 'trash', label: '移动到回收站' },
  ]
})

const folderRuleCustomTagGroups = computed<FolderRuleTagGroup[]>(() => {
  const normalizeTags = (tags: string[]) =>
    Array.from(new Set(tags.map((tag) => tag.trim()).filter(Boolean))).sort((a, b) => a.localeCompare(b, 'zh-CN'))
  const groups: FolderRuleTagGroup[] = []
  for (const folder of tagManagerFolders.value) {
    const tags = normalizeTags(folder.tags)
    if (tags.length === 0) continue
    groups.push({
      key: `folder:${folder.id}`,
      title: folder.name,
      tags,
    })
  }
  const unclassified = normalizeTags(tagManagerUnclassifiedTags.value)
  if (unclassified.length > 0) {
    groups.push({
      key: 'unclassified',
      title: '未分类标签',
      tags: unclassified,
    })
  }
  return groups
})

function clearGalleryBatchSelection() {
  galleryBatchSelectedImageIds.value = []
}

function closeBatchFolderPickerModal() {
  batchFolderPickerModal.value = null
  batchFolderPickerTargetId.value = null
}

function closeBatchTagModal() {
  batchTagModalOpen.value = false
  batchTagDraft.value = ''
  batchTagSuggestions.value = []
  batchTagPending.value = []
  batchTagCustomConflict.value = null
  batchTagExpandedFolderIds.value = []
  batchTagSuggestLoading.value = false
  if (batchTagSuggestTimer.value !== null) {
    window.clearTimeout(batchTagSuggestTimer.value)
    batchTagSuggestTimer.value = null
  }
}

function enterGalleryBatchMode(seedImageId?: string) {
  isGalleryBatchMode.value = true
  if (seedImageId) {
    galleryBatchSelectedImageIds.value = [seedImageId]
    return
  }
  clearGalleryBatchSelection()
}

function exitGalleryBatchMode() {
  closeBatchFolderPickerModal()
  closeBatchTagModal()
  isGalleryBatchMode.value = false
  clearGalleryBatchSelection()
}

function toggleGalleryBatchImageSelection(imageId: string) {
  if (!isGalleryBatchMode.value) return
  const next = new Set(galleryBatchSelectedImageIds.value)
  if (next.has(imageId)) {
    next.delete(imageId)
  } else {
    next.add(imageId)
  }
  galleryBatchSelectedImageIds.value = [...next]
}

function appendGalleryBatchImageSelection(imageIds: string[]) {
  if (!isGalleryBatchMode.value || imageIds.length === 0) return
  const next = new Set(galleryBatchSelectedImageIds.value)
  for (const imageId of imageIds) {
    next.add(imageId)
  }
  galleryBatchSelectedImageIds.value = [...next]
}

function selectAllGalleryBatchImages() {
  if (!isGalleryBatchMode.value) return
  galleryBatchSelectedImageIds.value = visibleImages.value.map((image) => image.id)
}

function toggleSelectAllGalleryBatchImages() {
  if (!isGalleryBatchMode.value) return
  if (isGalleryBatchAllSelected.value) {
    clearGalleryBatchSelection()
    return
  }
  selectAllGalleryBatchImages()
}

async function runBatchAction(
  actionName: string,
  action: (imageId: string) => Promise<void>,
  options?: { clearSelection?: boolean; exitBatchMode?: boolean },
) {
  const imageIds = batchSelectedImageIds.value
  if (imageIds.length === 0) {
    errorText.value = '请先选择图片'
    return
  }
  isLoading.value = true
  let succeeded = 0
  let failed = 0
  let firstError = ''
  try {
    for (const imageId of imageIds) {
      try {
        await action(imageId)
        succeeded += 1
      } catch (error) {
        failed += 1
        if (!firstError) {
          firstError = formatError(error)
        }
      }
    }
  } finally {
    isLoading.value = false
  }

  if (failed > 0) {
    errorText.value = `${actionName}：成功 ${succeeded}，失败 ${failed}${firstError ? `，原因：${firstError}` : ''}`
  } else {
    errorText.value = ''
    statusText.value = `${actionName}完成：${succeeded} 张`
  }

  if (succeeded > 0 && options?.clearSelection !== false) {
    clearGalleryBatchSelection()
  }

  if (succeeded + failed > 0 && (options?.exitBatchMode ?? true)) {
    exitGalleryBatchMode()
  }
}

async function runBatchFavorite() {
  const { invoke } = await import('@tauri-apps/api/core')
  await runBatchAction('加入我喜爱的', async (imageId) => {
    library.value = await invoke<LibraryStore>('toggle_image_favorite_command', {
      imageId,
      favorite: true,
    })
  })
}

async function runBatchMoveToTrash() {
  const { invoke } = await import('@tauri-apps/api/core')
  await runBatchAction('移入回收站', async (imageId) => {
    library.value = await invoke<LibraryStore>('remove_image_from_index_command', {
      imageId,
    })
  })
}

async function runBatchRemoveFromCurrentFolder() {
  if (typeof activeUserFolderId.value !== 'number') return
  const folderId = activeUserFolderId.value
  const { invoke } = await import('@tauri-apps/api/core')
  await runBatchAction('从文件夹中移除', async (imageId) => {
    library.value = await invoke<LibraryStore>('remove_image_from_user_folder_command', {
      imageId,
      folderId,
    })
  })
}

function openBatchFolderPicker(mode: 'copy' | 'move' | 'assign') {
  batchFolderPickerTargetId.value = null
  if (mode === 'copy') {
    batchFolderPickerModal.value = { mode, title: '复制到文件夹', confirmLabel: '复制' }
    return
  }
  if (mode === 'move') {
    batchFolderPickerModal.value = { mode, title: '移动到文件夹', confirmLabel: '移动' }
    return
  }
  batchFolderPickerModal.value = { mode, title: '归类到文件夹', confirmLabel: '归类' }
}

function onBatchFolderRowClick(folder: { id: number; hasChildren: boolean; isExpanded: boolean }) {
  batchFolderPickerTargetId.value = folder.id
  if (!folder.hasChildren) return
  toggleFolderExpanded(folder.id)
}

function isBatchFolderTargetSelected(folderId: number) {
  return batchFolderPickerTargetId.value === folderId
}

async function confirmBatchFolderAction() {
  const modal = batchFolderPickerModal.value
  const targetFolderId = batchFolderPickerTargetId.value
  if (!modal || targetFolderId === null) return
  if (modal.mode !== 'assign' && typeof activeUserFolderId.value === 'number' && activeUserFolderId.value === targetFolderId) {
    errorText.value = '目标文件夹不能与当前文件夹相同'
    return
  }

  if (modal.mode === 'copy') {
    await runBatchAction('复制到文件夹', async (imageId) => {
      await assignImageToFolder(imageId, targetFolderId)
    })
    closeBatchFolderPickerModal()
    return
  }

  if (modal.mode === 'move') {
    const fromFolderId = typeof activeUserFolderId.value === 'number' ? activeUserFolderId.value : null
    const { invoke } = await import('@tauri-apps/api/core')
    await runBatchAction('移动到文件夹', async (imageId) => {
      await assignImageToFolder(imageId, targetFolderId)
      if (fromFolderId !== null && fromFolderId !== targetFolderId) {
        library.value = await invoke<LibraryStore>('remove_image_from_user_folder_command', {
          imageId,
          folderId: fromFolderId,
        })
      }
    })
    closeBatchFolderPickerModal()
    return
  }

  await runBatchAction('归类到文件夹', async (imageId) => {
    await assignImageToFolder(imageId, targetFolderId)
  })
  closeBatchFolderPickerModal()
}

async function refreshBatchTagSuggestionsNow() {
  const keyword = batchTagDraft.value.trim()
  if (!batchTagModalOpen.value || !keyword) {
    batchTagSuggestions.value = []
    batchTagSuggestLoading.value = false
    return
  }
  const token = batchTagSuggestToken.value + 1
  batchTagSuggestToken.value = token
  batchTagSuggestLoading.value = true
  try {
    const rows = await suggestKnownAutoTagsForInput(keyword, 30, { includeDictionary: true })
    if (token !== batchTagSuggestToken.value) return
    batchTagSuggestions.value = rows
  } catch (error) {
    if (token !== batchTagSuggestToken.value) return
    batchTagSuggestions.value = []
    errorText.value = formatError(error)
  } finally {
    if (token === batchTagSuggestToken.value) {
      batchTagSuggestLoading.value = false
    }
  }
}

function queueBatchTagSuggestions() {
  if (batchTagSuggestTimer.value !== null) {
    window.clearTimeout(batchTagSuggestTimer.value)
    batchTagSuggestTimer.value = null
  }
  batchTagSuggestTimer.value = window.setTimeout(() => {
    batchTagSuggestTimer.value = null
    void refreshBatchTagSuggestionsNow()
  }, 120)
}

async function openBatchTagModal() {
  batchTagModalOpen.value = true
  batchTagDraft.value = ''
  batchTagSuggestions.value = []
  batchTagPending.value = []
  batchTagExpandedFolderIds.value = []
  await reloadTagManagementState()
}

function isBatchPendingCustomTag(tagText: string) {
  const normalized = tagText.trim()
  if (!normalized) return false
  return batchTagPending.value.some((tag) => tag.kind === 'custom' && tag.tagText === normalized)
}

function isBatchPendingSupplementTag(tagEn: string) {
  const normalized = tagEn.trim().toLowerCase()
  if (!normalized) return false
  return batchTagPending.value.some((tag) => tag.kind === 'supplement' && tag.tagEn.toLowerCase() === normalized)
}

function addPendingCustomTag(tagText: string) {
  const normalized = tagText.trim()
  if (!normalized || isBatchPendingCustomTag(normalized)) return
  batchTagPending.value = [
    ...batchTagPending.value,
    {
      id: `custom:${normalized}`,
      kind: 'custom',
      tagText: normalized,
      label: normalized,
    },
  ]
}

function addPendingSupplementTag(tagEn: string, tagZh?: string | null) {
  const normalizedEn = tagEn.trim()
  if (!normalizedEn || isBatchPendingSupplementTag(normalizedEn)) return
  const normalizedZh = (tagZh ?? '').trim()
  batchTagPending.value = [
    ...batchTagPending.value,
    {
      id: `supplement:${normalizedEn.toLowerCase()}`,
      kind: 'supplement',
      tagEn: normalizedEn,
      tagZh: normalizedZh || null,
      label: normalizedZh || normalizedEn,
      subLabel: normalizedZh ? normalizedEn : '',
    },
  ]
}

function removePendingBatchTag(tagId: string) {
  batchTagPending.value = batchTagPending.value.filter((tag) => tag.id !== tagId)
}

async function addPendingCustomTagFromDraft() {
  const tagText = batchTagDraft.value.trim()
  if (!tagText) return
  try {
    const match = await findExactKnownAutoTag(tagText)
    if (match) {
      batchTagCustomConflict.value = {
        input: tagText,
        tagEn: match.tagEn,
        tagZh: match.tagZh ?? null,
      }
      return
    }
    addPendingCustomTag(tagText)
  } catch (error) {
    errorText.value = formatError(error)
  }
}

async function addPendingSupplementTagFromDraft() {
  const keyword = batchTagDraft.value.trim()
  if (!keyword) return
  try {
    const match = await findExactKnownAutoTag(keyword)
    if (!match) {
      errorText.value = '未找到对应自动标签，请从候选列表选择'
      return
    }
    addPendingSupplementTag(match.tagEn, match.tagZh ?? null)
  } catch (error) {
    errorText.value = formatError(error)
  }
}

function addPendingSupplementTagBySuggestion(suggestion: KnownAutoTagSuggestion) {
  addPendingSupplementTag(suggestion.tagEn, suggestion.tagZh ?? null)
}

function addPendingExistingTag(tagText: string) {
  addPendingCustomTag(tagText)
}

function isBatchTagFolderExpanded(folderId: number) {
  return batchTagExpandedFolderIds.value.includes(folderId)
}

function toggleBatchTagFolderExpanded(folderId: number) {
  if (isBatchTagFolderExpanded(folderId)) {
    batchTagExpandedFolderIds.value = batchTagExpandedFolderIds.value.filter((id) => id !== folderId)
    return
  }
  batchTagExpandedFolderIds.value = [...batchTagExpandedFolderIds.value, folderId]
}

function resolveBatchCustomTagConflict(action: 'supplement' | 'custom') {
  const conflict = batchTagCustomConflict.value
  if (!conflict) return
  if (action === 'supplement') {
    addPendingSupplementTag(conflict.tagEn, conflict.tagZh)
  } else {
    const input = conflict.input.trim()
    if (input) {
      const suffix = '（自定义）'
      const customText = input.endsWith(suffix) ? input : `${input}${suffix}`
      addPendingCustomTag(customText)
    }
  }
  batchTagCustomConflict.value = null
}

function isExistingTagPending(tagText: string) {
  return isBatchPendingCustomTag(tagText)
}

async function applyBatchPendingTags() {
  const pending = batchTagPending.value
  if (pending.length === 0) {
    errorText.value = '请先加入要添加的标签'
    return
  }
  await runBatchAction('批量添加标签', async (imageId) => {
    for (const tag of pending) {
      if (tag.kind === 'custom') {
        await addImageUserCustomTag(imageId, tag.tagText)
      } else {
        await addImageUserSupplementTag(imageId, tag.tagEn, tag.tagZh)
      }
    }
  })
}

function onGalleryBatchAction(actionKey: GalleryBatchActionItem['key']) {
  if (batchSelectedImageIds.value.length === 0) {
    errorText.value = '请先选择图片'
    return
  }
  if (actionKey === 'copy-folder') {
    openBatchFolderPicker('copy')
    return
  }
  if (actionKey === 'move-folder') {
    openBatchFolderPicker('move')
    return
  }
  if (actionKey === 'assign-folder') {
    openBatchFolderPicker('assign')
    return
  }
  if (actionKey === 'add-tags') {
    void openBatchTagModal()
    return
  }
  if (actionKey === 'remove-from-folder') {
    void runBatchRemoveFromCurrentFolder()
    return
  }
  if (actionKey === 'favorite') {
    void runBatchFavorite()
    return
  }
  void runBatchMoveToTrash()
}

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
const tagManagerDragGhostEl = ref<HTMLElement | null>(null)

function clearTagManagerDragGhost() {
  if (!tagManagerDragGhostEl.value) return
  tagManagerDragGhostEl.value.remove()
  tagManagerDragGhostEl.value = null
}

function startTagManagerTagDrag(tagText: string, event: DragEvent) {
  const normalized = tagText.trim()
  if (!normalized) return
  tagManagerDraggingTagText.value = normalized
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move'
    event.dataTransfer.setData('text/plain', normalized)

    clearTagManagerDragGhost()
    const ghost = document.createElement('span')
    ghost.className = 'tag-manager-modal__tag-chip-ghost'
    ghost.textContent = normalized
    ghost.style.position = 'fixed'
    ghost.style.left = '-9999px'
    ghost.style.top = '-9999px'
    document.body.appendChild(ghost)
    tagManagerDragGhostEl.value = ghost
    event.dataTransfer.setDragImage(ghost, Math.round(ghost.offsetWidth / 2), Math.round(ghost.offsetHeight / 2))
  }
}

function endTagManagerTagDrag() {
  clearTagManagerDragGhost()
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
  startScanAllFoldersCollectOnly,
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
  if (autoScanOnStartup.value) {
    void runStartupAutoScanPipeline()
  }
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
  await runOneClickScanPipeline(async () => await startAutoScanIfEnabled())
}

async function runOneClickScanPipeline(
  startCollectPhase: () => Promise<boolean>,
) {
  if (startupAutoScanPipelineRunning.value) return
  startupAutoScanPipelineRunning.value = true
  try {
    await refreshBackgroundScanStatus()
    if (isBackgroundScanRunning.value) {
      await stopScanAllFolders()
      await waitUntilIdle(refreshBackgroundScanStatus, () => isBackgroundScanRunning.value)
    }

    let started = await startCollectPhase()
    if (!started) {
      await waitUntilIdle(refreshBackgroundScanStatus, () => isBackgroundScanRunning.value)
      started = await startCollectPhase()
      if (!started) return
    }

    await waitUntilIdle(refreshBackgroundScanStatus, () => isBackgroundScanRunning.value)
    await startThumbnailGeneration()
    await waitUntilIdle(refreshThumbnailGenerationStatus, () => isThumbnailGenerationRunning.value)

    await startNaturalLanguageScan()
    await waitUntilIdle(refreshNaturalLanguageScanStatus, () => isNaturalLanguageScanRunning.value)

    await startAtmosphereGeneration()
    await waitUntilIdle(refreshAtmosphereGenerationStatus, () => isAtmosphereGenerationRunning.value)

    await startColorSignatureGeneration()
    await waitUntilIdle(refreshColorSignatureGenerationStatus, () => isColorSignatureGenerationRunning.value)

    await startScanAllFolders()
    await waitUntilIdle(refreshBackgroundScanStatus, () => isBackgroundScanRunning.value)
  } finally {
    startupAutoScanPipelineRunning.value = false
  }
}

async function runOneClickScan() {
  await runOneClickScanPipeline(async () => await startScanAllFoldersCollectOnly())
}

onUnmounted(() => {
  clearTagManagerDragGhost()
  clearSidebarHoverCloseTimer()
  clearRightSidebarHoverCloseTimer()
  closeFolderRuleDanbooruSuggestions()
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

watch(batchTagDraft, () => {
  if (!batchTagModalOpen.value) return
  queueBatchTagSuggestions()
})

watch(
  [viewMode, activeUserFolderId],
  async ([nextViewMode, nextFolderId], [prevViewMode, prevFolderId]) => {
    if (
      isGalleryBatchMode.value &&
      (prevViewMode !== nextViewMode || prevFolderId !== nextFolderId || nextViewMode !== 'gallery')
    ) {
      exitGalleryBatchMode()
    }

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
      await addFolderByPath(selected)
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
  await addFolderByPath(folderPathInput.value)
}

async function addFolderByPath(rawFolderPath: string) {
  if (isAddingFolder.value) return
  errorText.value = ''
  const normalizedPath = rawFolderPath.trim()
  if (normalizedPath.length === 0) {
    errorText.value = '请输入图库文件夹路径'
    return
  }

  isAddingFolder.value = true
  statusText.value = '正在扫描图库...'

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('add_gallery_folder_command', {
      folderPath: normalizedPath,
    })
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

function openRemoveFolderConfirm(folderPath: string) {
  removeFolderConfirmPath.value = folderPath
}

function closeRemoveFolderConfirm() {
  removeFolderConfirmPath.value = null
}

function closeSystemTrashMoveErrorDialog() {
  systemTrashMoveErrorMessage.value = null
}

async function confirmRemoveFolder() {
  const folderPath = removeFolderConfirmPath.value
  if (!folderPath) return
  removeFolderConfirmPath.value = null
  await removeFolder(folderPath)
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

function guessImageMimeTypeFromName(fileName: string) {
  const ext = fileName.split('.').pop()?.toLowerCase() ?? ''
  if (ext === 'jpg' || ext === 'jpeg') return 'image/jpeg'
  if (ext === 'png') return 'image/png'
  if (ext === 'webp') return 'image/webp'
  if (ext === 'gif') return 'image/gif'
  if (ext === 'bmp') return 'image/bmp'
  if (ext === 'avif') return 'image/avif'
  if (ext === 'heic') return 'image/heic'
  if (ext === 'heif') return 'image/heif'
  return 'application/octet-stream'
}

function referenceBoardWorldPointFromClient(clientX: number, clientY: number, container: HTMLElement) {
  const rect = container.getBoundingClientRect()
  return {
    x: (clientX - rect.left - boardPan.value.x) / boardScale.value,
    y: (clientY - rect.top - boardPan.value.y) / boardScale.value,
  }
}

function onReferenceBoardExternalImageDragOver(event: DragEvent) {
  if (!activeReferenceBoard.value) return
  const files = Array.from(event.dataTransfer?.files ?? [])
  const hasImage = files.some((file) => file.type.startsWith('image/'))
  if (!hasImage) return
  if (event.dataTransfer) {
    event.dataTransfer.dropEffect = 'copy'
  }
}

async function onReferenceBoardExternalImageDrop(event: DragEvent) {
  if (!activeReferenceBoard.value) return
  const container = event.currentTarget as HTMLElement | null
  if (!container) return
  const files = Array.from(event.dataTransfer?.files ?? [])
  const imageFile = files.find((file) => file.type.startsWith('image/'))
  if (!imageFile) return

  try {
    const mimeType =
      imageFile.type && imageFile.type.startsWith('image/')
        ? imageFile.type
        : guessImageMimeTypeFromName(imageFile.name)
    if (!mimeType.startsWith('image/')) {
      errorText.value = '仅支持拖入图片文件'
      return
    }
    const bytes = Array.from(new Uint8Array(await imageFile.arrayBuffer()))
    const world = referenceBoardWorldPointFromClient(event.clientX, event.clientY, container)
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('paste_image_to_reference_board_command', {
      boardId: activeReferenceBoard.value.id,
      imageBytes: bytes,
      mimeType,
      x: world.x,
      y: world.y,
    })
    ensureBoardCanvasBoundsFor(activeReferenceBoard.value.id)
  } catch (error) {
    errorText.value = formatError(error)
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
  clearRightSidebarHoverCloseTimer()
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
  clearRightSidebarHoverCloseTimer()
  rightSidebarHoverCloseTimer.value = window.setTimeout(() => {
    rightSidebarHoverCloseTimer.value = null
    if (
      !rightSidebarPinned.value &&
      !boardDraft.value &&
      !isComposingBoardName.value &&
      renamingReferenceBoardFolderId.value === null &&
      renamingReferenceBoardId.value === null &&
      !isComposingReferenceBoardFolderRename.value &&
      !isComposingReferenceBoardRename.value &&
      draggedReferenceBoardId.value === null &&
      draggedReferenceBoardFolderId.value === null &&
      !isRightSidebarHoverSafeAreaActive()
    ) {
      rightSidebarHoverOpen.value = false
      closeBoardContextMenu()
    }
  }, 90)
}

function clearRightSidebarHoverCloseTimer() {
  if (rightSidebarHoverCloseTimer.value === null) return
  window.clearTimeout(rightSidebarHoverCloseTimer.value)
  rightSidebarHoverCloseTimer.value = null
}

function isRightSidebarHoverSafeAreaActive() {
  const sidebar = document.querySelector('.right-sidebar')
  if (sidebar instanceof HTMLElement && sidebar.matches(':hover')) {
    return true
  }
  const hotspot = document.querySelector('.right-sidebar-hotspot')
  if (hotspot instanceof HTMLElement && hotspot.matches(':hover')) {
    return true
  }
  return false
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

function canEditFolderRule(folderId: number) {
  const folder = folderTree.value.find((item) => item.id === folderId)
  if (!folder) return false
  return !folder.hasChildren
}

function createEmptyFolderRuleCondition(): FolderRuleConditionDraft {
  const id = folderRuleSeed.value++
  return {
    id,
    logic: 'AND',
    source: 'danbooru',
    keyword: '',
  }
}

function clearFolderRuleDanbooruSuggestionTimer() {
  if (folderRuleDanbooruSuggestTimer.value === null) return
  window.clearTimeout(folderRuleDanbooruSuggestTimer.value)
  folderRuleDanbooruSuggestTimer.value = null
}

function closeFolderRuleDanbooruSuggestions() {
  clearFolderRuleDanbooruSuggestionTimer()
  folderRuleDanbooruActiveConditionId.value = null
  folderRuleDanbooruSuggestLoading.value = false
  folderRuleDanbooruSuggestions.value = []
}

async function refreshFolderRuleDanbooruSuggestions(conditionId: number, keyword: string) {
  const normalized = keyword.trim()
  if (!normalized) {
    folderRuleDanbooruSuggestions.value = []
    folderRuleDanbooruSuggestLoading.value = false
    return
  }
  const token = folderRuleDanbooruSuggestToken.value + 1
  folderRuleDanbooruSuggestToken.value = token
  folderRuleDanbooruSuggestLoading.value = true
  try {
    const rows = await suggestKnownAutoTagsForInput(normalized, 24, { includeDictionary: true })
    if (token !== folderRuleDanbooruSuggestToken.value) return
    if (folderRuleDanbooruActiveConditionId.value !== conditionId) return
    folderRuleDanbooruSuggestions.value = rows
  } catch (error) {
    if (token !== folderRuleDanbooruSuggestToken.value) return
    folderRuleDanbooruSuggestions.value = []
    errorText.value = formatError(error)
  } finally {
    if (token === folderRuleDanbooruSuggestToken.value) {
      folderRuleDanbooruSuggestLoading.value = false
    }
  }
}

function queueFolderRuleDanbooruSuggestions(conditionId: number, keyword: string) {
  folderRuleDanbooruActiveConditionId.value = conditionId
  clearFolderRuleDanbooruSuggestionTimer()
  const normalized = keyword.trim()
  if (!normalized) {
    folderRuleDanbooruSuggestions.value = []
    folderRuleDanbooruSuggestLoading.value = false
    return
  }
  folderRuleDanbooruSuggestTimer.value = window.setTimeout(() => {
    folderRuleDanbooruSuggestTimer.value = null
    void refreshFolderRuleDanbooruSuggestions(conditionId, normalized)
  }, 120)
}

function onFolderRuleConditionSourceChange(condition: FolderRuleConditionDraft) {
  condition.keyword = ''
  if (condition.source !== 'danbooru' && folderRuleDanbooruActiveConditionId.value === condition.id) {
    closeFolderRuleDanbooruSuggestions()
  }
}

function onFolderRuleDanbooruKeywordInput(condition: FolderRuleConditionDraft, value: string) {
  condition.keyword = value.trim()
  queueFolderRuleDanbooruSuggestions(condition.id, condition.keyword)
}

function focusFolderRuleDanbooruCondition(condition: FolderRuleConditionDraft) {
  folderRuleDanbooruActiveConditionId.value = condition.id
  queueFolderRuleDanbooruSuggestions(condition.id, condition.keyword)
}

function scheduleCloseFolderRuleDanbooruSuggestions() {
  window.setTimeout(() => {
    if (!folderRuleEditor.value) return
    closeFolderRuleDanbooruSuggestions()
  }, 120)
}

function selectFolderRuleDanbooruSuggestion(condition: FolderRuleConditionDraft, suggestion: KnownAutoTagSuggestion) {
  condition.keyword = suggestion.tagEn
  closeFolderRuleDanbooruSuggestions()
}

function addFolderRuleCondition() {
  if (!folderRuleEditor.value) return
  folderRuleEditor.value = {
    ...folderRuleEditor.value,
    conditions: [
      ...folderRuleEditor.value.conditions,
      createEmptyFolderRuleCondition(),
    ],
  }
}

function removeFolderRuleCondition(conditionId: number) {
  if (!folderRuleEditor.value) return
  folderRuleEditor.value = {
    ...folderRuleEditor.value,
    conditions: folderRuleEditor.value.conditions.filter((item) => item.id !== conditionId),
  }
  if (folderRuleDanbooruActiveConditionId.value === conditionId) {
    closeFolderRuleDanbooruSuggestions()
  }
}

function closeFolderRuleEditor() {
  closeFolderRuleDanbooruSuggestions()
  folderRuleEditor.value = null
}

async function openFolderRuleEditor(folderId: number) {
  if (!canEditFolderRule(folderId)) {
    errorText.value = '仅最小层级文件夹支持编辑规则'
    closeFolderContextMenu()
    return
  }
  try {
    await reloadTagManagementState()
    const folder = folderTree.value.find((item) => item.id === folderId)
    if (!folder) return
    const { invoke } = await import('@tauri-apps/api/core')
    const raw = await invoke<Record<string, unknown> | null>('get_user_folder_rule_command', {
      folderId,
    })
    const loadedConditionsRaw = Array.isArray(raw?.conditions) ? raw?.conditions : []
    const loadedConditions = loadedConditionsRaw
      .map((item) => {
        const logicRaw = String((item as Record<string, unknown>).logic ?? 'AND').toUpperCase()
        const sourceRaw = String((item as Record<string, unknown>).source ?? 'danbooru').toLowerCase()
        const keyword = String((item as Record<string, unknown>).keyword ?? '').trim()
        const logic = logicRaw === 'OR' || logicRaw === 'NOT' ? logicRaw : 'AND'
        const source = sourceRaw === 'custom' || sourceRaw === 'filename' ? sourceRaw : 'danbooru'
        if (!keyword) return null
        return {
          id: folderRuleSeed.value++,
          logic: logic as FolderRuleConditionDraft['logic'],
          source: source as FolderRuleConditionDraft['source'],
          keyword,
        }
      })
      .filter((item): item is FolderRuleConditionDraft => Boolean(item))
    folderRuleEditor.value = {
      folderId,
      folderName: folder.name,
      conditions: loadedConditions.length > 0 ? loadedConditions : [createEmptyFolderRuleCondition()],
    }
    closeFolderContextMenu()
  } catch (error) {
    errorText.value = formatError(error)
  }
}

async function saveFolderRuleDraft(applyNow: boolean) {
  if (!folderRuleEditor.value) return
  try {
    const payload = folderRuleEditor.value.conditions.map((condition) => ({
      logic: condition.logic,
      source: condition.source,
      keyword: condition.keyword.trim(),
    }))
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('save_user_folder_rule_command', {
      folderId: folderRuleEditor.value.folderId,
      conditions: payload,
      applyNow,
    })
    statusText.value = applyNow
      ? `已保存规则并立即应用：${folderRuleEditor.value.folderName}`
      : `已保存规则：${folderRuleEditor.value.folderName}`
    closeFolderRuleEditor()
  } catch (error) {
    errorText.value = formatError(error)
  }
}

async function deleteFolderRuleDraft() {
  if (!folderRuleEditor.value) return
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('save_user_folder_rule_command', {
      folderId: folderRuleEditor.value.folderId,
      conditions: [],
      applyNow: false,
    })
    statusText.value = `已删除规则：${folderRuleEditor.value.folderName}`
    closeFolderRuleEditor()
  } catch (error) {
    errorText.value = formatError(error)
  }
}

function openGalleryImageMenu(item: GalleryLayoutItem, event: MouseEvent) {
  openGalleryImageMenuState(item, event, closeReferenceBoardCanvasMenu)
}

function openGalleryImageDetailFromGallery(item: GalleryLayoutItem) {
  if (galleryImageContextMenu.value) {
    closeGalleryImageContextMenu()
    return
  }
  openGalleryImageDetail(item)
}

function openGalleryBatchModeFromContextMenu(imageId: string) {
  enterGalleryBatchMode(imageId)
  closeGalleryImageContextMenu()
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

async function moveGalleryImageToSystemTrash(imageId: string) {
  systemTrashMoveErrorMessage.value = null
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('move_image_to_system_trash_command', {
      imageId,
    })
    if (activeImageDetailId.value === imageId) {
      closeImageDetail()
    }
  } catch (error) {
    const message = formatError(error)
    errorText.value = message
    systemTrashMoveErrorMessage.value = message
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
  runOneClickScan,
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
  removeFolder: openRemoveFolderConfirm,
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
  canEditFolderRule,
  openFolderRuleEditor,
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
  onReferenceBoardExternalImageDragOver,
  onReferenceBoardExternalImageDrop,
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
  openGalleryImageDetail: openGalleryImageDetailFromGallery,
  openGalleryImageMenu,
  exitGalleryBatchMode,
  toggleSelectAllGalleryBatchImages,
  onGalleryBatchAction,
  toggleGalleryBatchImageSelection,
  appendGalleryBatchImageSelection,
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
      'is-settings-view': isSettingsView,
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
            :is-one-click-scan-running="startupAutoScanPipelineRunning"
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
            :is-batch-mode="isGalleryBatchMode"
            :is-batch-all-selected="isGalleryBatchAllSelected"
            :batch-selected-image-ids="galleryBatchSelectedImageIds"
            :batch-action-labels="galleryBatchActionLabels"
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

    <div v-if="removeFolderConfirmPath" class="image-detail-modal__dialog-layer" @click="closeRemoveFolderConfirm()">
      <article class="image-detail-modal__dialog" @click.stop>
        <h4>确认移除索引</h4>
        <p class="image-detail-modal__dialog-text">该操作不会删除本地文件，仅移除该图库文件夹的索引。</p>
        <p class="image-detail-modal__dialog-text">{{ removeFolderConfirmPath }}</p>
        <div class="image-detail-modal__dialog-actions">
          <button type="button" class="secondary-button" @click="closeRemoveFolderConfirm()">取消</button>
          <button type="button" class="danger-button" @click="confirmRemoveFolder()">确认移除</button>
        </div>
      </article>
    </div>

    <div
      v-if="systemTrashMoveErrorMessage"
      class="image-detail-modal__dialog-layer"
      @click="closeSystemTrashMoveErrorDialog()"
    >
      <article class="image-detail-modal__dialog" @click.stop>
        <h4>移动到系统回收站失败</h4>
        <p class="image-detail-modal__dialog-text">{{ systemTrashMoveErrorMessage }}</p>
        <div class="image-detail-modal__dialog-actions">
          <button type="button" class="primary-button" @click="closeSystemTrashMoveErrorDialog()">知道了</button>
        </div>
      </article>
    </div>

    <div v-if="folderRuleEditor" class="folder-rule-editor-layer" @click="closeFolderRuleEditor()">
      <article class="folder-rule-editor" @click.stop>
        <header class="folder-rule-editor__header">
          <h3>编辑规则 · {{ folderRuleEditor.folderName }}</h3>
          <button type="button" class="folder-rule-editor__close" @click="closeFolderRuleEditor()">×</button>
        </header>
        <div class="folder-rule-editor__hint">为文件夹创建规则:满足条件的图片将自动加入这个文件夹</div>
        <div class="folder-rule-editor__body">
          <section class="folder-rule-editor__section">
            <div class="folder-rule-editor__section-title">条件列表</div>
            <div class="folder-rule-editor__conditions">
              <div
                v-for="condition in folderRuleEditor.conditions"
                :key="`folder-rule:${condition.id}`"
                class="folder-rule-editor__condition-item"
              >
                <div class="folder-rule-editor__condition-row">
                  <select v-model="condition.logic" class="folder-rule-editor__select">
                    <option value="AND">AND</option>
                    <option value="OR">OR</option>
                    <option value="NOT">NOT</option>
                  </select>
                  <select
                    v-model="condition.source"
                    class="folder-rule-editor__select"
                    @change="onFolderRuleConditionSourceChange(condition)"
                  >
                    <option value="danbooru">自动标签</option>
                    <option value="custom">自定义标签</option>
                    <option value="filename">文件名</option>
                  </select>
                  <template v-if="condition.source === 'custom'">
                    <select v-model="condition.keyword" class="folder-rule-editor__select folder-rule-editor__select--keyword">
                      <option value="">选择已有自定义标签</option>
                      <optgroup v-for="group in folderRuleCustomTagGroups" :key="`folder-rule-custom-group:${group.key}`" :label="group.title">
                        <option
                          v-for="tagText in group.tags"
                          :key="`folder-rule-custom-tag:${group.key}:${tagText}`"
                          :value="tagText"
                        >
                          {{ tagText }}
                        </option>
                      </optgroup>
                    </select>
                  </template>
                  <input
                    v-else
                    :value="condition.keyword"
                    class="folder-rule-editor__input"
                    type="text"
                    :placeholder="
                      condition.source === 'filename'
                        ? '输入文件名关键词'
                        : '输入自动标签关键词（支持联想）'
                    "
                    @focus="condition.source === 'danbooru' ? focusFolderRuleDanbooruCondition(condition) : null"
                    @blur="condition.source === 'danbooru' ? scheduleCloseFolderRuleDanbooruSuggestions() : null"
                    @input="
                      condition.source === 'danbooru'
                        ? onFolderRuleDanbooruKeywordInput(condition, ($event.target as HTMLInputElement).value)
                        : (condition.keyword = ($event.target as HTMLInputElement).value.trim())
                    "
                  />
                  <button
                    type="button"
                    class="folder-rule-editor__remove"
                    :disabled="folderRuleEditor.conditions.length <= 1"
                    @click="removeFolderRuleCondition(condition.id)"
                  >
                    删除
                  </button>
                </div>
                <div
                  v-if="condition.source === 'danbooru' && folderRuleDanbooruActiveConditionId === condition.id"
                  class="folder-rule-editor__suggestions"
                >
                  <button
                    v-for="item in folderRuleDanbooruSuggestions"
                    :key="`folder-rule-suggestion:${condition.id}:${item.tagEn}`"
                    type="button"
                    class="folder-rule-editor__suggestion"
                    @mousedown.prevent="selectFolderRuleDanbooruSuggestion(condition, item)"
                  >
                    <span>{{ item.tagZh || item.tagEn }}</span>
                    <small>{{ item.tagZh ? item.tagEn : '' }}</small>
                  </button>
                  <p v-if="folderRuleDanbooruSuggestLoading" class="folder-rule-editor__placeholder">搜索中...</p>
                  <p
                    v-else-if="condition.keyword && folderRuleDanbooruSuggestions.length === 0"
                    class="folder-rule-editor__placeholder"
                  >
                    暂无匹配标签
                  </p>
                </div>
              </div>
            </div>
            <button type="button" class="secondary-button folder-rule-editor__add" @click="addFolderRuleCondition()">
              新增条件
            </button>
          </section>
        </div>
        <footer class="folder-rule-editor__footer">
          <button type="button" class="secondary-button folder-rule-editor__delete" @click="deleteFolderRuleDraft()">
            删除规则
          </button>
          <div class="folder-rule-editor__footer-actions">
            <button type="button" class="secondary-button" @click="saveFolderRuleDraft(false)">保存规则</button>
            <button type="button" class="primary-button" @click="saveFolderRuleDraft(true)">保存并立即应用</button>
          </div>
        </footer>
      </article>
    </div>

    <div v-if="batchFolderPickerModal" class="batch-action-layer">
      <article class="batch-action-modal" @click.stop>
        <header class="batch-action-modal__header">
          <h3>{{ batchFolderPickerModal.title }}</h3>
          <button type="button" class="batch-action-modal__close" @click="closeBatchFolderPickerModal()">×</button>
        </header>
        <div class="batch-action-modal__body">
          <div class="batch-action-modal__hint">已选 {{ batchSelectedImageIds.length }} 张图片</div>
          <div class="batch-action-modal__folder-list">
            <div
              v-for="folder in folderTree"
              :key="`batch-folder:${folder.id}`"
              class="folder-tree__row"
              :class="{ 'is-active': isBatchFolderTargetSelected(folder.id) }"
              :style="{ paddingLeft: `${8 + folder.depth * 16}px` }"
              @click="onBatchFolderRowClick(folder)"
            >
              <div class="folder-tree__content">
                <component
                  :is="
                    folder.hasChildren ? (folder.isExpanded ? FolderOpen : FolderClose) : isBatchFolderTargetSelected(folder.id) ? FolderOpen : FolderClose
                  "
                  class="folder-tree__folder-icon"
                  theme="outline"
                  :size="16"
                  :stroke-width="3"
                  :fill="['currentColor']"
                  aria-hidden="true"
                />
                <button class="folder-tree__item" type="button">
                  <span class="folder-tree__item-label">{{ folder.name }}</span>
                </button>
              </div>
            </div>
          </div>
        </div>
        <footer class="batch-action-modal__footer">
          <button type="button" class="secondary-button" @click="closeBatchFolderPickerModal()">取消</button>
          <button
            type="button"
            class="primary-button"
            :disabled="batchFolderPickerTargetId === null"
            @click="confirmBatchFolderAction()"
          >
            {{ batchFolderPickerModal.confirmLabel }}
          </button>
        </footer>
      </article>
    </div>

    <div v-if="batchTagModalOpen" class="batch-action-layer">
      <article class="batch-action-modal batch-action-modal--tags" @click.stop>
        <header class="batch-action-modal__header">
          <h3>批量添加标签</h3>
          <button type="button" class="batch-action-modal__close" @click="closeBatchTagModal()">×</button>
        </header>
        <div class="batch-action-modal__body">
          <div class="batch-action-modal__hint">已选 {{ batchSelectedImageIds.length }} 张图片</div>
          <div class="batch-action-modal__pending">
            <div class="batch-action-modal__pending-title">待添加标签</div>
            <div class="batch-action-modal__pending-list">
              <span
                v-for="tag in batchTagPending"
                :key="`batch-pending-tag:${tag.id}`"
                class="gallery-search__chip batch-action-modal__pending-chip"
              >
                <span class="gallery-search__chip-text">{{ tag.label }}</span>
                <small v-if="tag.subLabel" class="batch-action-modal__pending-sub">{{ tag.subLabel }}</small>
                <button type="button" class="gallery-search__chip-remove" @click.stop="removePendingBatchTag(tag.id)">×</button>
              </span>
              <p v-if="batchTagPending.length === 0" class="batch-action-modal__placeholder">暂未添加标签</p>
            </div>
          </div>
          <input
            v-model.trim="batchTagDraft"
            class="batch-action-modal__input"
            type="text"
            placeholder="创建一个新标签或搜索自动标签"
            autocomplete="off"
            @keydown.enter.prevent="void addPendingCustomTagFromDraft()"
          />
          <div class="batch-action-modal__tag-actions">
            <button type="button" class="secondary-button" :disabled="!batchTagDraft" @click="void addPendingCustomTagFromDraft()">
              创建标签
            </button>
          </div>
          <div class="batch-action-modal__existing-tags">
            <div class="batch-action-modal__pending-title">已有标签</div>
            <section class="image-detail-modal__existing-tags-picker batch-action-modal__existing-tags-picker">
              <div
                v-if="tagManagerFolders.length > 0 || tagManagerUnclassifiedTags.length > 0"
                class="image-detail-modal__existing-folder-list"
              >
                <div
                  v-for="folder in tagManagerFolders"
                  :key="`batch-custom-folder:${folder.id}`"
                  class="image-detail-modal__existing-folder"
                >
                  <button
                    type="button"
                    class="image-detail-modal__existing-folder-toggle"
                    @click="toggleBatchTagFolderExpanded(folder.id)"
                  >
                    <span class="image-detail-modal__existing-folder-caret">
                      {{ isBatchTagFolderExpanded(folder.id) ? '▾' : '▸' }}
                    </span>
                    <span class="image-detail-modal__existing-folder-name">{{ folder.name }}</span>
                    <small>{{ folder.tags.length }}</small>
                  </button>
                  <div v-if="isBatchTagFolderExpanded(folder.id)" class="image-detail-modal__existing-tag-list">
                    <button
                      v-for="tagText in folder.tags"
                      :key="`batch-custom-folder-tag:${folder.id}:${tagText}`"
                      type="button"
                      class="gallery-search__chip image-detail-modal__existing-tag-chip"
                      :class="{ 'is-selected': isExistingTagPending(tagText) }"
                      @click="addPendingExistingTag(tagText)"
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
                    @click="toggleBatchTagFolderExpanded(-1)"
                  >
                    <span class="image-detail-modal__existing-folder-caret">
                      {{ isBatchTagFolderExpanded(-1) ? '▾' : '▸' }}
                    </span>
                    <span class="image-detail-modal__existing-folder-name">未分类标签</span>
                    <small>{{ tagManagerUnclassifiedTags.length }}</small>
                  </button>
                  <div v-if="isBatchTagFolderExpanded(-1)" class="image-detail-modal__existing-tag-list">
                    <button
                      v-for="tagText in tagManagerUnclassifiedTags"
                      :key="`batch-custom-unclassified-tag:${tagText}`"
                      type="button"
                      class="gallery-search__chip image-detail-modal__existing-tag-chip"
                      :class="{ 'is-selected': isExistingTagPending(tagText) }"
                      @click="addPendingExistingTag(tagText)"
                    >
                      <span class="gallery-search__chip-text">{{ tagText }}</span>
                    </button>
                  </div>
                </div>
              </div>
              <p v-else class="image-detail-modal__dialog-empty">暂无可选标签</p>
            </section>
          </div>
          <div class="batch-action-modal__tag-list">
            <button
              v-for="item in batchTagSuggestions"
              :key="`batch-tag-suggestion:${item.tagEn}`"
              type="button"
              class="batch-action-modal__tag-option"
              @click="addPendingSupplementTagBySuggestion(item)"
            >
              <span class="batch-action-modal__tag-main">{{ item.tagZh || item.tagEn }}</span>
              <small class="batch-action-modal__tag-sub">{{ item.tagZh ? item.tagEn : '' }}</small>
            </button>
            <p v-if="batchTagSuggestLoading" class="batch-action-modal__placeholder">搜索中...</p>
            <p v-else-if="batchTagDraft && batchTagSuggestions.length === 0" class="batch-action-modal__placeholder">暂无匹配标签</p>
          </div>
        </div>
        <footer class="batch-action-modal__footer">
          <button type="button" class="secondary-button" @click="closeBatchTagModal()">取消</button>
          <button type="button" class="primary-button" :disabled="batchTagPending.length === 0" @click="applyBatchPendingTags()">
            添加以上标签
          </button>
        </footer>
        <div v-if="batchTagCustomConflict" class="image-detail-modal__dialog-layer batch-action-modal__conflict-layer" @click.stop>
          <article class="image-detail-modal__dialog" @click.stop>
            <h4>标签已存在</h4>
            <p class="image-detail-modal__dialog-text">
              已有此自动标签（{{ batchTagCustomConflict.tagZh || batchTagCustomConflict.tagEn }}），进行补充还是新建用户自定义标签？
            </p>
            <div class="image-detail-modal__dialog-actions">
              <button type="button" class="secondary-button" @click="batchTagCustomConflict = null">取消</button>
              <button type="button" class="secondary-button" @click="resolveBatchCustomTagConflict('supplement')">补充</button>
              <button type="button" class="primary-button" @click="resolveBatchCustomTagConflict('custom')">自定义</button>
            </div>
          </article>
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
      <button type="button" @click="openGalleryBatchModeFromContextMenu(galleryImageContextMenu.imageId)">批量操作</button>
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
      <template v-else-if="activeUserFolderId === 'trash'">
        <button type="button" @click="restoreGalleryImageFromTrash(galleryImageContextMenu.imageId)">
          还原
        </button>
        <button
          class="is-danger"
          type="button"
          @click="moveGalleryImageToSystemTrash(galleryImageContextMenu.imageId)"
        >
          移动到系统回收站
        </button>
      </template>
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
