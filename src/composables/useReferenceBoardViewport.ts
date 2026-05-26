import { computed, ref, type Ref } from 'vue'

export type BoardCanvasBounds = {
  minX: number
  minY: number
  maxX: number
  maxY: number
}

type ReferenceBoardLike = {
  id: number
}

type ReferenceBoardItemLike = {
  boardId: number
  x: number
  y: number
  width: number
  height: number
  rotation: number
}

type BoardWorldBounds = { minX: number; minY: number; maxX: number; maxY: number }

type LibraryStoreLike = {
  referenceBoardItems: ReferenceBoardItemLike[]
}

type UseReferenceBoardViewportOptions<TLibraryStore extends LibraryStoreLike> = {
  library: Ref<TLibraryStore>
  activeReferenceBoardId: Ref<number | null>
  activeReferenceBoard: Ref<ReferenceBoardLike | null>
  defaultBoardCanvasWidth?: number
  defaultBoardCanvasHeight?: number
}

export function useReferenceBoardViewport<TLibraryStore extends LibraryStoreLike>(
  options: UseReferenceBoardViewportOptions<TLibraryStore>,
) {
  const boardCanvasBoundsById = ref<Record<number, BoardCanvasBounds>>({})

  const defaultBoardCanvasWidth = options.defaultBoardCanvasWidth ?? 1440
  const defaultBoardCanvasHeight = options.defaultBoardCanvasHeight ?? 960

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
    if (options.activeReferenceBoardId.value === null) {
      return createDefaultBoardCanvasBounds()
    }
    return getBoardCanvasBounds(options.activeReferenceBoardId.value)
  })

  function boundsOfReferenceBoardItem(item: ReferenceBoardItemLike): BoardWorldBounds {
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
      {
        minX: Number.POSITIVE_INFINITY,
        minY: Number.POSITIVE_INFINITY,
        maxX: Number.NEGATIVE_INFINITY,
        maxY: Number.NEGATIVE_INFINITY,
      },
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
    const items = options.library.value.referenceBoardItems.filter((item) => item.boardId === boardId)
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
    if (!options.activeReferenceBoard.value) return
    ensureBoardCanvasBoundsFor(options.activeReferenceBoard.value.id)
  }

  function syncBoardCanvasBounds(boardIds: Set<number>) {
    const nextBounds: Record<number, BoardCanvasBounds> = {}
    for (const boardId of boardIds) {
      nextBounds[boardId] = boardCanvasBoundsById.value[boardId] ?? createDefaultBoardCanvasBounds()
    }
    boardCanvasBoundsById.value = nextBounds
  }

  return {
    boardCanvasBoundsById,
    activeBoardCanvasBounds,
    createDefaultBoardCanvasBounds,
    getBoardCanvasBounds,
    boundsOfReferenceBoardItem,
    mergeBoardBounds,
    ensureBoardCanvasBoundsFor,
    ensureBoardCanvasBoundsForActiveBoard,
    syncBoardCanvasBounds,
  }
}
