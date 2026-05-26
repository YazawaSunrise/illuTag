import { computed, nextTick, ref, type Ref } from 'vue'

type ViewMode = 'gallery' | 'settings' | 'board'

type ReferenceBoardFolderLike = {
  id: number
  name: string
  sortOrder: number
}

type ReferenceBoardLike = {
  id: number
  folderId?: number | null
  name: string
  sortOrder: number
}

type ReferenceBoardItemLike = {
  id: number
  boardId: number
  imageId: string
  createdAt: number
}

type ImageLike = {
  id: string
  path: string
  thumbnailPath?: string | null
}

type LibraryStoreLike = {
  images: ImageLike[]
  referenceBoardFolders: ReferenceBoardFolderLike[]
  referenceBoards: ReferenceBoardLike[]
  referenceBoardItems: ReferenceBoardItemLike[]
}

type ReferenceBoardRow =
  | { kind: 'folder'; id: number; name: string; hasBoards: boolean; isExpanded: boolean }
  | { kind: 'board'; id: number; folderId: number | null; name: string; depth: number }

type BoardContextMenu =
  | { kind: 'space'; folderId: number | null; x: number; y: number }
  | { kind: 'folder'; folderId: number; x: number; y: number }
  | { kind: 'board'; boardId: number; x: number; y: number }
  | null

type BoardDraft = {
  kind: 'board' | 'folder'
  folderId: number | null
  x: number
  y: number
}

type UseReferenceBoardManagementOptions<TLibraryStore extends LibraryStoreLike> = {
  library: Ref<TLibraryStore>
  viewMode: Ref<ViewMode>
  activeReferenceBoardId: Ref<number | null>
  rightSidebarPinned: Ref<boolean>
  autoFixRightSidebarOnPreview: Ref<boolean>
  ensureBoardCanvasBoundsFor: (boardId: number) => void
  convertFileSrc: (path: string) => string
  setErrorText: (value: string) => void
  formatError: (error: unknown) => string
}

