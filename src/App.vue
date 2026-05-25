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
import { useFolderManagement } from './composables/useFolderManagement'
import { useGallerySearch } from './composables/useGallerySearch'
import { useReferenceBoardInteraction } from './composables/useReferenceBoardInteraction'
import { useReferenceBoardClipboard } from './composables/useReferenceBoardClipboard'
import type { GalleryImage, GalleryLayoutItem } from './types/gallery'

type LibraryFolder = {
  id: number
  path: string
  addedAt: number
  lastScannedAt?: number | null
}

type UserFolder = {
  id: number
  parentId?: number | null
  name: string
  sortOrder: number
  createdAt: number
  updatedAt: number
}

type ImageFolderAssignment = {
  imageId: string
  folderId: number
}

type ReferenceBoardFolder = {
  id: number
  name: string
  sortOrder: number
  createdAt: number
  updatedAt: number
}

type ReferenceBoard = {
  id: number
  folderId?: number | null
  name: string
  sortOrder: number
  createdAt: number
  updatedAt: number
}

type ReferenceBoardItem = {
  id: number
  boardId: number
  imageId: string
  x: number
  y: number
  width: number
  height: number
  rotation: number
  zIndex: number
  createdAt: number
}

type LibraryStore = {
  folders: LibraryFolder[]
  images: GalleryImage[]
  userFolders: UserFolder[]
  imageFolders: ImageFolderAssignment[]
  referenceBoardFolders: ReferenceBoardFolder[]
  referenceBoards: ReferenceBoard[]
  referenceBoardItems: ReferenceBoardItem[]
}

type ViewMode = 'gallery' | 'settings' | 'board'

type DragState = {
  imageId: string
  thumbnailUrl: string
  x: number
  y: number
  panelX: number
  panelY: number
  overFolderId: number | null
  overBoardId: number | null
  overRightSidebar: boolean
}

type ReferenceBoardRow =
  | { kind: 'folder'; id: number; name: string; hasBoards: boolean; isExpanded: boolean }
  | { kind: 'board'; id: number; folderId: number | null; name: string; depth: number }

type BoardContextMenu =
  | { kind: 'space'; folderId: number | null; x: number; y: number }
  | { kind: 'folder'; folderId: number; x: number; y: number }
  | { kind: 'board'; boardId: number; x: number; y: number }

type BoardDraft = {
  kind: 'board' | 'folder'
  folderId: number | null
  x: number
  y: number
}

type ImageDetailContextMenu = { x: number; y: number } | null
type GalleryImageContextMenu = { imageId: string; x: number; y: number } | null
type PreviewBoardDragKind = 'preview' | 'board' | 'gallery' | null

type PreviewBoardItemDragState = {
  itemId: number
  imageId: string
  sourceBoardId: number
  thumbnailUrl: string
  x: number
  y: number
  targetBoardId: number | null
  targetKind: PreviewBoardDragKind
  mode: 'copy' | 'move'
}

type BoardCanvasBounds = {
  minX: number
  minY: number
  maxX: number
  maxY: number
}

type BoardItemLayout = {
  x: number
  y: number
  width: number
  height: number
  rotation: number
}

type BoardHistoryChange = {
  itemId: number
  before: BoardItemLayout
  after: BoardItemLayout
}

type DeletedBoardItemSnapshot = {
  itemId: number
  boardId: number
  imageId: string
  layout: BoardItemLayout
  zIndex: number
}

type BoardLayoutHistoryEntry = {
  kind: 'layout'
  boardId: number
  changes: BoardHistoryChange[]
  selectionBefore: number | null
  selectionAfter: number | null
}

type BoardDeleteHistoryEntry = {
  kind: 'delete'
  boardId: number
  deletedItems: DeletedBoardItemSnapshot[]
  restoredItemIds: number[]
  selectionBefore: number | null
  selectionAfter: number | null
}

type BoardHistoryEntry = BoardLayoutHistoryEntry | BoardDeleteHistoryEntry

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
const statusText = ref('还没有添加图库文件夹')
const errorText = ref('')
const galleryEl = ref<HTMLElement | null>(null)
const viewportWidth = ref(960)
const viewportHeight = ref(720)
const folderPathInput = ref('')
const longPressTimer = ref<number | null>(null)
const dragState = ref<DragState | null>(null)
const expandedReferenceBoardFolderIds = ref<Set<number>>(new Set())
const dragExpandedReferenceBoardFolderIds = ref<Set<number>>(new Set())
const pressedItem = ref<GalleryLayoutItem | null>(null)
const pressedPointerId = ref<number | null>(null)
const pressStart = ref<{ x: number; y: number } | null>(null)
const pressCurrent = ref<{ x: number; y: number } | null>(null)
const activeReferenceBoardId = ref<number | null>(null)
const previewReferenceBoardIds = ref<Set<number>>(new Set())
const boardContextMenu = ref<BoardContextMenu | null>(null)
const boardDraft = ref<BoardDraft | null>(null)
const newBoardName = ref('')
const isComposingBoardName = ref(false)
const renamingReferenceBoardFolderId = ref<number | null>(null)
const renamingReferenceBoardFolderName = ref('')
const isComposingReferenceBoardFolderRename = ref(false)
const renamingReferenceBoardId = ref<number | null>(null)
const renamingReferenceBoardName = ref('')
const isComposingReferenceBoardRename = ref(false)
const draggedReferenceBoardId = ref<number | null>(null)
const draggedReferenceBoardFolderId = ref<number | null>(null)
const referenceBoardDragOverKind = ref<'board' | 'folder' | 'space' | null>(null)
const referenceBoardDragOverId = ref<number | null>(null)
const isWindowMaximized = ref(false)
const isTitlebarHovered = ref(false)
const galleryScrollTop = ref(0)
const galleryViewportHeight = ref(0)
const previewDragOverDeleteZone = ref(false)
const previewBoardItemDrag = ref<PreviewBoardItemDragState | null>(null)
const lastImageDragEndedAt = ref(0)
const lastPreviewBoardDragEndedAt = ref(0)
const boardCanvasBoundsById = ref<Record<number, BoardCanvasBounds>>({})
const boardUndoStack = ref<BoardHistoryEntry[]>([])
const boardRedoStack = ref<BoardHistoryEntry[]>([])
const boardSpaceFocusMode = ref<'item' | 'canvas'>('item')
const isApplyingBoardHistory = ref(false)
const imageDetailContextMenu = ref<ImageDetailContextMenu>(null)
const galleryImageContextMenu = ref<GalleryImageContextMenu>(null)
const previewBoardPointerId = ref<number | null>(null)
const dragReferenceBoardFolderCollapseTimer = ref<number | null>(null)
const boardPointerUseMaxAgeMs = 5000

const gap = 12
const defaultBoardCanvasWidth = 1440
const defaultBoardCanvasHeight = 960
const isSettingsView = computed(() => viewMode.value === 'settings')
const sidebarPinnedEffective = computed(() => isSettingsView.value || sidebarPinned.value)
const sidebarOpen = computed(() => sidebarPinnedEffective.value || sidebarHoverOpen.value)
const rightSidebarOpen = computed(() => rightSidebarPinned.value || rightSidebarHoverOpen.value)
const isTitlebarPinned = computed(() => !isWindowMaximized.value || isTitlebarHovered.value)
const isReferencePreviewActive = computed(() => previewReferenceBoardIds.value.size > 0)

const referenceBoardPreviewBlocks = computed(() => {
  const imageById = new Map(library.value.images.map((image) => [image.id, image]))
  const boardById = new Map(library.value.referenceBoards.map((board) => [board.id, board]))
  const rows: Array<{
    boardId: number
    name: string
    thumbnails: Array<{ itemId: number; imageId: string; thumbnailUrl: string }>
  }> = []

  for (const boardId of previewReferenceBoardIds.value) {
    const board = boardById.get(boardId)
    if (!board) continue

    const thumbnails = library.value.referenceBoardItems
      .filter((item) => item.boardId === boardId)
      .sort((a, b) => b.createdAt - a.createdAt)
      .slice(0, 12)
      .map((item) => {
        const image = imageById.get(item.imageId)
        if (!image) return null
        return {
          itemId: item.id,
          imageId: item.imageId,
          thumbnailUrl: convertFileSrc(image.path),
        }
      })
      .filter(
        (item): item is { itemId: number; imageId: string; thumbnailUrl: string } => item !== null,
      )

    rows.push({
      boardId,
      name: board.name,
      thumbnails,
    })
  }

  return rows
})

