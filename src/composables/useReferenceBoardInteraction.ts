import { ref, type Ref } from 'vue'

type ReferenceBoard = {
  id: number
}

type ReferenceBoardItem = {
  id: number
  boardId: number
  x: number
  y: number
  width: number
  height: number
  rotation: number
}

type LibraryStoreLike = {
  referenceBoardItems: ReferenceBoardItem[]
}

type ReferenceBoardCanvasMenu =
  | { kind: 'item'; itemId: number; x: number; y: number }
  | { kind: 'canvas'; x: number; y: number; worldX: number; worldY: number }
  | null

type BoardItemInteraction = {
  itemId: number
  mode: 'move' | 'resize' | 'rotate' | 'pan'
  pointerId: number
  startX: number
  startY: number
  itemX: number
  itemY: number
  itemWidth: number
  itemHeight: number
  itemRotation: number
  rotateStartAngle: number
  panX: number
  panY: number
}

type BoardItemLayout = {
  x: number
  y: number
  width: number
  height: number
  rotation: number
}

type UseReferenceBoardInteractionOptions<TLibraryStore extends LibraryStoreLike> = {
  library: Ref<TLibraryStore>
  activeReferenceBoard: Ref<ReferenceBoard | null>
  viewMode: Ref<string>
  ensureBoardCanvasBoundsFor: (boardId: number) => void
  setErrorText: (value: string) => void
  formatError: (error: unknown) => string
  clamp: (value: number, min: number, max: number) => number
  onLayoutHistory?: (payload: {
    boardId: number
    itemId: number
    before: BoardItemLayout
    after: BoardItemLayout
    selectionBefore: number | null
    selectionAfter: number | null
  }) => void
}

