import type { Ref } from 'vue'
import { ref } from 'vue'

type ReferenceBoard = {
  id: number
}

type ReferenceBoardItem = {
  id: number
  boardId: number
  imageId: string
  width: number
  height: number
  rotation: number
}

type LibraryStoreLike = {
  referenceBoardItems: ReferenceBoardItem[]
}

type InternalBoardCopyRef = {
  itemId: number
  imageId: string
  width: number
  height: number
  rotation: number
  copiedAt: number
}

type ClipboardImagePayload = {
  imageBytes: number[]
  mimeType: string
}

type ViewportMetrics = {
  left: number
  top: number
  width: number
  height: number
}

type UseReferenceBoardClipboardOptions<TLibraryStore extends LibraryStoreLike> = {
  library: Ref<TLibraryStore>
  activeReferenceBoard: Ref<ReferenceBoard | null>
  selectedReferenceBoardItemId: Ref<number | null>
  boardPan: Ref<{ x: number; y: number }>
  boardScale: Ref<number>
  lastBoardPointerWorld: Ref<{ x: number; y: number; at: number } | null>
  boardPointerUseMaxAgeMs: number
  closeReferenceBoardCanvasMenu: () => void
  ensureBoardCanvasBoundsFor: (boardId: number) => void
  getReferenceBoardViewportMetrics: () => ViewportMetrics | null
  setErrorText: (value: string) => void
  formatError: (error: unknown) => string
  internalBoardCopyMaxAgeMs?: number
}

const defaultInternalBoardCopyMaxAgeMs = 10 * 60 * 1000