const effectiveExpandedReferenceBoardFolderIds = computed(() => {
  const next = new Set(expandedReferenceBoardFolderIds.value)
  for (const folderId of dragExpandedReferenceBoardFolderIds.value) {
    next.add(folderId)
  }
  return next
})

const referenceBoardRows = computed<ReferenceBoardRow[]>(() =>
  buildReferenceBoardRows(effectiveExpandedReferenceBoardFolderIds.value),
)

function buildReferenceBoardRows(expandedIds: Set<number>) {
  const result: ReferenceBoardRow[] = []
  const boardsByFolder = new Map<number | null, ReferenceBoard[]>()
  for (const board of library.value.referenceBoards) {
    const key = board.folderId ?? null
    const group = boardsByFolder.get(key) ?? []
    group.push(board)
    boardsByFolder.set(key, group)
  }

  for (const group of boardsByFolder.values()) {
    group.sort(
      (a, b) =>
        (a.sortOrder ?? 0) - (b.sortOrder ?? 0) ||
        a.name.localeCompare(b.name, 'zh-Hans-CN') ||
        a.id - b.id,
    )
  }

  for (const board of boardsByFolder.get(null) ?? []) {
    result.push({ kind: 'board', id: board.id, folderId: null, name: board.name, depth: 0 })
  }

  const folders = [...library.value.referenceBoardFolders].sort(
    (a, b) =>
      (a.sortOrder ?? 0) - (b.sortOrder ?? 0) ||
      a.name.localeCompare(b.name, 'zh-Hans-CN') ||
      a.id - b.id,
  )
  for (const folder of folders) {
    const boards = boardsByFolder.get(folder.id) ?? []
    const isExpanded = expandedIds.has(folder.id)
    result.push({ kind: 'folder', id: folder.id, name: folder.name, hasBoards: boards.length > 0, isExpanded })
    if (isExpanded) {
      for (const board of boards) {
        result.push({ kind: 'board', id: board.id, folderId: folder.id, name: board.name, depth: 1 })
      }
    }
  }

  return result
}

const activeReferenceBoard = computed(() =>
  library.value.referenceBoards.find((board) => board.id === activeReferenceBoardId.value) ?? null,
)

const activeReferenceBoardItems = computed(() => {
  if (activeReferenceBoardId.value === null) return []
  const imagesById = new Map(library.value.images.map((image) => [image.id, image]))
  return library.value.referenceBoardItems
    .filter((item) => item.boardId === activeReferenceBoardId.value)
    .map((item) => ({ item, image: imagesById.get(item.imageId) }))
    .filter((entry): entry is { item: ReferenceBoardItem; image: GalleryImage } => Boolean(entry.image))
})

function createDefaultBoardCanvasBounds(): BoardCanvasBounds {
  return {
    minX: 0,
    minY: 0,
    maxX: defaultBoardCanvasWidth,
    maxY: defaultBoardCanvasHeight,
  }
}

function getBoardCanvasBounds(boardId: number) {
  return boardCanvasBoundsById.value[boardId] ?? createDefaultBoardCanvasBounds()
}

const activeBoardCanvasBounds = computed(() => {
  if (activeReferenceBoardId.value === null) {
    return createDefaultBoardCanvasBounds()
  }
  return getBoardCanvasBounds(activeReferenceBoardId.value)
})

const boardContextMenuStyle = computed(() => {
  if (!boardContextMenu.value) return {}
  return {
    left: `${boardContextMenu.value.x}px`,
    top: `${boardContextMenu.value.y}px`,
  }
})

const boardDraftStyle = computed(() => {
  if (!boardDraft.value) return {}
  return {
    left: `${boardDraft.value.x}px`,
    top: `${boardDraft.value.y}px`,
  }
})

const referenceBoardCanvasMenuStyle = computed(() => {
  if (!referenceBoardCanvasMenu.value) return {}
  return {
    left: `${referenceBoardCanvasMenu.value.x}px`,
    top: `${referenceBoardCanvasMenu.value.y}px`,
  }
})

const imageDetailContextMenuStyle = computed(() => {
  if (!imageDetailContextMenu.value) return {}
  return {
    left: `${imageDetailContextMenu.value.x}px`,
    top: `${imageDetailContextMenu.value.y}px`,
  }
})

const galleryImageContextMenuStyle = computed(() => {
  if (!galleryImageContextMenu.value) return {}
  return {
    left: `${galleryImageContextMenu.value.x}px`,
    top: `${galleryImageContextMenu.value.y}px`,
  }
})

const columnCount = computed(() => {
  const width = viewportWidth.value
  const isPortrait = viewportHeight.value > width

  if (isPortrait || width < 760) return 2
  if (width < 1120) return 4
  if (width < 1500) return 5
  return 6
})

const columnWidth = computed(() => {
  const availableWidth = Math.max(320, viewportWidth.value)
  return Math.floor((availableWidth - gap * (columnCount.value - 1)) / columnCount.value)
})

const masonryContentWidth = computed(() => {
  if (columnCount.value <= 0) return viewportWidth.value
  return columnCount.value * columnWidth.value + gap * (columnCount.value - 1)
})

const minItemHeight = computed(() => Math.max(96, columnWidth.value * 0.55))
const maxItemHeight = computed(() => Math.max(260, columnWidth.value * 2.15))

const layoutItems = computed<GalleryLayoutItem[]>(() => {
  const result: GalleryLayoutItem[] = []
  const columnHeights = Array.from({ length: columnCount.value }, () => 0)

  for (const image of visibleImages.value) {
    const columnIndex = shortestColumnIndex(columnHeights)
    const naturalHeight =
      image.width > 0 ? (image.height / image.width) * columnWidth.value : minItemHeight.value
    const height = clamp(naturalHeight, minItemHeight.value, maxItemHeight.value)

    result.push({
      id: image.id,
      thumbnailUrl: convertFileSrc(image.path),
      x: columnIndex * (columnWidth.value + gap),
      y: columnHeights[columnIndex],
      width: columnWidth.value,
      height,
      columnIndex,
    })

    columnHeights[columnIndex] += height + gap
  }

  return result
})

const totalHeight = computed(() => {
  if (layoutItems.value.length === 0) return 0
  return Math.max(...layoutItems.value.map((item) => item.y + item.height))
})

const renderedLayoutItems = computed(() => {
  if (layoutItems.value.length === 0) return layoutItems.value
  if (galleryViewportHeight.value <= 0) return layoutItems.value

  const buffer = Math.max(480, galleryViewportHeight.value * 0.8)
  const viewportTop = galleryScrollTop.value - buffer
  const viewportBottom = galleryScrollTop.value + galleryViewportHeight.value + buffer

  return layoutItems.value.filter((item) => item.y + item.height >= viewportTop && item.y <= viewportBottom)
})

const searchPanelStyle = computed<Record<string, string>>(() => ({
  '--search-reveal': '1',
  '--search-opacity': '1',
  '--search-translate-y': '0%',
}))

const {
  sidebarPinned,
  rightSidebarPinned,
  autoFixRightSidebarOnPreview,
  themeMode,
  initAppSettingsFromStorage,
  setSidebarPinned,
  setRightSidebarPinned,
  setAutoFixRightSidebarOnPreview,
  setThemeMode,
} = useAppSettings()

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
  searchConfidenceMin,
  searchConfidenceMax,
  searchRunning,
  searchError,
  isSearchFocused,
  isSearchPointerInside,
  visibleImages,
  activeImageDetailId,
  activeImageDetail,
  groupedImageTags,
  setSearchPointerInside,
  setSearchFocus,
  setSearchZhInput,
  openSearchZhSuggestionPanel,
  closeSearchZhSuggestionPanelDeferred,
  selectSearchZhSuggestion,
  removeSearchZhSuggestion,
  setSearchEnQuery,
  setSearchFileNameQuery,
  setSearchConfidenceMin,
  setSearchConfidenceMax,
  closeImageDetail,
  openGalleryImageDetail,
} = useGallerySearch<LibraryStore>({
  library,
  folderScopedImages,
  activeUserFolderId,
  lastImageDragEndedAt,
  formatError,
  clamp,
  onOpenImageDetail() {
    imageDetailContextMenu.value = null
  },
  onCloseImageDetail() {
    imageDetailContextMenu.value = null
  },
})