export function useReferenceBoardManagement<TLibraryStore extends LibraryStoreLike>(
  options: UseReferenceBoardManagementOptions<TLibraryStore>,
) {
  const expandedReferenceBoardFolderIds = ref<Set<number>>(new Set())
  const dragExpandedReferenceBoardFolderIds = ref<Set<number>>(new Set())
  const previewReferenceBoardIds = ref<Set<number>>(new Set())
  const boardContextMenu = ref<BoardContextMenu>(null)
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
  const dragReferenceBoardFolderCollapseTimer = ref<number | null>(null)

  const isReferencePreviewActive = computed(() => previewReferenceBoardIds.value.size > 0)

  const referenceBoardPreviewBlocks = computed(() => {
    const imageById = new Map(options.library.value.images.map((image) => [image.id, image]))
    const boardById = new Map(options.library.value.referenceBoards.map((board) => [board.id, board]))
    const rows: Array<{
      boardId: number
      name: string
      thumbnails: Array<{ itemId: number; imageId: string; thumbnailUrl: string }>
    }> = []

    for (const boardId of previewReferenceBoardIds.value) {
      const board = boardById.get(boardId)
      if (!board) continue

      const thumbnails = options.library.value.referenceBoardItems
        .filter((item) => item.boardId === boardId)
        .sort((a, b) => b.createdAt - a.createdAt)
        .slice(0, 12)
        .map((item) => {
          const image = imageById.get(item.imageId)
          if (!image) return null
          return {
            itemId: item.id,
            imageId: item.imageId,
            thumbnailUrl: options.convertFileSrc(image.thumbnailPath || image.path),
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

  function buildReferenceBoardRows(expandedIds: Set<number>) {
    const result: ReferenceBoardRow[] = []
    const boardsByFolder = new Map<number | null, ReferenceBoardLike[]>()
    for (const board of options.library.value.referenceBoards) {
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

    const folders = [...options.library.value.referenceBoardFolders].sort(
      (a, b) =>
        (a.sortOrder ?? 0) - (b.sortOrder ?? 0) ||
        a.name.localeCompare(b.name, 'zh-Hans-CN') ||
        a.id - b.id,
    )
    for (const folder of folders) {
      const boards = boardsByFolder.get(folder.id) ?? []
      const isExpanded = expandedIds.has(folder.id)
      result.push({
        kind: 'folder',
        id: folder.id,
        name: folder.name,
        hasBoards: boards.length > 0,
        isExpanded,
      })
      if (isExpanded) {
        for (const board of boards) {
          result.push({ kind: 'board', id: board.id, folderId: folder.id, name: board.name, depth: 1 })
        }
      }
    }

    return result
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

  function onReferenceBoardFolderRowClick(folderId: number) {
    if (renamingReferenceBoardFolderId.value === folderId) return
    toggleReferenceBoardFolderExpanded(folderId)
  }

  function showReferenceBoard(boardId: number) {
    options.activeReferenceBoardId.value = boardId
    options.ensureBoardCanvasBoundsFor(boardId)
    options.viewMode.value = 'board'
  }

  function closeBoardContextMenu() {
    boardContextMenu.value = null
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

  function clearReferenceBoardDragState() {
    draggedReferenceBoardId.value = null
    draggedReferenceBoardFolderId.value = null
    referenceBoardDragOverKind.value = null
    referenceBoardDragOverId.value = null
  }

  async function reorderReferenceBoardFolder(folderId: number, targetFolderId: number) {
    if (folderId === targetFolderId) return
    const { invoke } = await import('@tauri-apps/api/core')
    options.library.value = await invoke<TLibraryStore>('reorder_reference_board_folder_command', {
      folderId,
      targetFolderId,
    })
  }

  async function moveReferenceBoardToFolder(boardId: number, folderId: number | null) {
    const { invoke } = await import('@tauri-apps/api/core')
    options.library.value = await invoke<TLibraryStore>('move_reference_board_to_folder_command', {
      boardId,
      folderId,
    })
  }

  async function reorderReferenceBoard(boardId: number, targetBoardId: number) {
    if (boardId === targetBoardId) return
    const { invoke } = await import('@tauri-apps/api/core')
    options.library.value = await invoke<TLibraryStore>('reorder_reference_board_command', {
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
      options.setErrorText(options.formatError(error))
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
      options.setErrorText(options.formatError(error))
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
      options.setErrorText(options.formatError(error))
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
      if (options.autoFixRightSidebarOnPreview.value && !options.rightSidebarPinned.value) {
        options.rightSidebarPinned.value = true
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

  function setNewBoardName(value: string) {
    newBoardName.value = value
  }

  function setComposingBoardName(value: boolean) {
    isComposingBoardName.value = value
  }

  async function commitBoardDraft() {
    if (isComposingBoardName.value || !boardDraft.value) return
    const name = newBoardName.value.trim()
    if (!name) return

    try {
      const { invoke } = await import('@tauri-apps/api/core')
      if (boardDraft.value.kind === 'folder') {
        options.library.value = await invoke<TLibraryStore>('create_reference_board_folder_command', {
          name,
        })
      } else {
        options.library.value = await invoke<TLibraryStore>('create_reference_board_command', {
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
      options.setErrorText(options.formatError(error))
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
    const folder = options.library.value.referenceBoardFolders.find((item) => item.id === folderId)
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
    const current = options.library.value.referenceBoardFolders.find((item) => item.id === folderId)
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
      options.library.value = await invoke<TLibraryStore>('rename_reference_board_folder_command', {
        folderId,
        name,
      })
    } catch (error) {
      options.setErrorText(options.formatError(error))
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
    const folder = options.library.value.referenceBoardFolders.find((item) => item.id === folderId)
    if (!folder) return
    if (!window.confirm(`删除文件夹“${folder.name}”？`)) return

    try {
      const { invoke } = await import('@tauri-apps/api/core')
      options.library.value = await invoke<TLibraryStore>('delete_reference_board_folder_command', {
        folderId,
      })
      const next = new Set(expandedReferenceBoardFolderIds.value)
      next.delete(folderId)
      expandedReferenceBoardFolderIds.value = next
    } catch (error) {
      options.setErrorText(options.formatError(error))
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
    const board = options.library.value.referenceBoards.find((item) => item.id === boardId)
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
    const current = options.library.value.referenceBoards.find((item) => item.id === boardId)
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
      options.library.value = await invoke<TLibraryStore>('rename_reference_board_command', {
        boardId,
        name,
      })
    } catch (error) {
      options.setErrorText(options.formatError(error))
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
    const board = options.library.value.referenceBoards.find((item) => item.id === boardId)
    if (!board) return
    if (!window.confirm(`删除参考板“${board.name}”？`)) return

    try {
      const { invoke } = await import('@tauri-apps/api/core')
      options.library.value = await invoke<TLibraryStore>('delete_reference_board_command', {
        boardId,
      })
      if (options.activeReferenceBoardId.value === boardId) {
        options.activeReferenceBoardId.value = null
        options.viewMode.value = 'gallery'
      }
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
    closeBoardContextMenu()
  }

  return {
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
  }
}