export function useReferenceBoardClipboard<TLibraryStore extends LibraryStoreLike>(
  options: UseReferenceBoardClipboardOptions<TLibraryStore>,
) {
  const clipboardWriteTask = ref<Promise<void> | null>(null)
  const internalBoardCopyRef = ref<InternalBoardCopyRef | null>(null)
  const internalBoardCopyMaxAgeMs =
    options.internalBoardCopyMaxAgeMs ?? defaultInternalBoardCopyMaxAgeMs

  async function copyReferenceBoardItemToClipboard(itemId: number) {
    const boardItem = options.library.value.referenceBoardItems.find((item) => item.id === itemId)
    if (!boardItem) return
    options.selectedReferenceBoardItemId.value = itemId
    internalBoardCopyRef.value = {
      itemId: boardItem.id,
      imageId: boardItem.imageId,
      width: boardItem.width,
      height: boardItem.height,
      rotation: boardItem.rotation,
      copiedAt: Date.now(),
    }
    options.closeReferenceBoardCanvasMenu()
    const task = copyImageToSystemClipboard(boardItem.imageId)
    clipboardWriteTask.value = task
    try {
      await task
    } catch (error) {
      options.setErrorText(buildClipboardCopyErrorText(error, '参考板复制'))
    } finally {
      if (clipboardWriteTask.value === task) {
        clipboardWriteTask.value = null
      }
    }
  }

  async function pasteReferenceBoardContent(worldX?: number, worldY?: number) {
    if (!options.activeReferenceBoard.value) return

    const pastePoint = resolveReferenceBoardPastePoint(worldX, worldY)
    try {
      if (clipboardWriteTask.value) {
        try {
          await clipboardWriteTask.value
        } catch {}
      }
      const { invoke } = await import('@tauri-apps/api/core')
      const clipboardImage = await readClipboardImageForReferenceBoard()
      if (clipboardImage) {
        options.library.value = await invoke<TLibraryStore>('paste_image_to_reference_board_command', {
          boardId: options.activeReferenceBoard.value.id,
          imageBytes: clipboardImage.imageBytes,
          mimeType: clipboardImage.mimeType,
          x: pastePoint.x,
          y: pastePoint.y,
        })
        options.ensureBoardCanvasBoundsFor(options.activeReferenceBoard.value.id)
        return
      }

      if (
        internalBoardCopyRef.value &&
        Date.now() - internalBoardCopyRef.value.copiedAt <= internalBoardCopyMaxAgeMs
      ) {
        const copied = internalBoardCopyRef.value
        const sourceStillExists = options.library.value.referenceBoardItems.some(
          (item) => item.id === copied.itemId,
        )
        if (sourceStillExists) {
          options.library.value = await invoke<TLibraryStore>('duplicate_reference_board_item_command', {
            itemId: copied.itemId,
            x: pastePoint.x,
            y: pastePoint.y,
          })
          options.ensureBoardCanvasBoundsFor(options.activeReferenceBoard.value.id)
          return
        }
        internalBoardCopyRef.value = null
      }

      options.setErrorText('剪贴板中没有可粘贴的图片。')
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      options.closeReferenceBoardCanvasMenu()
    }
  }

  function resolveReferenceBoardPastePoint(worldX?: number, worldY?: number) {
    if (Number.isFinite(worldX) && Number.isFinite(worldY)) {
      return { x: Number(worldX), y: Number(worldY) }
    }

    if (
      options.lastBoardPointerWorld.value &&
      Date.now() - options.lastBoardPointerWorld.value.at <= options.boardPointerUseMaxAgeMs
    ) {
      return {
        x: options.lastBoardPointerWorld.value.x,
        y: options.lastBoardPointerWorld.value.y,
      }
    }

    const viewport = options.getReferenceBoardViewportMetrics()
    const scale = Math.max(options.boardScale.value, 0.001)
    return {
      x: ((viewport?.width ?? window.innerWidth) * 0.5 - options.boardPan.value.x) / scale,
      y: ((viewport?.height ?? window.innerHeight) * 0.5 - options.boardPan.value.y) / scale,
    }
  }

  async function readClipboardImageForReferenceBoard(): Promise<ClipboardImagePayload | null> {
    if (!('clipboard' in navigator) || typeof navigator.clipboard.read !== 'function') {
      return null
    }

    try {
      const items = await navigator.clipboard.read()
      for (const item of items) {
        const type = item.types.find((entry) => entry.startsWith('image/'))
        if (!type) continue
        const blob = await item.getType(type)
        const buffer = await blob.arrayBuffer()
        return {
          imageBytes: Array.from(new Uint8Array(buffer)),
          mimeType: type,
        }
      }
      return null
    } catch {
      return null
    }
  }

  async function copyImageToSystemClipboard(imageId: string) {
    const { invoke } = await import('@tauri-apps/api/core')
    let backendError: unknown = null
    try {
      await invoke('copy_image_to_system_clipboard_command', { imageId })
      return
    } catch (error) {
      backendError = error
    }

    try {
      await copyImageToSystemClipboardByWebApi(imageId)
      return
    } catch (error) {
      const backendRaw = formatRawErrorNameMessage(backendError)
      const fallbackRaw = formatRawErrorNameMessage(error)
      throw new Error(
        [
          `后端系统剪贴板写入失败：${backendRaw}`,
          `Web Clipboard 回退失败：${fallbackRaw}`,
          `能力检测：${formatClipboardFeatureAvailability()}`,
        ].join('\n'),
      )
    }
  }

  async function copyImageToSystemClipboardByWebApi(imageId: string) {
    const { invoke } = await import('@tauri-apps/api/core')
    const payload = await invoke<{ bytes: number[]; mime_type?: string; mimeType?: string }>(
      'read_image_bytes_command',
      {
        imageId,
      },
    )
    const mimeType = payload.mime_type || payload.mimeType || 'image/png'
    const sourceBlob = new Blob([new Uint8Array(payload.bytes)], { type: mimeType })
    const features = clipboardFeatureAvailability()
    if (!features.hasClipboardWrite || !features.hasClipboardItem) {
      throw new Error(`Clipboard API 不可用：${formatClipboardFeatureAvailability(features)}`)
    }

    let convertError: unknown = null
    let pngBlob: Blob | null = null
    try {
      pngBlob = await convertImageBlobToPng(sourceBlob)
    } catch (error) {
      convertError = error
    }

    const data: Record<string, Blob> = {}
    if (pngBlob) {
      data['image/png'] = pngBlob
    } else {
      data[mimeType] = sourceBlob
    }

    try {
      await navigator.clipboard.write([new ClipboardItem(data)])
    } catch (error) {
      const rawWrite = formatRawErrorNameMessage(error)
      const rawConvert = convertError ? formatRawErrorNameMessage(convertError) : null
      throw new Error(
        [
          `系统剪贴板写入失败：${rawWrite}`,
          rawConvert ? `PNG 转换错误：${rawConvert}` : null,
        ]
          .filter(Boolean)
          .join('\n'),
      )
    }
  }

  async function convertImageBlobToPng(blob: Blob) {
    if (typeof createImageBitmap !== 'function') {
      return null
    }
    const bitmap = await createImageBitmap(blob)
    try {
      const canvas = document.createElement('canvas')
      canvas.width = bitmap.width
      canvas.height = bitmap.height
      const ctx = canvas.getContext('2d')
      if (!ctx) {
        throw new Error('Canvas 2D 上下文不可用')
      }
      ctx.drawImage(bitmap, 0, 0)
      return await new Promise<Blob | null>((resolve, reject) => {
        canvas.toBlob((png) => {
          if (png) resolve(png)
          else reject(new Error('canvas.toBlob 返回空结果'))
        }, 'image/png')
      })
    } finally {
      bitmap.close()
    }
  }

  function formatRawErrorNameMessage(error: unknown) {
    if (error instanceof Error) return `${error.name}: ${error.message}`
    const maybe = error as { name?: unknown; message?: unknown } | null
    if (maybe && typeof maybe === 'object') {
      const name = typeof maybe.name === 'string' ? maybe.name : 'UnknownError'
      const message = typeof maybe.message === 'string' ? maybe.message : String(error)
      return `${name}: ${message}`
    }
    return `UnknownError: ${String(error)}`
  }

  function clipboardFeatureAvailability() {
    const hasNavigator = typeof navigator !== 'undefined'
    const hasClipboard = hasNavigator && 'clipboard' in navigator && !!navigator.clipboard
    const hasClipboardWrite =
      hasClipboard && typeof (navigator.clipboard as Clipboard).write === 'function'
    const hasClipboardItem = typeof ClipboardItem !== 'undefined'
    const hasCreateImageBitmap = typeof createImageBitmap === 'function'
    return {
      hasNavigator,
      hasClipboard,
      hasClipboardWrite,
      hasClipboardItem,
      hasCreateImageBitmap,
    }
  }

  function formatClipboardFeatureAvailability(features = clipboardFeatureAvailability()) {
    return `navigator=${features.hasNavigator}, clipboard=${features.hasClipboard}, clipboard.write=${features.hasClipboardWrite}, ClipboardItem=${features.hasClipboardItem}, createImageBitmap=${features.hasCreateImageBitmap}`
  }

  function buildClipboardCopyErrorText(error: unknown, scene: string) {
    return [
      `${scene}失败`,
      `原始错误：${formatRawErrorNameMessage(error)}`,
      `能力检测：${formatClipboardFeatureAvailability()}`,
    ].join('\n')
  }

  function clearInternalBoardCopyRefForItem(itemId: number) {
    if (internalBoardCopyRef.value?.itemId === itemId) {
      internalBoardCopyRef.value = null
    }
  }

  function clearInternalBoardCopyRefForItems(itemIds: Iterable<number>) {
    if (!internalBoardCopyRef.value) return
    for (const itemId of itemIds) {
      if (internalBoardCopyRef.value.itemId === itemId) {
        internalBoardCopyRef.value = null
        return
      }
    }
  }

  return {
    copyReferenceBoardItemToClipboard,
    pasteReferenceBoardContent,
    copyImageToSystemClipboard,
    buildClipboardCopyErrorText,
    clearInternalBoardCopyRefForItem,
    clearInternalBoardCopyRefForItems,
  }
}
