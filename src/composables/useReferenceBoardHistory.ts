import { computed, ref, type Ref } from 'vue'

type BoardItemLayout = {
  x: number
  y: number
  width: number
  height: number
  rotation: number
  flipX: boolean
  flipY: boolean
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

export type BoardHistoryEntry = BoardLayoutHistoryEntry | BoardDeleteHistoryEntry

type ReferenceBoardItemLike = {
  id: number
  boardId: number
  imageId: string
  x: number
  y: number
  width: number
  height: number
  rotation: number
  flipX: boolean
  flipY: boolean
  zIndex: number
}

type LibraryStoreLike = {
  referenceBoardItems: ReferenceBoardItemLike[]
}

type UseReferenceBoardHistoryOptions<TLibraryStore extends LibraryStoreLike> = {
  library: Ref<TLibraryStore>
  activeReferenceBoardId: Ref<number | null>
  selectedReferenceBoardItemId: Ref<number | null>
  ensureBoardCanvasBoundsFor: (boardId: number) => void
  clearInternalBoardCopyRefForItems: (itemIds: Set<number>) => void
  closeReferenceBoardCanvasMenu: () => void
  setErrorText: (value: string) => void
  formatError: (error: unknown) => string
  maxHistoryLength?: number
}

type BoardHistoryState = {
  entries: BoardHistoryEntry[]
  index: number
}

export function useReferenceBoardHistory<TLibraryStore extends LibraryStoreLike>(
  options: UseReferenceBoardHistoryOptions<TLibraryStore>,
) {
  const boardHistoryByBoardId = ref<Record<number, BoardHistoryState>>({})
  const boardHistory = computed(() => {
    const boardId = options.activeReferenceBoardId.value
    if (boardId === null) return []
    return getBoardHistoryState(boardId)?.entries ?? []
  })
  const boardHistoryIndex = computed(() => {
    const boardId = options.activeReferenceBoardId.value
    if (boardId === null) return -1
    return getBoardHistoryState(boardId)?.index ?? -1
  })
  const isApplyingBoardHistory = ref(false)
  const maxHistoryLength = options.maxHistoryLength ?? 200

  function getBoardHistoryState(boardId: number) {
    return boardHistoryByBoardId.value[boardId]
  }

  function ensureBoardHistoryState(boardId: number) {
    const existing = getBoardHistoryState(boardId)
    if (existing) return existing
    const created: BoardHistoryState = { entries: [], index: -1 }
    boardHistoryByBoardId.value[boardId] = created
    return created
  }

  function resetBoardHistory(boardId?: number) {
    const targetBoardId = boardId ?? options.activeReferenceBoardId.value
    if (targetBoardId === null || targetBoardId === undefined) return
    boardHistoryByBoardId.value[targetBoardId] = { entries: [], index: -1 }
  }

  function pruneBoardHistory(validBoardIds: Set<number>) {
    const next: Record<number, BoardHistoryState> = {}
    for (const [rawBoardId, state] of Object.entries(boardHistoryByBoardId.value)) {
      const boardId = Number(rawBoardId)
      if (!Number.isFinite(boardId) || !validBoardIds.has(boardId)) continue
      next[boardId] = state
    }
    boardHistoryByBoardId.value = next
  }

  function pushBoardHistory(entry: BoardHistoryEntry) {
    if (isApplyingBoardHistory.value) return
    if (entry.kind === 'layout' && entry.changes.length === 0) return
    if (entry.kind === 'delete' && entry.deletedItems.length === 0) return

    const state = ensureBoardHistoryState(entry.boardId)
    if (state.index < state.entries.length - 1) {
      state.entries = state.entries.slice(0, state.index + 1)
    }

    state.entries.push(entry)
    if (state.entries.length > maxHistoryLength) {
      const overflow = state.entries.length - maxHistoryLength
      state.entries.splice(0, overflow)
    }
    state.index = state.entries.length - 1
  }

  function cloneBoardItemLayout(item: ReferenceBoardItemLike): BoardItemLayout {
    return {
      x: item.x,
      y: item.y,
      width: item.width,
      height: item.height,
      rotation: item.rotation,
      flipX: item.flipX,
      flipY: item.flipY,
    }
  }

  function boardLayoutEquals(a: BoardItemLayout, b: BoardItemLayout) {
    const epsilon = 0.001
    return (
      Math.abs(a.x - b.x) <= epsilon &&
      Math.abs(a.y - b.y) <= epsilon &&
      Math.abs(a.width - b.width) <= epsilon &&
      Math.abs(a.height - b.height) <= epsilon &&
      Math.abs(a.rotation - b.rotation) <= epsilon &&
      a.flipX === b.flipX &&
      a.flipY === b.flipY
    )
  }

  function collectBoardLayoutMap(boardId: number) {
    const map = new Map<number, BoardItemLayout>()
    for (const item of options.library.value.referenceBoardItems) {
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

  async function applyBoardHistorySnapshot(
    boardId: number,
    changes: BoardHistoryChange[],
    kind: 'before' | 'after',
    selectedItemId: number | null,
  ) {
    const targets: Array<{ change: BoardHistoryChange; item: ReferenceBoardItemLike }> = []
    for (const change of changes) {
      const item = options.library.value.referenceBoardItems.find((entry) => entry.id === change.itemId)
      if (!item || item.boardId !== boardId) continue
      targets.push({ change, item })
    }

    if (targets.length === 0) {
      options.selectedReferenceBoardItemId.value = null
      return
    }

    const { invoke } = await import('@tauri-apps/api/core')
    for (const { change, item } of targets) {
      const layout = kind === 'before' ? change.before : change.after
      options.library.value = await invoke<TLibraryStore>('update_reference_board_item_layout_command', {
        itemId: item.id,
        x: layout.x,
        y: layout.y,
        width: layout.width,
        height: layout.height,
        rotation: layout.rotation,
        flipX: layout.flipX,
        flipY: layout.flipY,
      })
    }

    const hasSelected =
      selectedItemId !== null &&
      options.library.value.referenceBoardItems.some((item) => item.id === selectedItemId)
    options.selectedReferenceBoardItemId.value = hasSelected ? selectedItemId : null
  }

  function snapshotDeletedBoardItems(itemIds: number[]) {
    const snapshots: DeletedBoardItemSnapshot[] = []
    const uniqueIds = [...new Set(itemIds)]
    for (const itemId of uniqueIds) {
      const item = options.library.value.referenceBoardItems.find((entry) => entry.id === itemId)
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
        options.library.value.referenceBoardItems
          .filter((item) => item.boardId === entry.boardId)
          .map((item) => item.id),
      )
      options.library.value = await invoke<TLibraryStore>('restore_reference_board_item_command', {
        boardId: entry.boardId,
        imageId: deletedItem.imageId,
        x: deletedItem.layout.x,
        y: deletedItem.layout.y,
        width: deletedItem.layout.width,
        height: deletedItem.layout.height,
        rotation: deletedItem.layout.rotation,
        flipX: deletedItem.layout.flipX,
        flipY: deletedItem.layout.flipY,
        zIndex: deletedItem.zIndex,
      })
      const created = options.library.value.referenceBoardItems.find(
        (item) => item.boardId === entry.boardId && !beforeIds.has(item.id),
      )
      if (!created) continue
      restoredItemIds.push(created.id)
    }

    entry.restoredItemIds = restoredItemIds
    options.ensureBoardCanvasBoundsFor(entry.boardId)
  }

  async function applyBoardDeleteRedo(entry: BoardDeleteHistoryEntry) {
    const { invoke } = await import('@tauri-apps/api/core')
    for (const itemId of entry.restoredItemIds) {
      if (
        !options.library.value.referenceBoardItems.some(
          (item) => item.id === itemId && item.boardId === entry.boardId,
        )
      ) {
        continue
      }
      options.library.value = await invoke<TLibraryStore>('remove_reference_board_item_command', {
        itemId,
      })
    }
    entry.restoredItemIds = []
  }

  async function undoReferenceBoardHistory() {
    const boardId = options.activeReferenceBoardId.value
    if (boardId === null || isApplyingBoardHistory.value) return
    const state = getBoardHistoryState(boardId)
    if (!state || state.index < 0) return
    const entry = state.entries[state.index]
    if (!entry) return

    isApplyingBoardHistory.value = true
    try {
      if (entry.kind === 'layout') {
        await applyBoardHistorySnapshot(entry.boardId, entry.changes, 'before', entry.selectionBefore)
      } else {
        await applyBoardDeleteUndo(entry)
        const selectedId = entry.restoredItemIds[entry.restoredItemIds.length - 1] ?? null
        options.selectedReferenceBoardItemId.value = selectedId
      }
      state.index -= 1
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      isApplyingBoardHistory.value = false
    }
  }

  async function redoReferenceBoardHistory() {
    const boardId = options.activeReferenceBoardId.value
    if (boardId === null || isApplyingBoardHistory.value) return
    const state = getBoardHistoryState(boardId)
    if (!state) return
    const nextIndex = state.index + 1
    if (nextIndex >= state.entries.length) return
    const entry = state.entries[nextIndex]
    if (!entry) return

    isApplyingBoardHistory.value = true
    try {
      if (entry.kind === 'layout') {
        await applyBoardHistorySnapshot(entry.boardId, entry.changes, 'after', entry.selectionAfter)
      } else {
        await applyBoardDeleteRedo(entry)
        options.selectedReferenceBoardItemId.value = null
      }
      state.index = nextIndex
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      isApplyingBoardHistory.value = false
    }
  }

  async function removeReferenceBoardItemsWithHistory(itemIds: number[]) {
    const snapshots = snapshotDeletedBoardItems(itemIds)
    if (snapshots.length === 0) return
    const boardId = snapshots[0].boardId
    const allSameBoard = snapshots.every((item) => item.boardId === boardId)
    if (!allSameBoard) return
    const selectionBefore = options.selectedReferenceBoardItemId.value

    try {
      const { invoke } = await import('@tauri-apps/api/core')
      for (const snapshot of snapshots) {
        options.library.value = await invoke<TLibraryStore>('remove_reference_board_item_command', {
          itemId: snapshot.itemId,
        })
      }

      const removedIds = new Set(snapshots.map((item) => item.itemId))
      if (
        options.selectedReferenceBoardItemId.value !== null &&
        removedIds.has(options.selectedReferenceBoardItemId.value)
      ) {
        options.selectedReferenceBoardItemId.value = null
      }
      options.clearInternalBoardCopyRefForItems(removedIds)

      pushBoardHistory({
        kind: 'delete',
        boardId,
        deletedItems: snapshots,
        restoredItemIds: [],
        selectionBefore,
        selectionAfter: options.selectedReferenceBoardItemId.value,
      })

      options.closeReferenceBoardCanvasMenu()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function removeReferenceBoardItem(itemId: number) {
    await removeReferenceBoardItemsWithHistory([itemId])
  }

  return {
    boardHistory,
    boardHistoryIndex,
    isApplyingBoardHistory,
    resetBoardHistory,
    pruneBoardHistory,
    pushBoardHistory,
    collectBoardLayoutMap,
    buildBoardHistoryChanges,
    undoReferenceBoardHistory,
    redoReferenceBoardHistory,
    removeReferenceBoardItem,
    removeReferenceBoardItemsWithHistory,
  }
}