const {
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
} = useBackgroundScan({
  loadLibrary,
  formatError,
  setErrorText(value) {
    errorText.value = value
  },
  autoScanOnStartupStorageKey,
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
  window.addEventListener('click', closeImageDetailContextMenu)
  window.addEventListener('click', closeGalleryImageContextMenu)
  window.addEventListener('keydown', handleGlobalKeydown)
  void refreshBackgroundScanStatus()
  startBackgroundScanPolling()
  void startStartupCleanup()
  startAutoScanIfEnabled()
})

onUnmounted(() => {
  stopBackgroundScanPolling()
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
  window.removeEventListener('click', closeImageDetailContextMenu)
  window.removeEventListener('click', closeGalleryImageContextMenu)
  window.removeEventListener('keydown', handleGlobalKeydown)
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
  boardUndoStack.value = []
  boardRedoStack.value = []
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
    const next = new Set([...previewReferenceBoardIds.value].filter((id) => exists.has(id)))
    if (next.size !== previewReferenceBoardIds.value.size) {
      previewReferenceBoardIds.value = next
    }
    const nextBounds: Record<number, BoardCanvasBounds> = {}
    for (const boardId of exists) {
      nextBounds[boardId] = boardCanvasBoundsById.value[boardId] ?? createDefaultBoardCanvasBounds()
    }
    boardCanvasBoundsById.value = nextBounds
  },
)

watch([visibleImages, sidebarPinned], async () => {
  await nextTick()
  updateViewportSize()
})

async function loadLibrary() {
  isLoading.value = true
  errorText.value = ''

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('list_library')
    for (const board of library.value.referenceBoards) {
      ensureBoardCanvasBoundsFor(board.id)
    }
    updateStatus()
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    isLoading.value = false
    await nextTick()
    updateViewportSize()
  }
}

async function pickFolder() {
  errorText.value = ''
  const selected = await open({
    directory: true,
    multiple: false,
    title: '选择图库文件夹',
  })

  if (typeof selected === 'string') {
    folderPathInput.value = selected
  }
}

async function addFolder() {
  errorText.value = ''

  if (folderPathInput.value.trim().length === 0) {
    errorText.value = '请输入图库文件夹路径'
    return
  }

  isLoading.value = true
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
    isLoading.value = false
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

function toggleReferenceBoardFolderExpanded(folderId: number) {
  const next = new Set(expandedReferenceBoardFolderIds.value)
  if (next.has(folderId)) {
    next.delete(folderId)
  } else {
    next.add(folderId)
  }
  expandedReferenceBoardFolderIds.value = next
}

function expandReferenceBoardFolder(folderId: number) {
  const next = new Set(expandedReferenceBoardFolderIds.value)
  next.add(folderId)
  expandedReferenceBoardFolderIds.value = next
}

function referenceBoardIdFromPoint(x: number, y: number) {
  const element = document.elementFromPoint(x, y)
  const boardElement = element?.closest<HTMLElement>('[data-reference-board-id]')
  const boardId = boardElement?.dataset.referenceBoardId
  return boardId ? Number(boardId) : null
}

function referenceBoardFolderIdFromPoint(x: number, y: number) {
  const element = document.elementFromPoint(x, y)
  const folderElement = element?.closest<HTMLElement>('[data-reference-board-folder-id]')
  const folderId = folderElement?.dataset.referenceBoardFolderId
  return folderId ? Number(folderId) : null
}

function isPointInsideFolderDropPanel(x: number, y: number) {
  return Boolean(document.elementFromPoint(x, y)?.closest('.folder-drop-panel'))
}

function isPointInsideRightSidebarArea(x: number, y: number) {
  return Boolean(document.elementFromPoint(x, y)?.closest('.right-sidebar, .right-sidebar-hotspot'))
}

function clearDragReferenceBoardFolderCollapseTimer() {
  if (dragReferenceBoardFolderCollapseTimer.value !== null) {
    window.clearTimeout(dragReferenceBoardFolderCollapseTimer.value)
    dragReferenceBoardFolderCollapseTimer.value = null
  }
}

function clearDragExpandedReferenceBoardFoldersNow() {
  clearDragReferenceBoardFolderCollapseTimer()
  dragExpandedReferenceBoardFolderIds.value = new Set()
}

function scheduleClearDragExpandedReferenceBoardFolders(delayMs = 160) {
  if (dragExpandedReferenceBoardFolderIds.value.size === 0) return
  if (dragReferenceBoardFolderCollapseTimer.value !== null) return
  dragReferenceBoardFolderCollapseTimer.value = window.setTimeout(() => {
    dragReferenceBoardFolderCollapseTimer.value = null
    dragExpandedReferenceBoardFolderIds.value = new Set()
  }, delayMs)
}

function keepDragExpandedReferenceBoardFolder(folderId: number) {
  clearDragReferenceBoardFolderCollapseTimer()
  if (expandedReferenceBoardFolderIds.value.has(folderId)) {
    dragExpandedReferenceBoardFolderIds.value = new Set()
    return
  }
  const current = [...dragExpandedReferenceBoardFolderIds.value][0]
  if (current === folderId) return
  dragExpandedReferenceBoardFolderIds.value = new Set([folderId])
}

function floatingPanelPosition(x: number, y: number) {
  const panelWidth = 184
  const panelHeight = 260
  const side = x + panelWidth + 36 > window.innerWidth ? 'left' : 'right'
  return {
    x: side === 'right' ? x + 28 : x - panelWidth - 28,
    y: Math.max(84, Math.min(y - 40, window.innerHeight - panelHeight - 16)),
  }
}

function onReferenceBoardFolderRowClick(folderId: number) {
  if (renamingReferenceBoardFolderId.value === folderId) return

  toggleReferenceBoardFolderExpanded(folderId)
}

function showReferenceBoard(boardId: number) {
  activeReferenceBoardId.value = boardId
  ensureBoardCanvasBoundsFor(boardId)
  viewMode.value = 'board'
}

function closeBoardContextMenu() {
  boardContextMenu.value = null
}

function closeImageDetailContextMenu() {
  imageDetailContextMenu.value = null
}

function closeGalleryImageContextMenu() {
  galleryImageContextMenu.value = null
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
    closeImageDetail()
    closeGalleryImageContextMenu()
    clearImagePress()
    clearFolderPress()

    pressedItem.value = null
    pressedPointerId.value = null
    pressStart.value = null
    pressCurrent.value = null
    dragState.value = null
    dragExpandedFolderIds.value = new Set()
    clearDragExpandedReferenceBoardFoldersNow()

    folderPointerState.value = null
    draggedFolderId.value = null
    folderDragOverId.value = null
    clearBoardInteraction()
    return
  }

  if (event.isComposing || isEditableKeyboardTarget(event)) return

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

function clearImagePress() {
  if (longPressTimer.value !== null) {
    window.clearTimeout(longPressTimer.value)
    longPressTimer.value = null
  }
}

function cancelImageDrag() {
  clearImagePress()
  pressedItem.value = null
  pressedPointerId.value = null
  pressStart.value = null
  pressCurrent.value = null
  dragState.value = null
  dragExpandedFolderIds.value = new Set()
  clearDragExpandedReferenceBoardFoldersNow()
}

function clearPreviewBoardItemDrag() {
  const hadDrag = previewBoardItemDrag.value !== null
  previewBoardPointerId.value = null
  previewBoardItemDrag.value = null
  previewDragOverDeleteZone.value = false
  if (hadDrag) {
    lastPreviewBoardDragEndedAt.value = Date.now()
  }
}

function previewBoardDragIconKind(state: PreviewBoardItemDragState) {
  if (state.targetKind === 'gallery') return 'delete'
  if (state.targetBoardId === null || state.targetBoardId === state.sourceBoardId) return 'none'
  return state.mode === 'move' ? 'move' : 'copy'
}

function onPreviewReferenceThumbClick(boardId: number) {
  if (Date.now() - lastPreviewBoardDragEndedAt.value < 260) return
  showReferenceBoard(boardId)
}

function previewDropModeFromEvent(
  event: DragEvent,
  element: EventTarget | null,
): 'copy' | 'move' {
  const host = element instanceof HTMLElement ? element : null
  if (!host) return 'copy'
  const rect = host.getBoundingClientRect()
  const midX = rect.left + rect.width / 2
  return event.clientX < midX ? 'move' : 'copy'
}

function setPreviewBoardDragTarget(
  boardId: number,
  kind: Exclude<PreviewBoardDragKind, 'gallery' | null>,
  mode: 'copy' | 'move',
  event: DragEvent,
) {
  const state = previewBoardItemDrag.value
  if (!state) return
  if (state.sourceBoardId === boardId) {
    state.targetBoardId = null
    state.targetKind = null
    previewDragOverDeleteZone.value = false
    return
  }
  state.targetBoardId = boardId
  state.targetKind = kind
  state.mode = mode
  state.x = event.clientX
  state.y = event.clientY
  previewDragOverDeleteZone.value = false
  if (event.dataTransfer) {
    event.dataTransfer.dropEffect = mode === 'move' ? 'move' : 'copy'
  }
}

function startPreviewBoardItemDrag(
  itemId: number,
  imageId: string,
  sourceBoardId: number,
  thumbnailUrl: string,
  event: DragEvent,
) {
  closeBoardContextMenu()
  clearReferenceBoardDragState()
  previewBoardItemDrag.value = {
    itemId,
    imageId,
    sourceBoardId,
    thumbnailUrl,
    x: event.clientX,
    y: event.clientY,
    targetBoardId: null,
    targetKind: null,
    mode: 'copy',
  }
  previewDragOverDeleteZone.value = false
  event.dataTransfer?.setData('text/plain', `preview-item:${itemId}`)
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'copyMove'
  }
}

