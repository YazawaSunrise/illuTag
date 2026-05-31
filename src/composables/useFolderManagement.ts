import { computed, nextTick, ref, type Ref } from 'vue'
import type { GalleryImage } from '../types/gallery'

type UserFolderLike = {
  id: number
  parentId?: number | null
  name: string
  sortOrder: number
}

type ImageFolderAssignmentLike = {
  imageId: string
  folderId: number
}

type LibraryStoreLike = {
  images: GalleryImage[]
  userFolders: UserFolderLike[]
  imageFolders: ImageFolderAssignmentLike[]
}

type FolderTreeItem = UserFolderLike & {
  depth: number
  hasChildren: boolean
  isExpanded: boolean
}

type FolderContextMenu =
  | { kind: 'space'; x: number; y: number }
  | { kind: 'folder'; folderId: number; x: number; y: number }
  | null

type FolderDraft = {
  parentId: number | null
  x: number
  y: number
}

type FolderPointerState = {
  folderId: number
  pointerId: number
  startX: number
  startY: number
  currentX: number
  currentY: number
  isDragging: boolean
}

type UseFolderManagementOptions<TLibraryStore extends LibraryStoreLike> = {
  library: Ref<TLibraryStore>
  viewMode: Ref<'gallery' | 'settings' | 'board'>
  activeReferenceBoardId: Ref<number | null>
  setErrorText: (value: string) => void
  formatError: (error: unknown) => string
  updateStatus: () => void
  closeBoardContextMenu: () => void
  clamp: (value: number, min: number, max: number) => number
  folderDragDelayMs?: number
}

const defaultFolderDragDelayMs = 160