export function useReferenceBoardInteraction<TLibraryStore extends LibraryStoreLike>(
  options: UseReferenceBoardInteractionOptions<TLibraryStore>,
) {
  const boardScale = ref(1)
  const boardPan = ref({ x: 80, y: 72 })
  const boardInteraction = ref<BoardItemInteraction | null>(null)
  const selectedReferenceBoardItemId = ref<number | null>(null)
  const referenceBoardCanvasMenu = ref<ReferenceBoardCanvasMenu>(null)
  const lastBoardPointerWorld = ref<{ x: number; y: number; at: number } | null>(null)

  function closeReferenceBoardCanvasMenu() {
    referenceBoardCanvasMenu.value = null
  }

  function openReferenceBoardItemMenu(itemId: number, event: MouseEvent) {
    event.preventDefault()
    event.stopPropagation()
    rememberBoardPointerFromClient(event.clientX, event.clientY)
    selectedReferenceBoardItemId.value = itemId
    referenceBoardCanvasMenu.value = {
      kind: 'item',
      itemId,
      x: event.clientX,
      y: event.clientY,
    }
  }

  function openReferenceBoardCanvasMenu(event: MouseEvent) {
    if (options.viewMode.value !== 'board') return
    event.preventDefault()
    event.stopPropagation()

    const target = event.target as HTMLElement | null
    if (target?.closest('.reference-board-card')) return

    rememberBoardPointerFromClient(event.clientX, event.clientY)
    const worldPoint = worldPointFromClient(event.clientX, event.clientY)
    referenceBoardCanvasMenu.value = {
      kind: 'canvas',
      x: event.clientX,
      y: event.clientY,
      worldX: worldPoint.x,
      worldY: worldPoint.y,
    }
  }

  function getReferenceBoardViewportMetrics() {
    const page = document.querySelector<HTMLElement>('.reference-board-page')
    if (!page) return null
    const rect = page.getBoundingClientRect()
    const styles = window.getComputedStyle(page)
    const paddingLeft = Number.parseFloat(styles.paddingLeft || '0') || 0
    const paddingRight = Number.parseFloat(styles.paddingRight || '0') || 0
    const paddingTop = Number.parseFloat(styles.paddingTop || '0') || 0
    const paddingBottom = Number.parseFloat(styles.paddingBottom || '0') || 0
    return {
      left: rect.left + paddingLeft,
      top: rect.top + paddingTop,
      width: Math.max(1, page.clientWidth - paddingLeft - paddingRight),
      height: Math.max(1, page.clientHeight - paddingTop - paddingBottom),
    }
  }

  function worldPointFromClient(clientX: number, clientY: number) {
    const viewport = getReferenceBoardViewportMetrics()
    const scale = Math.max(0.001, boardScale.value)
    if (!viewport) {
      return {
        x: (clientX - boardPan.value.x) / scale,
        y: (clientY - boardPan.value.y) / scale,
      }
    }
    return {
      x: (clientX - viewport.left - boardPan.value.x) / scale,
      y: (clientY - viewport.top - boardPan.value.y) / scale,
    }
  }

  function rememberBoardPointerFromClient(clientX: number, clientY: number) {
    if (options.viewMode.value !== 'board' || !options.activeReferenceBoard.value) return
    const viewport = getReferenceBoardViewportMetrics()
    if (
      viewport &&
      (clientX < viewport.left ||
        clientX > viewport.left + viewport.width ||
        clientY < viewport.top ||
        clientY > viewport.top + viewport.height)
    ) {
      return
    }
    const world = worldPointFromClient(clientX, clientY)
    lastBoardPointerWorld.value = {
      x: world.x,
      y: world.y,
      at: Date.now(),
    }
  }

  function trackBoardPointer(event: PointerEvent) {
    rememberBoardPointerFromClient(event.clientX, event.clientY)
  }

  function zoomReferenceBoard(event: WheelEvent) {
    if (!options.activeReferenceBoard.value) return
    event.preventDefault()

    const viewport = getReferenceBoardViewportMetrics()
    const nextScale = options.clamp(boardScale.value * (event.deltaY < 0 ? 1.08 : 0.92), 0.2, 4)
    const worldPoint = worldPointFromClient(event.clientX, event.clientY)
    const baseLeft = viewport?.left ?? 0
    const baseTop = viewport?.top ?? 0
    boardScale.value = nextScale
    boardPan.value = {
      x: event.clientX - baseLeft - worldPoint.x * nextScale,
      y: event.clientY - baseTop - worldPoint.y * nextScale,
    }
  }

  function startBoardPan(event: PointerEvent) {
    if (event.button !== 0) return
    const target = event.target as HTMLElement | null
    if (target?.closest('.reference-board-card')) return
    closeReferenceBoardCanvasMenu()
    selectedReferenceBoardItemId.value = null

    boardInteraction.value = {
      itemId: -1,
      mode: 'pan',
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      itemX: 0,
      itemY: 0,
      itemWidth: 0,
      itemHeight: 0,
      itemRotation: 0,
      rotateStartAngle: 0,
      panX: boardPan.value.x,
      panY: boardPan.value.y,
    }
  }

  function moveBoardInteraction(event: PointerEvent) {
    const interaction = boardInteraction.value
    if (!interaction || interaction.pointerId !== event.pointerId) return

    if (interaction.mode === 'pan') {
      boardPan.value = {
        x: interaction.panX + (event.clientX - interaction.startX),
        y: interaction.panY + (event.clientY - interaction.startY),
      }
      return
    }

    const item = options.library.value.referenceBoardItems.find((entry) => entry.id === interaction.itemId)
    if (!item) return

    if (interaction.mode === 'rotate') {
      const centerX = interaction.itemX + interaction.itemWidth / 2
      const centerY = interaction.itemY + interaction.itemHeight / 2
      const worldPoint = worldPointFromClient(event.clientX, event.clientY)
      const angle = (Math.atan2(worldPoint.y - centerY, worldPoint.x - centerX) * 180) / Math.PI
      const nextRotation = interaction.itemRotation + (angle - interaction.rotateStartAngle)
      item.rotation = event.shiftKey ? Math.round(nextRotation / 45) * 45 : nextRotation
      return
    }

    const scale = Math.max(0.001, boardScale.value)
    const deltaX = (event.clientX - interaction.startX) / scale
    const deltaY = (event.clientY - interaction.startY) / scale

    if (interaction.mode === 'move') {
      item.x = interaction.itemX + deltaX
      item.y = interaction.itemY + deltaY
      return
    }

    const baseWidth = Math.max(1, interaction.itemWidth)
    const baseHeight = Math.max(1, interaction.itemHeight)
    const aspect = baseWidth / baseHeight
    const widthByX = baseWidth + deltaX
    const widthByY = (baseHeight + deltaY) * aspect
    const useY = Math.abs(deltaY / baseHeight) > Math.abs(deltaX / baseWidth)
    let nextWidth = useY ? widthByY : widthByX

    const minWidth = Math.max(56, 56 * aspect)
    const maxWidth = Math.min(4000, 4000 * aspect)
    if (maxWidth >= minWidth) {
      nextWidth = options.clamp(nextWidth, minWidth, maxWidth)
    }

    item.width = nextWidth
    item.height = nextWidth / aspect
  }

  async function finishBoardInteraction(event: PointerEvent) {
    const interaction = boardInteraction.value
    if (!interaction || interaction.pointerId !== event.pointerId) return
    boardInteraction.value = null

    if (interaction.mode === 'pan') return
    const item = options.library.value.referenceBoardItems.find((entry) => entry.id === interaction.itemId)
    if (!item) return
    const boardId = item.boardId
    const before: BoardItemLayout = {
      x: interaction.itemX,
      y: interaction.itemY,
      width: interaction.itemWidth,
      height: interaction.itemHeight,
      rotation: interaction.itemRotation,
    }
    const after = cloneBoardItemLayout(item)
    const selectionBefore = interaction.itemId
    const selectionAfter = selectedReferenceBoardItemId.value

    try {
      const { invoke } = await import('@tauri-apps/api/core')
      options.library.value = await invoke<TLibraryStore>('update_reference_board_item_layout_command', {
        itemId: item.id,
        x: item.x,
        y: item.y,
        width: item.width,
        height: item.height,
        rotation: item.rotation,
      })
      options.ensureBoardCanvasBoundsFor(boardId)
      if (!boardLayoutEquals(before, after)) {
        options.onLayoutHistory?.({
          boardId,
          itemId: item.id,
          before,
          after,
          selectionBefore,
          selectionAfter,
        })
      }
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  async function bringReferenceBoardItemToFront(itemId: number) {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      options.library.value = await invoke<TLibraryStore>('bring_reference_board_item_to_front_command', { itemId })
    } catch (error) {
      options.setErrorText(options.formatError(error))
    }
  }

  function startBoardItemMove(item: ReferenceBoardItem, event: PointerEvent) {
    if (event.button !== 0) return
    event.stopPropagation()
    event.preventDefault()
    selectedReferenceBoardItemId.value = item.id
    closeReferenceBoardCanvasMenu()
    void bringReferenceBoardItemToFront(item.id)

    boardInteraction.value = {
      itemId: item.id,
      mode: 'move',
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      itemX: item.x,
      itemY: item.y,
      itemWidth: item.width,
      itemHeight: item.height,
      itemRotation: item.rotation,
      rotateStartAngle: 0,
      panX: boardPan.value.x,
      panY: boardPan.value.y,
    }
  }

  function startBoardItemResize(item: ReferenceBoardItem, event: PointerEvent) {
    if (event.button !== 0) return
    if (selectedReferenceBoardItemId.value !== item.id) return
    event.stopPropagation()
    event.preventDefault()
    selectedReferenceBoardItemId.value = item.id
    closeReferenceBoardCanvasMenu()
    void bringReferenceBoardItemToFront(item.id)

    boardInteraction.value = {
      itemId: item.id,
      mode: 'resize',
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      itemX: item.x,
      itemY: item.y,
      itemWidth: item.width,
      itemHeight: item.height,
      itemRotation: item.rotation,
      rotateStartAngle: 0,
      panX: boardPan.value.x,
      panY: boardPan.value.y,
    }
  }

  function startBoardItemRotate(item: ReferenceBoardItem, event: PointerEvent) {
    if (event.button !== 0) return
    if (selectedReferenceBoardItemId.value !== item.id) return
    event.stopPropagation()
    event.preventDefault()
    closeReferenceBoardCanvasMenu()
    void bringReferenceBoardItemToFront(item.id)

    const centerX = item.x + item.width / 2
    const centerY = item.y + item.height / 2
    const worldPoint = worldPointFromClient(event.clientX, event.clientY)
    const rotateStartAngle = (Math.atan2(worldPoint.y - centerY, worldPoint.x - centerX) * 180) / Math.PI

    boardInteraction.value = {
      itemId: item.id,
      mode: 'rotate',
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      itemX: item.x,
      itemY: item.y,
      itemWidth: item.width,
      itemHeight: item.height,
      itemRotation: item.rotation,
      rotateStartAngle,
      panX: boardPan.value.x,
      panY: boardPan.value.y,
    }
  }

  function clearBoardInteraction() {
    boardInteraction.value = null
  }

  return {
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
  }
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