function startPreviewBoardItemPointerDrag(
  itemId: number,
  imageId: string,
  sourceBoardId: number,
  thumbnailUrl: string,
  event: PointerEvent,
) {
  if (event.button !== 0) return
  event.preventDefault()
  event.stopPropagation()
  closeBoardContextMenu()
  clearReferenceBoardDragState()
  previewBoardPointerId.value = event.pointerId
  previewBoardItemDrag.value = {
    itemId,
    imageId,
    sourceBoardId,
    thumbnailUrl,
    x: event.clientX,
    y: event.clientY,
    targetBoardId: null,
    targetKind: null,
    mode: 'copy',
  }
  previewDragOverDeleteZone.value = false
}

function movePreviewBoardItemPointerDrag(event: PointerEvent) {
  if (previewBoardPointerId.value === null || previewBoardPointerId.value !== event.pointerId) return
  const state = previewBoardItemDrag.value
  if (!state) return

  state.x = event.clientX
  state.y = event.clientY

  const element = document.elementFromPoint(event.clientX, event.clientY) as HTMLElement | null
  if (element?.closest('.gallery-page')) {
    state.targetBoardId = null
    state.targetKind = 'gallery'
    state.mode = 'move'
    previewDragOverDeleteZone.value = true
    return
  }

  const boardElement = element?.closest<HTMLElement>('[data-reference-board-id]')
  if (boardElement?.dataset.referenceBoardId) {
    const boardId = Number(boardElement.dataset.referenceBoardId)
    if (Number.isFinite(boardId)) {
      if (boardId === state.sourceBoardId) {
        state.targetBoardId = null
        state.targetKind = null
        previewDragOverDeleteZone.value = false
        return
      }
      const mode = event.clientX < boardElement.getBoundingClientRect().left + boardElement.getBoundingClientRect().width / 2 ? 'move' : 'copy'
      const kind: Exclude<PreviewBoardDragKind, 'gallery' | null> = boardElement.closest('.reference-board-preview__block')
        ? 'preview'
        : 'board'
      state.targetBoardId = boardId
      state.targetKind = kind
      state.mode = mode
      previewDragOverDeleteZone.value = false
      return
    }
  }

  state.targetBoardId = null
  state.targetKind = null
  previewDragOverDeleteZone.value = false
}

async function finishPreviewBoardItemPointerDrag(event: PointerEvent) {
  if (previewBoardPointerId.value === null || previewBoardPointerId.value !== event.pointerId) return
  const state = previewBoardItemDrag.value
  if (!state) {
    clearPreviewBoardItemDrag()
    return
  }

  previewBoardPointerId.value = null
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    if (state.targetKind === 'gallery') {
      await removeReferenceBoardItemsWithHistory([state.itemId])
      return
    }

    if (state.targetBoardId !== null) {
      if (state.mode === 'copy') {
        library.value = await invoke<LibraryStore>('add_image_to_reference_board_command', {
          imageId: state.imageId,
          boardId: state.targetBoardId,
        })
        ensureBoardCanvasBoundsFor(state.targetBoardId)
      } else if (state.sourceBoardId !== state.targetBoardId) {
        library.value = await invoke<LibraryStore>('add_image_to_reference_board_command', {
          imageId: state.imageId,
          boardId: state.targetBoardId,
        })
        ensureBoardCanvasBoundsFor(state.targetBoardId)
        library.value = await invoke<LibraryStore>('remove_reference_board_item_command', {
          itemId: state.itemId,
        })
        if (selectedReferenceBoardItemId.value === state.itemId) {
          selectedReferenceBoardItemId.value = null
        }
        clearInternalBoardCopyRefForItem(state.itemId)
      }
    }
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    clearPreviewBoardItemDrag()
  }
}

function onPreviewBoardItemDragOverPreview(boardId: number, event: DragEvent) {
  if (!previewBoardItemDrag.value) return
  event.preventDefault()
  event.stopPropagation()
  const mode = previewDropModeFromEvent(event, event.currentTarget)
  setPreviewBoardDragTarget(boardId, 'preview', mode, event)
}

function onPreviewBoardItemDragOverBoard(boardId: number, event: DragEvent) {
  if (!previewBoardItemDrag.value) return
  event.preventDefault()
  event.stopPropagation()
  const mode = previewDropModeFromEvent(event, event.currentTarget)
  setPreviewBoardDragTarget(boardId, 'board', mode, event)
}

async function dropPreviewBoardItem(boardId: number, event: DragEvent) {
  if (!previewBoardItemDrag.value) return
  event.preventDefault()
  event.stopPropagation()
  const state = previewBoardItemDrag.value
  const mode = previewDropModeFromEvent(event, event.currentTarget) ?? state.mode

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    if (mode === 'copy') {
      library.value = await invoke<LibraryStore>('add_image_to_reference_board_command', {
        imageId: state.imageId,
        boardId,
      })
      ensureBoardCanvasBoundsFor(boardId)
    } else if (state.sourceBoardId !== boardId) {
      library.value = await invoke<LibraryStore>('add_image_to_reference_board_command', {
        imageId: state.imageId,
        boardId,
      })
      ensureBoardCanvasBoundsFor(boardId)
      library.value = await invoke<LibraryStore>('remove_reference_board_item_command', {
        itemId: state.itemId,
      })
      if (selectedReferenceBoardItemId.value === state.itemId) {
        selectedReferenceBoardItemId.value = null
      }
      clearInternalBoardCopyRefForItem(state.itemId)
    }
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    clearPreviewBoardItemDrag()
  }
}

function endPreviewBoardItemDrag() {
  clearPreviewBoardItemDrag()
}

function onGalleryPreviewBoardItemDragOver(event: DragEvent) {
  const state = previewBoardItemDrag.value
  if (!state) return
  event.preventDefault()
  event.stopPropagation()
  state.x = event.clientX
  state.y = event.clientY
  state.targetBoardId = null
  state.targetKind = 'gallery'
  state.mode = 'move'
  previewDragOverDeleteZone.value = true
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'
}

