import { ref, type Ref } from 'vue'

type PreviewBoardDragKind = 'preview' | 'board' | 'gallery' | null

export type PreviewBoardItemDragState = {
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

type LibraryStoreLike = Record<string, unknown>

type UsePreviewBoardDragOptions<TLibraryStore extends LibraryStoreLike> = {
  library: Ref<TLibraryStore>
  selectedReferenceBoardItemId: Ref<number | null>
  closeBoardContextMenu: () => void
  clearReferenceBoardDragState: () => void
  ensureBoardCanvasBoundsFor: (boardId: number) => void
  removeReferenceBoardItemsWithHistory: (itemIds: number[]) => Promise<void>
  clearInternalBoardCopyRefForItem: (itemId: number) => void
  showReferenceBoard: (boardId: number) => void
  setErrorText: (value: string) => void
  formatError: (error: unknown) => string
}

export function usePreviewBoardDrag<TLibraryStore extends LibraryStoreLike>(
  options: UsePreviewBoardDragOptions<TLibraryStore>,
) {
  const previewDragOverDeleteZone = ref(false)
  const previewBoardItemDrag = ref<PreviewBoardItemDragState | null>(null)
  const lastPreviewBoardDragEndedAt = ref(0)
  const previewBoardPointerId = ref<number | null>(null)

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
    options.showReferenceBoard(boardId)
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
    options.closeBoardContextMenu()
    options.clearReferenceBoardDragState()
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
    options.closeBoardContextMenu()
    options.clearReferenceBoardDragState()
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
        const boardRect = boardElement.getBoundingClientRect()
        const mode = event.clientX < boardRect.left + boardRect.width / 2 ? 'move' : 'copy'
        const kind: Exclude<PreviewBoardDragKind, 'gallery' | null> = boardElement.closest(
          '.reference-board-preview__block',
        )
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
        await options.removeReferenceBoardItemsWithHistory([state.itemId])
        return
      }

      if (state.targetBoardId !== null) {
        if (state.mode === 'copy') {
          options.library.value = await invoke<TLibraryStore>('add_image_to_reference_board_command', {
            imageId: state.imageId,
            boardId: state.targetBoardId,
          })
          options.ensureBoardCanvasBoundsFor(state.targetBoardId)
        } else if (state.sourceBoardId !== state.targetBoardId) {
          options.library.value = await invoke<TLibraryStore>('add_image_to_reference_board_command', {
            imageId: state.imageId,
            boardId: state.targetBoardId,
          })
          options.ensureBoardCanvasBoundsFor(state.targetBoardId)
          options.library.value = await invoke<TLibraryStore>('remove_reference_board_item_command', {
            itemId: state.itemId,
          })
          if (options.selectedReferenceBoardItemId.value === state.itemId) {
            options.selectedReferenceBoardItemId.value = null
          }
          options.clearInternalBoardCopyRefForItem(state.itemId)
        }
      }
    } catch (error) {
      options.setErrorText(options.formatError(error))
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
        options.library.value = await invoke<TLibraryStore>('add_image_to_reference_board_command', {
          imageId: state.imageId,
          boardId,
        })
        options.ensureBoardCanvasBoundsFor(boardId)
      } else if (state.sourceBoardId !== boardId) {
        options.library.value = await invoke<TLibraryStore>('add_image_to_reference_board_command', {
          imageId: state.imageId,
          boardId,
        })
        options.ensureBoardCanvasBoundsFor(boardId)
        options.library.value = await invoke<TLibraryStore>('remove_reference_board_item_command', {
          itemId: state.itemId,
        })
        if (options.selectedReferenceBoardItemId.value === state.itemId) {
          options.selectedReferenceBoardItemId.value = null
        }
        options.clearInternalBoardCopyRefForItem(state.itemId)
      }
    } catch (error) {
      options.setErrorText(options.formatError(error))
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
      await options.removeReferenceBoardItemsWithHistory([state.itemId])
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      clearPreviewBoardItemDrag()
    }
  }

  return {
    previewDragOverDeleteZone,
    previewBoardItemDrag,
    clearPreviewBoardItemDrag,
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
  }
}