export function useFolderManagement<TLibraryStore extends LibraryStoreLike>(
  options: UseFolderManagementOptions<TLibraryStore>,
) {
  const activeUserFolderId = ref<number | 'all' | 'random' | 'favorites' | 'trash'>('all')
  const randomGalleryVisitSerial = ref(0)
  const unclassifiedOnlyParentFolderId = ref<number | null>(null)
  const newFolderName = ref('')
  const folderDraft = ref<FolderDraft | null>(null)
  const isComposingFolderName = ref(false)

  const expandedFolderIds = ref<Set<number>>(new Set())
  const dragExpandedFolderIds = ref<Set<number>>(new Set())
  const folderContextMenu = ref<FolderContextMenu>(null)

  const renamingUserFolderId = ref<number | null>(null)
  const renamingUserFolderName = ref('')
  const isComposingUserFolderRename = ref(false)

  const folderPressTimer = ref<number | null>(null)
  const folderPointerState = ref<FolderPointerState | null>(null)
  const draggedFolderId = ref<number | null>(null)
  const folderDragOverId = ref<number | null>(null)
  const suppressNextFolderClick = ref(false)

  const folderGroups = computed(() => {
    const byParent = new Map<number | null, UserFolderLike[]>()
    for (const folder of options.library.value.userFolders) {
      const key = folder.parentId ?? null
      const group = byParent.get(key) ?? []
      group.push(folder)
      byParent.set(key, group)
    }

    for (const group of byParent.values()) {
      group.sort(
        (a, b) =>
          (a.sortOrder ?? 0) - (b.sortOrder ?? 0) ||
          a.name.localeCompare(b.name, 'zh-Hans-CN') ||
          a.id - b.id,
      )
    }

    return byParent
  })

  const folderScopedImages = computed(() => {
    const galleryImages = options.library.value.images.filter((image) => image.source !== 'reference')
    if (activeUserFolderId.value === 'trash') {
      return galleryImages.filter((image) => image.trashed)
    }
    const activeImages = galleryImages.filter((image) => !image.trashed)
    if (activeUserFolderId.value === 'favorites') {
      return activeImages.filter((image) => image.isFavorite)
    }
    if (activeUserFolderId.value === 'all' || activeUserFolderId.value === 'random') return activeImages

    const scopeFolderIds = collectDescendantFolderIds(activeUserFolderId.value)
    const hasChildFolders = (folderGroups.value.get(activeUserFolderId.value) ?? []).length > 0
    const isUnclassifiedOnlyParentView =
      hasChildFolders && unclassifiedOnlyParentFolderId.value === activeUserFolderId.value
    if (!isUnclassifiedOnlyParentView) {
      const assignedIds = new Set(
        options.library.value.imageFolders
          .filter((assignment) => scopeFolderIds.has(assignment.folderId))
          .map((assignment) => assignment.imageId),
      )
      return activeImages.filter((image) => assignedIds.has(image.id))
    }

    const descendantFolderIds = new Set(scopeFolderIds)
    descendantFolderIds.delete(activeUserFolderId.value)

    const imageAssignedFolderIds = new Map<string, Set<number>>()
    for (const assignment of options.library.value.imageFolders) {
      const ids = imageAssignedFolderIds.get(assignment.imageId) ?? new Set<number>()
      ids.add(assignment.folderId)
      imageAssignedFolderIds.set(assignment.imageId, ids)
    }

    return activeImages.filter((image) => {
      const assignedFolderIds = imageAssignedFolderIds.get(image.id)
      if (!assignedFolderIds) return false
      if (!assignedFolderIds.has(activeUserFolderId.value as number)) return false
      for (const folderId of descendantFolderIds) {
        if (assignedFolderIds.has(folderId)) return false
      }
      return true
    })
  })

  const parentFoldersWithUnclassifiedImages = computed(() => {
    const activeImageIds = new Set(
      options.library.value.images
        .filter((image) => image.source !== 'reference' && !image.trashed)
        .map((image) => image.id),
    )

    const imageAssignedFolderIds = new Map<string, Set<number>>()
    for (const assignment of options.library.value.imageFolders) {
      if (!activeImageIds.has(assignment.imageId)) continue
      const ids = imageAssignedFolderIds.get(assignment.imageId) ?? new Set<number>()
      ids.add(assignment.folderId)
      imageAssignedFolderIds.set(assignment.imageId, ids)
    }

    const result = new Set<number>()
    for (const folder of options.library.value.userFolders) {
      const hasChildren = (folderGroups.value.get(folder.id) ?? []).length > 0
      if (!hasChildren) continue

      const scopeFolderIds = collectDescendantFolderIds(folder.id)
      scopeFolderIds.delete(folder.id)

      let hasUnclassified = false
      for (const assignedFolderIds of imageAssignedFolderIds.values()) {
        if (!assignedFolderIds.has(folder.id)) continue
        let hasDescendantAssignment = false
        for (const descendantId of scopeFolderIds) {
          if (assignedFolderIds.has(descendantId)) {
            hasDescendantAssignment = true
            break
          }
        }
        if (!hasDescendantAssignment) {
          hasUnclassified = true
          break
        }
      }

      if (hasUnclassified) result.add(folder.id)
    }
    return result
  })

  const folderTree = computed<FolderTreeItem[]>(() => {
    const mergedExpandedIds = new Set<number>(expandedFolderIds.value)
    for (const folderId of dragExpandedFolderIds.value) {
      mergedExpandedIds.add(folderId)
    }
    return buildFolderTree(mergedExpandedIds)
  })
  const dropFolderTree = computed<FolderTreeItem[]>(() => buildFolderTree(dragExpandedFolderIds.value))

  const contextMenuStyle = computed(() => {
    if (!folderContextMenu.value) return {}
    return {
      left: `${folderContextMenu.value.x}px`,
      top: `${folderContextMenu.value.y}px`,
    }
  })

  const folderDraftStyle = computed(() => {
    if (!folderDraft.value) return {}
    return {
      left: `${folderDraft.value.x}px`,
      top: `${folderDraft.value.y}px`,
    }
  })

  async function adjustFolderContextMenuPosition() {
    if (!folderContextMenu.value) return
    await nextTick()
    if (!folderContextMenu.value) return
    const menu = document.querySelector<HTMLElement>('.left-sidebar__context-menu')
    if (!menu) return
    const padding = 8
    const { width, height } = menu.getBoundingClientRect()
    const maxX = Math.max(padding, window.innerWidth - width - padding)
    const maxY = Math.max(padding, window.innerHeight - height - padding)
    const nextX = options.clamp(folderContextMenu.value.x, padding, maxX)
    const nextY = options.clamp(folderContextMenu.value.y, padding, maxY)
    if (nextX === folderContextMenu.value.x && nextY === folderContextMenu.value.y) return
    folderContextMenu.value = {
      ...folderContextMenu.value,
      x: nextX,
      y: nextY,
    }
  }

  function collectDescendantFolderIds(rootFolderId: number) {
    const childrenByParent = new Map<number, number[]>()
    for (const folder of options.library.value.userFolders) {
      if (folder.parentId == null) continue
      const group = childrenByParent.get(folder.parentId) ?? []
      group.push(folder.id)
      childrenByParent.set(folder.parentId, group)
    }

    const ids = new Set<number>()
    const stack = [rootFolderId]
    while (stack.length > 0) {
      const folderId = stack.pop()!
      if (ids.has(folderId)) continue
      ids.add(folderId)
      for (const childId of childrenByParent.get(folderId) ?? []) {
        stack.push(childId)
      }
    }
    return ids
  }

  function buildFolderTree(expandedIds: Set<number>) {
    const result: FolderTreeItem[] = []
    const append = (parentId: number | null, depth: number) => {
      for (const folder of folderGroups.value.get(parentId) ?? []) {
        const hasChildren = (folderGroups.value.get(folder.id) ?? []).length > 0
        const isExpanded = expandedIds.has(folder.id)
        result.push({ ...folder, depth, hasChildren, isExpanded })
        if (isExpanded) append(folder.id, depth + 1)
      }
    }
    append(null, 0)
    return result
  }

  async function createUserFolder(
    parentId: number | null = null,
    createOptions?: { name?: string; startRename?: boolean },
  ) {
    const name = (createOptions?.name ?? newFolderName.value).trim()
    if (!name) return

    const beforeIds = new Set(
      createOptions?.startRename ? options.library.value.userFolders.map((folder) => folder.id) : [],
    )
    options.setErrorText('')
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      options.library.value = await invoke<TLibraryStore>('create_user_folder_command', {
        parentId,
        name,
      })
      newFolderName.value = ''
      folderDraft.value = null
      if (parentId !== null) expandFolder(parentId)
      if (createOptions?.startRename) {
        const createdFolder = options.library.value.userFolders
          .filter((folder) => !beforeIds.has(folder.id) && (folder.parentId ?? null) === parentId)
          .sort((a, b) => b.id - a.id)[0]
        if (createdFolder) {
          startUserFolderRename(createdFolder.id)
        }
      }
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function reorderUserFolder(folderId: number, targetFolderId: number) {
    if (folderId === targetFolderId) return

    const dragged = options.library.value.userFolders.find((item) => item.id === folderId)
    const target = options.library.value.userFolders.find((item) => item.id === targetFolderId)
    if (!dragged || !target || (dragged.parentId ?? null) !== (target.parentId ?? null)) return

    options.setErrorText('')
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      options.library.value = await invoke<TLibraryStore>('reorder_user_folder_command', {
        folderId,
        targetFolderId,
      })
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function deleteUserFolder(folderId: number) {
    const folder = options.library.value.userFolders.find((item) => item.id === folderId)
    if (!folder) return
    closeFolderContextMenu()

    if (!window.confirm(`删除文件夹“${folder.name}”？子文件夹和图片归类关系也会一起移除。`)) {
      return
    }

    options.setErrorText('')
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      options.library.value = await invoke<TLibraryStore>('delete_user_folder_command', { folderId })
      removeExpandedFolder(folderId)
      if (unclassifiedOnlyParentFolderId.value === folderId) {
        unclassifiedOnlyParentFolderId.value = null
      }
      if (
        activeUserFolderId.value !== 'all' &&
        activeUserFolderId.value !== 'random' &&
        activeUserFolderId.value !== 'favorites' &&
        activeUserFolderId.value !== 'trash' &&
        !options.library.value.userFolders.some((item) => item.id === activeUserFolderId.value)
      ) {
        activeUserFolderId.value = 'all'
      }
      options.updateStatus()
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function openCreateFolderDraft(parentId: number | null, _x: number, _y: number) {
    closeFolderContextMenu()
    closeCreateFolderDraft()
    await createUserFolder(parentId, { name: '新建文件夹', startRename: true })
  }

  function closeCreateFolderDraft() {
    folderDraft.value = null
    newFolderName.value = ''
    isComposingFolderName.value = false
  }

  function commitFolderDraft() {
    if (isComposingFolderName.value) return
    if (!folderDraft.value) return
    void createUserFolder(folderDraft.value.parentId)
  }

  function toggleFolderExpanded(folderId: number) {
    const next = new Set(expandedFolderIds.value)
    if (next.has(folderId)) {
      next.delete(folderId)
    } else {
      next.add(folderId)
    }
    expandedFolderIds.value = next
  }

  function expandFolder(folderId: number) {
    const next = new Set(expandedFolderIds.value)
    next.add(folderId)
    expandedFolderIds.value = next
  }

  function removeExpandedFolder(folderId: number) {
    const next = new Set(expandedFolderIds.value)
    next.delete(folderId)
    expandedFolderIds.value = next
  }

  function openFolderSectionMenu(event: MouseEvent) {
    event.preventDefault()
    event.stopPropagation()
    folderContextMenu.value = { kind: 'space', x: event.clientX, y: event.clientY }
    options.closeBoardContextMenu()
    void adjustFolderContextMenuPosition()
  }

  function openFolderMenu(folderId: number, event: MouseEvent) {
    event.preventDefault()
    event.stopPropagation()
    folderContextMenu.value = { kind: 'folder', folderId, x: event.clientX, y: event.clientY }
    options.closeBoardContextMenu()
    void adjustFolderContextMenuPosition()
  }

  function closeFolderContextMenu() {
    folderContextMenu.value = null
  }

  function showAllImages() {
    options.viewMode.value = 'gallery'
    activeUserFolderId.value = 'all'
    unclassifiedOnlyParentFolderId.value = null
    options.activeReferenceBoardId.value = null
  }

  function showRandomImages() {
    options.viewMode.value = 'gallery'
    activeUserFolderId.value = 'random'
    randomGalleryVisitSerial.value += 1
    unclassifiedOnlyParentFolderId.value = null
    options.activeReferenceBoardId.value = null
  }

  function showFavoriteImages() {
    options.viewMode.value = 'gallery'
    activeUserFolderId.value = 'favorites'
    unclassifiedOnlyParentFolderId.value = null
    options.activeReferenceBoardId.value = null
  }

  function showTrashImages() {
    options.viewMode.value = 'gallery'
    activeUserFolderId.value = 'trash'
    unclassifiedOnlyParentFolderId.value = null
    options.activeReferenceBoardId.value = null
  }

  function onUserFolderRowClick(folder: FolderTreeItem) {
    if (suppressNextFolderClick.value) {
      suppressNextFolderClick.value = false
      return
    }

    if (activeUserFolderId.value === folder.id && folder.isExpanded) {
      removeExpandedFolder(folder.id)
    } else {
      expandFolder(folder.id)
    }

    options.viewMode.value = 'gallery'
    activeUserFolderId.value = folder.id
    unclassifiedOnlyParentFolderId.value = null
    options.activeReferenceBoardId.value = null
  }

  function toggleFolderUnclassifiedOnly(folderId: number) {
    if (!folderHasChildren(folderId)) return
    if (!parentFoldersWithUnclassifiedImages.value.has(folderId)) {
      if (unclassifiedOnlyParentFolderId.value === folderId) {
        unclassifiedOnlyParentFolderId.value = null
      }
      return
    }
    options.viewMode.value = 'gallery'
    activeUserFolderId.value = folderId
    options.activeReferenceBoardId.value = null
    expandFolder(folderId)
    unclassifiedOnlyParentFolderId.value =
      unclassifiedOnlyParentFolderId.value === folderId ? null : folderId
  }

  function startUserFolderRename(folderId: number) {
    const folder = options.library.value.userFolders.find((entry) => entry.id === folderId)
    if (!folder) return
    renamingUserFolderId.value = folderId
    renamingUserFolderName.value = folder.name
    isComposingUserFolderRename.value = false
    closeFolderContextMenu()
    void nextTick(() => {
      const input = document.querySelector<HTMLInputElement>(
        `[data-user-folder-rename-id="${folderId}"]`,
      )
      input?.focus()
      input?.select()
    })
  }

  function setRenamingUserFolderName(value: string) {
    renamingUserFolderName.value = value
  }

  function startComposingUserFolderRename() {
    isComposingUserFolderRename.value = true
  }

  function endComposingUserFolderRename() {
    isComposingUserFolderRename.value = false
  }

  function cancelUserFolderRename() {
    renamingUserFolderId.value = null
    renamingUserFolderName.value = ''
    isComposingUserFolderRename.value = false
  }

  async function commitUserFolderRename() {
    if (isComposingUserFolderRename.value) return
    const folderId = renamingUserFolderId.value
    if (folderId === null) return
    const name = renamingUserFolderName.value.trim()
    if (!name) {
      cancelUserFolderRename()
      return
    }
    const current = options.library.value.userFolders.find((entry) => entry.id === folderId)
    if (!current) {
      cancelUserFolderRename()
      return
    }
    if (name === current.name) {
      cancelUserFolderRename()
      return
    }

    options.setErrorText('')
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      options.library.value = await invoke<TLibraryStore>('rename_user_folder_command', {
        folderId,
        name,
      })
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      cancelUserFolderRename()
    }
  }

  function onUserFolderRenameEnter(event: KeyboardEvent) {
    event.preventDefault()
    if (isComposingUserFolderRename.value) return
    void commitUserFolderRename()
  }

  function clearFolderPress() {
    if (folderPressTimer.value !== null) {
      window.clearTimeout(folderPressTimer.value)
      folderPressTimer.value = null
    }
  }

  function sidebarFolderIdFromPoint(x: number, y: number) {
    const element = document.elementFromPoint(x, y)
    const folderElement = element?.closest<HTMLElement>('[data-sidebar-folder-id]')
    const folderId = folderElement?.dataset.sidebarFolderId
    return folderId ? Number(folderId) : null
  }

  function canReorderFolder(folderId: number, targetFolderId: number) {
    if (folderId === targetFolderId) return false
    const dragged = options.library.value.userFolders.find((item) => item.id === folderId)
    const target = options.library.value.userFolders.find((item) => item.id === targetFolderId)
    return Boolean(dragged && target && (dragged.parentId ?? null) === (target.parentId ?? null))
  }

  function startFolderPointer(folderId: number, event: PointerEvent) {
    if (event.button !== 0) return

    clearFolderPress()
    folderPointerState.value = {
      folderId,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      currentX: event.clientX,
      currentY: event.clientY,
      isDragging: false,
    }

    folderPressTimer.value = window.setTimeout(() => {
      const state = folderPointerState.value
      if (!state || state.folderId !== folderId || state.pointerId !== event.pointerId) return
      state.isDragging = true
      draggedFolderId.value = folderId
      suppressNextFolderClick.value = true
    }, options.folderDragDelayMs ?? defaultFolderDragDelayMs)
  }

  function moveFolderPointer(event: PointerEvent) {
    const state = folderPointerState.value
    if (!state || state.pointerId !== event.pointerId) return

    state.currentX = event.clientX
    state.currentY = event.clientY

    if (!state.isDragging) {
      const distance = Math.hypot(state.currentX - state.startX, state.currentY - state.startY)
      if (distance >= 6) {
        clearFolderPress()
        state.isDragging = true
        draggedFolderId.value = state.folderId
        suppressNextFolderClick.value = true
      } else {
        return
      }
    }

    const targetFolderId = sidebarFolderIdFromPoint(state.currentX, state.currentY)
    folderDragOverId.value =
      targetFolderId !== null && canReorderFolder(state.folderId, targetFolderId) ? targetFolderId : null
  }

  async function finishFolderPointer(event: PointerEvent) {
    const state = folderPointerState.value
    if (!state || state.pointerId !== event.pointerId) return

    clearFolderPress()
    const draggedId = state.folderId
    const targetFolderId = folderDragOverId.value
    const shouldReorder =
      state.isDragging && targetFolderId !== null && canReorderFolder(draggedId, targetFolderId)

    folderPointerState.value = null
    draggedFolderId.value = null
    folderDragOverId.value = null

    if (!shouldReorder) return
    try {
      await reorderUserFolder(draggedId, targetFolderId!)
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  function folderIdFromPoint(x: number, y: number) {
    const element = document.elementFromPoint(x, y)
    const sidebarFolderElement = element?.closest<HTMLElement>('[data-sidebar-folder-id]')
    const sidebarFolderId = sidebarFolderElement?.dataset.sidebarFolderId
    if (sidebarFolderId) return Number(sidebarFolderId)

    const folderElement = element?.closest<HTMLElement>('[data-folder-id]')
    const folderId = folderElement?.dataset.folderId
    return folderId ? Number(folderId) : null
  }

  function folderHasChildren(folderId: number) {
    return (folderGroups.value.get(folderId) ?? []).length > 0
  }

  function expandedDropFolderIdsFor(folderId: number) {
    const expandedIds = new Set<number>()
    let current = options.library.value.userFolders.find((folder) => folder.id === folderId)

    while (current?.parentId != null) {
      expandedIds.add(current.parentId)
      current = options.library.value.userFolders.find((folder) => folder.id === current?.parentId)
    }

    if (folderHasChildren(folderId)) expandedIds.add(folderId)
    return expandedIds
  }

  async function assignImageToFolder(imageId: string, folderId: number) {
    const { invoke } = await import('@tauri-apps/api/core')
    options.library.value = await invoke<TLibraryStore>('assign_image_to_user_folder_command', {
      imageId,
      folderId,
    })
  }

  return {
    activeUserFolderId,
    randomGalleryVisitSerial,
    unclassifiedOnlyParentFolderId,
    newFolderName,
    folderDraft,
    isComposingFolderName,
    expandedFolderIds,
    dragExpandedFolderIds,
    folderContextMenu,
    renamingUserFolderId,
    renamingUserFolderName,
    isComposingUserFolderRename,
    folderPressTimer,
    folderPointerState,
    draggedFolderId,
    folderDragOverId,
    suppressNextFolderClick,
    folderGroups,
    folderScopedImages,
    parentFoldersWithUnclassifiedImages,
    folderTree,
    dropFolderTree,
    contextMenuStyle,
    folderDraftStyle,
    createUserFolder,
    reorderUserFolder,
    deleteUserFolder,
    openCreateFolderDraft,
    closeCreateFolderDraft,
    commitFolderDraft,
    toggleFolderExpanded,
    expandFolder,
    removeExpandedFolder,
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
  }
}