async function onGalleryPreviewBoardItemDrop(event: DragEvent) {
  const state = previewBoardItemDrag.value
  if (!state) return
  event.preventDefault()
  event.stopPropagation()
  try {
    await removeReferenceBoardItemsWithHistory([state.itemId])
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    clearPreviewBoardItemDrag()
  }
}

async function addImageToReferenceBoard(boardId: number) {
  if (!dragState.value) return
  const { invoke } = await import('@tauri-apps/api/core')
  library.value = await invoke<LibraryStore>('add_image_to_reference_board_command', {
    imageId: dragState.value.imageId,
    boardId,
  })
  ensureBoardCanvasBoundsFor(boardId)
}

function startImagePress(item: GalleryLayoutItem, event: PointerEvent) {
  if (event.button !== 0) return

  clearImagePress()
  pressedItem.value = item
  pressedPointerId.value = event.pointerId
  pressStart.value = { x: event.clientX, y: event.clientY }
  pressCurrent.value = { x: event.clientX, y: event.clientY }

  longPressTimer.value = window.setTimeout(() => {
    if (!pressedItem.value || !pressCurrent.value) return
    const current = pressCurrent.value
    const panelPosition = floatingPanelPosition(current.x, current.y)
    dragState.value = {
      imageId: item.id,
      thumbnailUrl: item.thumbnailUrl,
      x: current.x,
      y: current.y,
      panelX: panelPosition.x,
      panelY: panelPosition.y,
      overFolderId: null,
      overBoardId: null,
      overRightSidebar: false,
    }
    dragExpandedFolderIds.value = new Set()
    clearDragExpandedReferenceBoardFoldersNow()
  }, imageDragDelayMs)
}

function moveImageDrag(event: PointerEvent) {
  if (!dragState.value) {
    if (pressedItem.value && pressedPointerId.value === event.pointerId) {
      pressCurrent.value = { x: event.clientX, y: event.clientY }
    }
    return
  }

  dragState.value.x = event.clientX
  dragState.value.y = event.clientY
  const overFolderCandidateId = folderIdFromPoint(event.clientX, event.clientY)
  const overFolderId =
    overFolderCandidateId !== null && !folderHasChildren(overFolderCandidateId)
      ? overFolderCandidateId
      : null
  const overBoardId = referenceBoardIdFromPoint(event.clientX, event.clientY)
  const overBoardFolderId = referenceBoardFolderIdFromPoint(event.clientX, event.clientY)
  const overRightSidebar = isPointInsideRightSidebarArea(event.clientX, event.clientY)
  dragState.value.overFolderId = overFolderId
  dragState.value.overBoardId = overBoardId
  dragState.value.overRightSidebar = overRightSidebar

  if (overFolderCandidateId !== null) {
    dragExpandedFolderIds.value = expandedDropFolderIdsFor(overFolderCandidateId)
  } else if (!isPointInsideFolderDropPanel(event.clientX, event.clientY)) {
    dragExpandedFolderIds.value = new Set()
  }

  if (overBoardFolderId !== null) {
    keepDragExpandedReferenceBoardFolder(overBoardFolderId)
    return
  }

  const tempExpandedFolderId = [...dragExpandedReferenceBoardFolderIds.value][0]
  if (tempExpandedFolderId === undefined) return

  if (!overRightSidebar) {
    clearDragExpandedReferenceBoardFoldersNow()
    return
  }

  if (overBoardId !== null) {
    const board = library.value.referenceBoards.find((item) => item.id === overBoardId)
    if (board?.folderId === tempExpandedFolderId) {
      clearDragReferenceBoardFolderCollapseTimer()
      return
    }
  }

  scheduleClearDragExpandedReferenceBoardFolders()
}

async function finishImageDrag(event: PointerEvent) {
  clearImagePress()

  if (!dragState.value) {
    pressedItem.value = null
    return
  }

  try {
    const boardId = dragState.value.overBoardId ?? referenceBoardIdFromPoint(event.clientX, event.clientY)
    if (boardId !== null) {
      await addImageToReferenceBoard(boardId)
      const board = library.value.referenceBoards.find((item) => item.id === boardId)
      if (board?.folderId != null) {
        expandReferenceBoardFolder(board.folderId)
      }
    }

    if (!dragState.value) return
    const folderCandidateId = dragState.value.overFolderId ?? folderIdFromPoint(event.clientX, event.clientY)
    const folderId =
      folderCandidateId !== null && !folderHasChildren(folderCandidateId) ? folderCandidateId : null
    if (folderId !== null) {
      await assignImageToFolder(dragState.value.imageId, folderId)
    }
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    lastImageDragEndedAt.value = Date.now()
    cancelImageDrag()
  }
}

function clearReferenceBoardDragState() {
  draggedReferenceBoardId.value = null
  draggedReferenceBoardFolderId.value = null
  referenceBoardDragOverKind.value = null
  referenceBoardDragOverId.value = null
}

async function reorderReferenceBoardFolder(folderId: number, targetFolderId: number) {
  if (folderId === targetFolderId) return
  const { invoke } = await import('@tauri-apps/api/core')
  library.value = await invoke<LibraryStore>('reorder_reference_board_folder_command', {
    folderId,
    targetFolderId,
  })
}

async function moveReferenceBoardToFolder(boardId: number, folderId: number | null) {
  const { invoke } = await import('@tauri-apps/api/core')
  library.value = await invoke<LibraryStore>('move_reference_board_to_folder_command', {
    boardId,
    folderId,
  })
}

async function reorderReferenceBoard(boardId: number, targetBoardId: number) {
  if (boardId === targetBoardId) return
  const { invoke } = await import('@tauri-apps/api/core')
  library.value = await invoke<LibraryStore>('reorder_reference_board_command', {
    boardId,
    targetBoardId,
  })
}

