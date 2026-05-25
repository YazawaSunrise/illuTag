import { ref, type Ref } from 'vue'
import type { GalleryLayoutItem } from '../types/gallery'

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

type ReferenceBoardLike = {
  id: number
  folderId?: number | null
}

type LibraryStoreLike = {
  referenceBoards: ReferenceBoardLike[]
}

type UseImageDragAndDropOptions<TLibraryStore extends LibraryStoreLike> = {
  library: Ref<TLibraryStore>
  imageDragDelayMs: number
  dragExpandedFolderIds: Ref<Set<number>>
  folderIdFromPoint: (x: number, y: number) => number | null
  folderHasChildren: (folderId: number) => boolean
  expandedDropFolderIdsFor: (folderId: number) => Set<number>
  assignImageToFolder: (imageId: string, folderId: number) => Promise<void>
  referenceBoardIdFromPoint: (x: number, y: number) => number | null
  referenceBoardFolderIdFromPoint: (x: number, y: number) => number | null
  isPointInsideRightSidebarArea: (x: number, y: number) => boolean
  keepDragExpandedReferenceBoardFolder: (folderId: number) => void
  clearDragExpandedReferenceBoardFoldersNow: () => void
  clearDragReferenceBoardFolderCollapseTimer: () => void
  scheduleClearDragExpandedReferenceBoardFolders: (delayMs?: number) => void
  dragExpandedReferenceBoardFolderIds: Ref<Set<number>>
  addImageToReferenceBoard: (imageId: string, boardId: number) => Promise<void>
  expandReferenceBoardFolder: (folderId: number) => void
  setErrorText: (value: string) => void
  formatError: (error: unknown) => string
}

export function useImageDragAndDrop<TLibraryStore extends LibraryStoreLike>(
  options: UseImageDragAndDropOptions<TLibraryStore>,
) {
  const longPressTimer = ref<number | null>(null)
  const dragState = ref<DragState | null>(null)
  const pressedItem = ref<GalleryLayoutItem | null>(null)
  const pressedPointerId = ref<number | null>(null)
  const pressCurrent = ref<{ x: number; y: number } | null>(null)
  const lastImageDragEndedAt = ref(0)

  function floatingPanelPosition(x: number, y: number) {
    const panelWidth = 184
    const panelHeight = 260
    const side = x + panelWidth + 36 > window.innerWidth ? 'left' : 'right'
    return {
      x: side === 'right' ? x + 28 : x - panelWidth - 28,
      y: Math.max(84, Math.min(y - 40, window.innerHeight - panelHeight - 16)),
    }
  }

  function isPointInsideFolderDropPanel(x: number, y: number) {
    return Boolean(document.elementFromPoint(x, y)?.closest('.folder-drop-panel'))
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
    pressCurrent.value = null
    dragState.value = null
    options.dragExpandedFolderIds.value = new Set()
    options.clearDragExpandedReferenceBoardFoldersNow()
  }

  function startImagePress(item: GalleryLayoutItem, event: PointerEvent) {
    if (event.button !== 0) return

    clearImagePress()
    pressedItem.value = item
    pressedPointerId.value = event.pointerId
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
      options.dragExpandedFolderIds.value = new Set()
      options.clearDragExpandedReferenceBoardFoldersNow()
    }, options.imageDragDelayMs)
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
    const overFolderCandidateId = options.folderIdFromPoint(event.clientX, event.clientY)
    const overFolderId =
      overFolderCandidateId !== null && !options.folderHasChildren(overFolderCandidateId)
        ? overFolderCandidateId
        : null
    const overBoardId = options.referenceBoardIdFromPoint(event.clientX, event.clientY)
    const overBoardFolderId = options.referenceBoardFolderIdFromPoint(event.clientX, event.clientY)
    const overRightSidebar = options.isPointInsideRightSidebarArea(event.clientX, event.clientY)
    dragState.value.overFolderId = overFolderId
    dragState.value.overBoardId = overBoardId
    dragState.value.overRightSidebar = overRightSidebar

    if (overFolderCandidateId !== null) {
      options.dragExpandedFolderIds.value = options.expandedDropFolderIdsFor(overFolderCandidateId)
    } else if (!isPointInsideFolderDropPanel(event.clientX, event.clientY)) {
      options.dragExpandedFolderIds.value = new Set()
    }

    if (overBoardFolderId !== null) {
      options.keepDragExpandedReferenceBoardFolder(overBoardFolderId)
      return
    }

    const tempExpandedFolderId = [...options.dragExpandedReferenceBoardFolderIds.value][0]
    if (tempExpandedFolderId === undefined) return

    if (!overRightSidebar) {
      options.clearDragExpandedReferenceBoardFoldersNow()
      return
    }

    if (overBoardId !== null) {
      const board = options.library.value.referenceBoards.find((item) => item.id === overBoardId)
      if (board?.folderId === tempExpandedFolderId) {
        options.clearDragReferenceBoardFolderCollapseTimer()
        return
      }
    }

    options.scheduleClearDragExpandedReferenceBoardFolders()
  }

  async function finishImageDrag(event: PointerEvent) {
    clearImagePress()

    if (!dragState.value) {
      pressedItem.value = null
      return
    }

    try {
      const boardId =
        dragState.value.overBoardId ?? options.referenceBoardIdFromPoint(event.clientX, event.clientY)
      if (boardId !== null) {
        await options.addImageToReferenceBoard(dragState.value.imageId, boardId)
        const board = options.library.value.referenceBoards.find((item) => item.id === boardId)
        if (board?.folderId != null) {
          options.expandReferenceBoardFolder(board.folderId)
        }
      }

      if (!dragState.value) return
      const folderCandidateId =
        dragState.value.overFolderId ?? options.folderIdFromPoint(event.clientX, event.clientY)
      const folderId =
        folderCandidateId !== null && !options.folderHasChildren(folderCandidateId)
          ? folderCandidateId
          : null
      if (folderId !== null) {
        await options.assignImageToFolder(dragState.value.imageId, folderId)
      }
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      lastImageDragEndedAt.value = Date.now()
      cancelImageDrag()
    }
  }

  return {
    dragState,
    lastImageDragEndedAt,
    clearImagePress,
    cancelImageDrag,
    startImagePress,
    moveImageDrag,
    finishImageDrag,
  }
}