function startReferenceBoardFolderDrag(folderId: number, event: DragEvent) {
  if (renamingReferenceBoardFolderId.value === folderId) return
  draggedReferenceBoardFolderId.value = folderId
  draggedReferenceBoardId.value = null
  referenceBoardDragOverKind.value = null
  referenceBoardDragOverId.value = null
  closeBoardContextMenu()
  event.dataTransfer?.setData('text/plain', `folder:${folderId}`)
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function startReferenceBoardDrag(boardId: number, event: DragEvent) {
  if (renamingReferenceBoardId.value === boardId) return
  draggedReferenceBoardId.value = boardId
  draggedReferenceBoardFolderId.value = null
  referenceBoardDragOverKind.value = null
  referenceBoardDragOverId.value = null
  closeBoardContextMenu()
  event.dataTransfer?.setData('text/plain', `board:${boardId}`)
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function onReferenceBoardDragOverFolder(folderId: number, event: DragEvent) {
  if (draggedReferenceBoardFolderId.value === null && draggedReferenceBoardId.value === null) return
  event.preventDefault()
  event.stopPropagation()
  referenceBoardDragOverKind.value = 'folder'
  referenceBoardDragOverId.value = folderId
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'
}

function onReferenceBoardDragOverBoard(boardId: number, event: DragEvent) {
  if (draggedReferenceBoardId.value === null) return
  event.preventDefault()
  event.stopPropagation()
  referenceBoardDragOverKind.value = 'board'
  referenceBoardDragOverId.value = boardId
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'
}

function onReferenceBoardDragOverSpace(event: DragEvent) {
  if (draggedReferenceBoardId.value === null && draggedReferenceBoardFolderId.value === null) return
  event.preventDefault()
  referenceBoardDragOverKind.value = 'space'
  referenceBoardDragOverId.value = null
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'
}

async function dropOnReferenceBoardFolder(folderId: number, event: DragEvent) {
  event.preventDefault()
  event.stopPropagation()
  try {
    if (draggedReferenceBoardFolderId.value !== null) {
      await reorderReferenceBoardFolder(draggedReferenceBoardFolderId.value, folderId)
      return
    }
    if (draggedReferenceBoardId.value !== null) {
      await moveReferenceBoardToFolder(draggedReferenceBoardId.value, folderId)
      expandReferenceBoardFolder(folderId)
    }
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    clearReferenceBoardDragState()
  }
}

async function dropOnReferenceBoard(boardId: number, event: DragEvent) {
  event.preventDefault()
  event.stopPropagation()
  try {
    if (draggedReferenceBoardId.value !== null) {
      await reorderReferenceBoard(draggedReferenceBoardId.value, boardId)
    }
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    clearReferenceBoardDragState()
  }
}

async function dropOnReferenceBoardSpace(event: DragEvent) {
  event.preventDefault()
  try {
    if (draggedReferenceBoardId.value !== null) {
      await moveReferenceBoardToFolder(draggedReferenceBoardId.value, null)
    }
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    clearReferenceBoardDragState()
  }
}

function endReferenceBoardDrag() {
  clearReferenceBoardDragState()
}

function openBoardSpaceMenu(folderId: number | null, event: MouseEvent) {
  event.preventDefault()
  event.stopPropagation()
  boardContextMenu.value = { kind: 'space', folderId, x: event.clientX, y: event.clientY }
}

function openReferenceBoardFolderMenu(folderId: number, event: MouseEvent) {
  event.preventDefault()
  event.stopPropagation()
  boardContextMenu.value = { kind: 'folder', folderId, x: event.clientX, y: event.clientY }
}

function openReferenceBoardMenu(boardId: number, event: MouseEvent) {
  event.preventDefault()
  event.stopPropagation()
  boardContextMenu.value = { kind: 'board', boardId, x: event.clientX, y: event.clientY }
}

function toggleReferenceBoardPreview(boardId: number) {
  const next = new Set(previewReferenceBoardIds.value)
  if (next.has(boardId)) {
    next.delete(boardId)
  } else {
    next.add(boardId)
    if (autoFixRightSidebarOnPreview.value && !rightSidebarPinned.value) {
      rightSidebarPinned.value = true
    }
  }
  previewReferenceBoardIds.value = next
  closeBoardContextMenu()
}

function removeReferenceBoardPreview(boardId: number) {
  const next = new Set(previewReferenceBoardIds.value)
  if (!next.delete(boardId)) return
  previewReferenceBoardIds.value = next
}

async function openBoardDraft(kind: 'board' | 'folder', folderId: number | null, x: number, y: number) {
  boardDraft.value = { kind, folderId, x, y }
  newBoardName.value = ''
  await nextTick()
  const input = document.querySelector<HTMLInputElement>('[data-board-draft-input]')
  input?.focus()
  input?.select()
}

function closeBoardDraft() {
  boardDraft.value = null
  newBoardName.value = ''
}

async function commitBoardDraft() {
  if (isComposingBoardName.value || !boardDraft.value) return
  const name = newBoardName.value.trim()
  if (!name) return

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    if (boardDraft.value.kind === 'folder') {
      library.value = await invoke<LibraryStore>('create_reference_board_folder_command', {
        name,
      })
    } else {
      library.value = await invoke<LibraryStore>('create_reference_board_command', {
        folderId: boardDraft.value.folderId,
        name,
      })
      if (boardDraft.value.folderId !== null) {
        expandReferenceBoardFolder(boardDraft.value.folderId)
      }
    }
    closeBoardDraft()
    closeBoardContextMenu()
  } catch (error) {
    errorText.value = formatError(error)
  }
}

function cancelReferenceBoardFolderRename() {
  renamingReferenceBoardFolderId.value = null
  renamingReferenceBoardFolderName.value = ''
  isComposingReferenceBoardFolderRename.value = false
}

function startComposingReferenceBoardFolderRename() {
  isComposingReferenceBoardFolderRename.value = true
}

function endComposingReferenceBoardFolderRename() {
  isComposingReferenceBoardFolderRename.value = false
}

function setRenamingReferenceBoardFolderName(value: string) {
  renamingReferenceBoardFolderName.value = value
}

function startReferenceBoardFolderRename(folderId: number) {
  const folder = library.value.referenceBoardFolders.find((item) => item.id === folderId)
  if (!folder) return
  renamingReferenceBoardFolderId.value = folderId
  renamingReferenceBoardFolderName.value = folder.name
  isComposingReferenceBoardFolderRename.value = false
  closeBoardContextMenu()
  void nextTick(() => {
    const input = document.querySelector<HTMLInputElement>(
      `[data-reference-board-folder-rename-id="${folderId}"]`,
    )
    input?.focus()
    input?.select()
  })
}

async function commitReferenceBoardFolderRename() {
  if (isComposingReferenceBoardFolderRename.value) return
  const folderId = renamingReferenceBoardFolderId.value
  if (folderId === null) return
  const name = renamingReferenceBoardFolderName.value.trim()
  if (!name) {
    cancelReferenceBoardFolderRename()
    return
  }
  const current = library.value.referenceBoardFolders.find((item) => item.id === folderId)
  if (!current) {
    cancelReferenceBoardFolderRename()
    return
  }
  if (name === current.name) {
    cancelReferenceBoardFolderRename()
    return
  }

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('rename_reference_board_folder_command', {
      folderId,
      name,
    })
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    cancelReferenceBoardFolderRename()
  }
}

function onReferenceBoardFolderRenameEnter(event: KeyboardEvent) {
  event.preventDefault()
  if (isComposingReferenceBoardFolderRename.value) return
  void commitReferenceBoardFolderRename()
}

async function deleteReferenceBoardFolder(folderId: number) {
  const folder = library.value.referenceBoardFolders.find((item) => item.id === folderId)
  if (!folder) return
  if (!window.confirm(`删除文件夹“${folder.name}”？`)) return

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('delete_reference_board_folder_command', {
      folderId,
    })
    const next = new Set(expandedReferenceBoardFolderIds.value)
    next.delete(folderId)
    expandedReferenceBoardFolderIds.value = next
  } catch (error) {
    errorText.value = formatError(error)
  }
  closeBoardContextMenu()
}

function cancelReferenceBoardRename() {
  renamingReferenceBoardId.value = null
  renamingReferenceBoardName.value = ''
  isComposingReferenceBoardRename.value = false
}

function startComposingReferenceBoardRename() {
  isComposingReferenceBoardRename.value = true
}

function endComposingReferenceBoardRename() {
  isComposingReferenceBoardRename.value = false
}

function setRenamingReferenceBoardName(value: string) {
  renamingReferenceBoardName.value = value
}

function startReferenceBoardRename(boardId: number) {
  const board = library.value.referenceBoards.find((item) => item.id === boardId)
  if (!board) return
  renamingReferenceBoardId.value = boardId
  renamingReferenceBoardName.value = board.name
  isComposingReferenceBoardRename.value = false
  closeBoardContextMenu()
  void nextTick(() => {
    const input = document.querySelector<HTMLInputElement>(`[data-reference-board-rename-id="${boardId}"]`)
    input?.focus()
    input?.select()
  })
}

async function commitReferenceBoardRename() {
  if (isComposingReferenceBoardRename.value) return
  const boardId = renamingReferenceBoardId.value
  if (boardId === null) return
  const name = renamingReferenceBoardName.value.trim()
  if (!name) {
    cancelReferenceBoardRename()
    return
  }
  const current = library.value.referenceBoards.find((item) => item.id === boardId)
  if (!current) {
    cancelReferenceBoardRename()
    return
  }
  if (name === current.name) {
    cancelReferenceBoardRename()
    return
  }

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('rename_reference_board_command', {
      boardId,
      name,
    })
  } catch (error) {
    errorText.value = formatError(error)
  } finally {
    cancelReferenceBoardRename()
  }
}

function onReferenceBoardRenameEnter(event: KeyboardEvent) {
  event.preventDefault()
  if (isComposingReferenceBoardRename.value) return
  void commitReferenceBoardRename()
}

async function deleteReferenceBoard(boardId: number) {
  const board = library.value.referenceBoards.find((item) => item.id === boardId)
  if (!board) return
  if (!window.confirm(`删除参考板“${board.name}”？`)) return

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    library.value = await invoke<LibraryStore>('delete_reference_board_command', {
      boardId,
    })
    if (activeReferenceBoardId.value === boardId) {
      activeReferenceBoardId.value = null
      viewMode.value = 'gallery'
    }
  } catch (error) {
    errorText.value = formatError(error)
  }
  closeBoardContextMenu()
}

function cloneBoardItemLayout(item: ReferenceBoardItem): BoardItemLayout {
  return {
    x: item.x,
    y: item.y,
    width: item.width,
    height: item.height,
    rotation: item.rotation,
  }
}

function boardLayoutEquals(a: BoardItemLayout, b: BoardItemLayout) {
  const epsilon = 0.001
  return (
    Math.abs(a.x - b.x) <= epsilon &&
    Math.abs(a.y - b.y) <= epsilon &&
    Math.abs(a.width - b.width) <= epsilon &&
    Math.abs(a.height - b.height) <= epsilon &&
    Math.abs(a.rotation - b.rotation) <= epsilon
  )
}

function collectBoardLayoutMap(boardId: number) {
  const map = new Map<number, BoardItemLayout>()
  for (const item of library.value.referenceBoardItems) {
    if (item.boardId !== boardId) continue
    map.set(item.id, cloneBoardItemLayout(item))
  }
  return map
}

function buildBoardHistoryChanges(
  beforeMap: Map<number, BoardItemLayout>,
  afterMap: Map<number, BoardItemLayout>,
) {
  const changes: BoardHistoryChange[] = []
  for (const [itemId, before] of beforeMap) {
    const after = afterMap.get(itemId)
    if (!after) continue
    if (!boardLayoutEquals(before, after)) {
      changes.push({ itemId, before, after })
    }
  }
  return changes
}

function pushBoardHistory(entry: BoardHistoryEntry) {
  if (isApplyingBoardHistory.value) return
  if (entry.kind === 'layout' && entry.changes.length === 0) return
  if (entry.kind === 'delete' && entry.deletedItems.length === 0) return
  boardUndoStack.value.push(entry)
  if (boardUndoStack.value.length > 200) {
    boardUndoStack.value.splice(0, boardUndoStack.value.length - 200)
  }
  boardRedoStack.value = []
}

async function applyBoardHistorySnapshot(
  boardId: number,
  changes: BoardHistoryChange[],
  kind: 'before' | 'after',
  selectedItemId: number | null,
) {
  const targets: Array<{ change: BoardHistoryChange; item: ReferenceBoardItem }> = []
  for (const change of changes) {
    const item = library.value.referenceBoardItems.find((entry) => entry.id === change.itemId)
    if (!item || item.boardId !== boardId) continue
    targets.push({ change, item })
  }

  if (targets.length === 0) {
    selectedReferenceBoardItemId.value = null
    return
  }

  const { invoke } = await import('@tauri-apps/api/core')
  for (const { change, item } of targets) {
    const layout = kind === 'before' ? change.before : change.after
    library.value = await invoke<LibraryStore>('update_reference_board_item_layout_command', {
      itemId: item.id,
      x: layout.x,
      y: layout.y,
      width: layout.width,
      height: layout.height,
      rotation: layout.rotation,
    })
  }

  const hasSelected = selectedItemId !== null && library.value.referenceBoardItems.some((item) => item.id === selectedItemId)
  selectedReferenceBoardItemId.value = hasSelected ? selectedItemId : null
}

function snapshotDeletedBoardItems(itemIds: number[]) {
  const uniqueIds = [...new Set(itemIds)]
  const snapshots: DeletedBoardItemSnapshot[] = []
  for (const itemId of uniqueIds) {
    const item = library.value.referenceBoardItems.find((entry) => entry.id === itemId)
    if (!item) continue
    snapshots.push({
      itemId: item.id,
      boardId: item.boardId,
      imageId: item.imageId,
      layout: cloneBoardItemLayout(item),
      zIndex: item.zIndex,
    })
  }
  return snapshots
}

async function applyBoardDeleteUndo(entry: BoardDeleteHistoryEntry) {
  const { invoke } = await import('@tauri-apps/api/core')
  const restoredItemIds: number[] = []

  const deletedItems = [...entry.deletedItems].sort((a, b) => a.zIndex - b.zIndex || a.itemId - b.itemId)
  for (const deletedItem of deletedItems) {
    const beforeIds = new Set(
      library.value.referenceBoardItems.filter((item) => item.boardId === entry.boardId).map((item) => item.id),
    )
    library.value = await invoke<LibraryStore>('restore_reference_board_item_command', {
      boardId: entry.boardId,
      imageId: deletedItem.imageId,
      x: deletedItem.layout.x,
      y: deletedItem.layout.y,
      width: deletedItem.layout.width,
      height: deletedItem.layout.height,
      rotation: deletedItem.layout.rotation,
      zIndex: deletedItem.zIndex,
    })
    const created = library.value.referenceBoardItems.find(
      (item) => item.boardId === entry.boardId && !beforeIds.has(item.id),
    )
    if (!created) continue
    restoredItemIds.push(created.id)
  }

  entry.restoredItemIds = restoredItemIds
  ensureBoardCanvasBoundsFor(entry.boardId)
}

async function applyBoardDeleteRedo(entry: BoardDeleteHistoryEntry) {
  const { invoke } = await import('@tauri-apps/api/core')
  for (const itemId of entry.restoredItemIds) {
    if (!library.value.referenceBoardItems.some((item) => item.id === itemId && item.boardId === entry.boardId)) continue
    library.value = await invoke<LibraryStore>('remove_reference_board_item_command', {
      itemId,
    })
  }
  entry.restoredItemIds = []
}

async function undoReferenceBoardHistory() {
  if (boardUndoStack.value.length === 0 || isApplyingBoardHistory.value) return
  const entry = boardUndoStack.value.pop()
  if (!entry) return
  if (activeReferenceBoardId.value !== entry.boardId) return

  isApplyingBoardHistory.value = true
  try {
    if (entry.kind === 'layout') {
      await applyBoardHistorySnapshot(entry.boardId, entry.changes, 'before', entry.selectionBefore)
    } else {
      await applyBoardDeleteUndo(entry)
      const selectedId = entry.restoredItemIds[entry.restoredItemIds.length - 1] ?? null
      selectedReferenceBoardItemId.value = selectedId
    }
    boardRedoStack.value.push(entry)
  } catch (error) {
    errorText.value = formatError(error)
    boardUndoStack.value.push(entry)
  } finally {
    isApplyingBoardHistory.value = false
  }
}

async function redoReferenceBoardHistory() {
  if (boardRedoStack.value.length === 0 || isApplyingBoardHistory.value) return
  const entry = boardRedoStack.value.pop()
  if (!entry) return
  if (activeReferenceBoardId.value !== entry.boardId) return

  isApplyingBoardHistory.value = true
  try {
    if (entry.kind === 'layout') {
      await applyBoardHistorySnapshot(entry.boardId, entry.changes, 'after', entry.selectionAfter)
    } else {
      await applyBoardDeleteRedo(entry)
      selectedReferenceBoardItemId.value = null
    }
    boardUndoStack.value.push(entry)
  } catch (error) {
    errorText.value = formatError(error)
    boardRedoStack.value.push(entry)
  } finally {
    isApplyingBoardHistory.value = false
  }
}

type BoardWorldBounds = { minX: number; minY: number; maxX: number; maxY: number }

function boundsOfReferenceBoardItem(item: ReferenceBoardItem): BoardWorldBounds {
  const radians = (item.rotation * Math.PI) / 180
  const cos = Math.cos(radians)
  const sin = Math.sin(radians)
  const halfW = item.width / 2
  const halfH = item.height / 2
  const centerX = item.x + halfW
  const centerY = item.y + halfH
  const corners = [
    { x: -halfW, y: -halfH },
    { x: halfW, y: -halfH },
    { x: halfW, y: halfH },
    { x: -halfW, y: halfH },
  ]
  const rotated = corners.map(({ x, y }) => ({
    x: centerX + x * cos - y * sin,
    y: centerY + x * sin + y * cos,
  }))
  return {
    minX: Math.min(...rotated.map((point) => point.x)),
    minY: Math.min(...rotated.map((point) => point.y)),
    maxX: Math.max(...rotated.map((point) => point.x)),
    maxY: Math.max(...rotated.map((point) => point.y)),
  }
}

function mergeBoardBounds(bounds: BoardWorldBounds[]) {
  return bounds.reduce(
    (acc, entry) => ({
      minX: Math.min(acc.minX, entry.minX),
      minY: Math.min(acc.minY, entry.minY),
      maxX: Math.max(acc.maxX, entry.maxX),
      maxY: Math.max(acc.maxY, entry.maxY),
    }),
    { minX: Number.POSITIVE_INFINITY, minY: Number.POSITIVE_INFINITY, maxX: Number.NEGATIVE_INFINITY, maxY: Number.NEGATIVE_INFINITY },
  )
}

function setBoardCanvasBounds(boardId: number, bounds: BoardCanvasBounds) {
  boardCanvasBoundsById.value = {
    ...boardCanvasBoundsById.value,
    [boardId]: bounds,
  }
}

function ensureBoardCanvasBoundsFor(boardId: number) {
  const current = getBoardCanvasBounds(boardId)
  const items = library.value.referenceBoardItems.filter((item) => item.boardId === boardId)
  if (items.length === 0) {
    setBoardCanvasBounds(boardId, current)
    return
  }
  const itemsBounds = mergeBoardBounds(items.map(boundsOfReferenceBoardItem))
  const next: BoardCanvasBounds = {
    minX: Math.min(current.minX, itemsBounds.minX),
    minY: Math.min(current.minY, itemsBounds.minY),
    maxX: Math.max(current.maxX, itemsBounds.maxX),
    maxY: Math.max(current.maxY, itemsBounds.maxY),
  }
  if (
    next.minX !== current.minX ||
    next.minY !== current.minY ||
    next.maxX !== current.maxX ||
    next.maxY !== current.maxY
  ) {
    setBoardCanvasBounds(boardId, next)
  }
}

function ensureBoardCanvasBoundsForActiveBoard() {
  if (!activeReferenceBoard.value) return
  ensureBoardCanvasBoundsFor(activeReferenceBoard.value.id)
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

  const folderId = pickImportedLibraryFolderIdForImport()
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
  } finally {
    closeReferenceBoardCanvasMenu()
  }
}

function canImportReferenceBoardItemToLibrary(itemId: number) {
  const boardItem = library.value.referenceBoardItems.find((item) => item.id === itemId)
  if (!boardItem) return false
  const image = library.value.images.find((item) => item.id === boardItem.imageId)
  if (!image) return false
  return image.source === 'reference'
}

function pickImportedLibraryFolderIdForImport() {
  const folders = library.value.folders
  if (folders.length === 0) {
    errorText.value = '请先在设置中导入至少一个本地图库文件夹。'
    return null
  }
  if (folders.length === 1) {
    return folders[0].id
  }

  const options = folders.map((folder, index) => `${index + 1}. ${folder.path}`).join('\n')
  const raw = window.prompt(
    `选择要保存到的图库文件夹（输入序号）：\n${options}`,
    '1',
  )
  if (raw === null) return null

  const selectedIndex = Number.parseInt(raw.trim(), 10)
  if (!Number.isFinite(selectedIndex) || selectedIndex < 1 || selectedIndex > folders.length) {
    errorText.value = '无效选择，请输入列表中的序号。'
    return null
  }
  return folders[selectedIndex - 1].id
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

async function removeReferenceBoardItem(itemId: number) {
  await removeReferenceBoardItemsWithHistory([itemId])
}

async function removeReferenceBoardItemsWithHistory(itemIds: number[]) {
  const snapshots = snapshotDeletedBoardItems(itemIds)
  if (snapshots.length === 0) return
  const boardId = snapshots[0].boardId
  const allSameBoard = snapshots.every((item) => item.boardId === boardId)
  if (!allSameBoard) return
  const selectionBefore = selectedReferenceBoardItemId.value

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    for (const snapshot of snapshots) {
      library.value = await invoke<LibraryStore>('remove_reference_board_item_command', {
        itemId: snapshot.itemId,
      })
    }

    const removedIds = new Set(snapshots.map((item) => item.itemId))
    if (selectedReferenceBoardItemId.value !== null && removedIds.has(selectedReferenceBoardItemId.value)) {
      selectedReferenceBoardItemId.value = null
    }
    clearInternalBoardCopyRefForItems(removedIds)

    pushBoardHistory({
      kind: 'delete',
      boardId,
      deletedItems: snapshots,
      restoredItemIds: [],
      selectionBefore,
      selectionAfter: selectedReferenceBoardItemId.value,
    })

    closeReferenceBoardCanvasMenu()
  } catch (error) {
    errorText.value = formatError(error)
  }
}

function updateStatus() {
  const imageCount = library.value.images.length
  const folderCount = library.value.folders.length
  statusText.value =
    imageCount > 0 ? `${imageCount} 张图片，来自 ${folderCount} 个图库文件夹` : '还没有添加图库文件夹'
}

function updateViewportSize() {
  if (galleryEl.value) {
    const styles = window.getComputedStyle(galleryEl.value)
    const paddingLeft = Number.parseFloat(styles.paddingLeft || '0') || 0
    const paddingRight = Number.parseFloat(styles.paddingRight || '0') || 0
    viewportWidth.value = Math.max(320, galleryEl.value.clientWidth - paddingLeft - paddingRight)
  } else {
    viewportWidth.value = window.innerWidth
  }
  viewportHeight.value = window.innerHeight
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

function setGalleryElement(element: HTMLElement | null) {
  galleryEl.value = element
  galleryViewportHeight.value = element?.clientHeight ?? 0
  galleryScrollTop.value = element?.scrollTop ?? 0
  updateViewportSize()
}

function onGalleryScroll(scrollTop: number, clientHeight: number) {
  galleryScrollTop.value = scrollTop
  galleryViewportHeight.value = clientHeight
}

function onGalleryWheel(_event: WheelEvent) {}

function hideSearchPanel() {}

function setNewFolderName(value: string) {
  newFolderName.value = value
}

function setComposingFolderName(value: boolean) {
  isComposingFolderName.value = value
}

function setNewBoardName(value: string) {
  newBoardName.value = value
}

function setComposingBoardName(value: boolean) {
  isComposingBoardName.value = value
}

function openGalleryImageMenu(item: GalleryLayoutItem, event: MouseEvent) {
  event.preventDefault()
  event.stopPropagation()
  galleryImageContextMenu.value = {
    imageId: item.id,
    x: event.clientX,
    y: event.clientY,
  }
  imageDetailContextMenu.value = null
}

function openImageDetailMenu(event: MouseEvent) {
  if (!activeImageDetail.value) return
  event.preventDefault()
  event.stopPropagation()
  closeGalleryImageContextMenu()
  imageDetailContextMenu.value = { x: event.clientX, y: event.clientY }
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
  setAutoScanOnStartup,
  startScanAllFolders,
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
  setSearchConfidenceMin,
  setSearchConfidenceMax,
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

function shortestColumnIndex(heights: number[]) {
  let index = 0
  for (let i = 1; i < heights.length; i += 1) {
    if (heights[i] < heights[index]) index = i
  }
  return index
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
        'is-reference-preview-active': isReferencePreviewActive && rightSidebarPinned,
        'is-right-sidebar-fixed': rightSidebarPinned,
      }"
    >
      <SettingsView
        v-if="viewMode === 'settings'"
        :sidebar-pinned="sidebarPinned"
        :auto-fix-right-sidebar-on-preview="autoFixRightSidebarOnPreview"
        :auto-scan-on-startup="autoScanOnStartup"
        :is-background-scan-running="isBackgroundScanRunning"
        :scan-progress-text="scanProgressText"
        :scan-recent-errors="scanRecentErrors"
        :theme-mode="themeMode"
        :folder-path-input="folderPathInput"
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
        :search-zh-input="searchZhInput"
        :search-zh-selected="searchZhSelected"
        :search-zh-suggestions="searchZhSuggestions"
        :search-zh-open="searchZhOpen"
        :search-en-query="searchEnQuery"
        :search-file-name-query="searchFileNameQuery"
        :search-confidence-min="searchConfidenceMin"
        :search-confidence-max="searchConfidenceMax"
        :search-running="searchRunning"
        :search-error="searchError"
        :is-loading="isLoading"
        :layout-items="renderedLayoutItems"
        :total-height="totalHeight"
        :content-width="masonryContentWidth"
        :drag-state="dragState ? { imageId: dragState.imageId } : null"
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
