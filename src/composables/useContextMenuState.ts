import { computed, ref } from 'vue'
import type { GalleryLayoutItem } from '../types/gallery'

type ImageDetailContextMenu = { x: number; y: number } | null
type GalleryImageContextMenu = { imageId: string; x: number; y: number } | null

export function useContextMenuState() {
  const imageDetailContextMenu = ref<ImageDetailContextMenu>(null)
  const galleryImageContextMenu = ref<GalleryImageContextMenu>(null)

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

  function closeImageDetailContextMenu() {
    imageDetailContextMenu.value = null
  }

  function closeGalleryImageContextMenu() {
    galleryImageContextMenu.value = null
  }

  function openGalleryImageMenu(
    item: GalleryLayoutItem,
    event: MouseEvent,
    closeReferenceBoardCanvasMenu?: () => void,
  ) {
    event.preventDefault()
    event.stopPropagation()
    closeReferenceBoardCanvasMenu?.()
    galleryImageContextMenu.value = {
      imageId: item.id,
      x: event.clientX,
      y: event.clientY,
    }
    imageDetailContextMenu.value = null
  }

  function openImageDetailMenu(
    event: MouseEvent,
    isDetailAvailable: boolean,
    closeReferenceBoardCanvasMenu?: () => void,
  ) {
    if (!isDetailAvailable) return
    event.preventDefault()
    event.stopPropagation()
    closeReferenceBoardCanvasMenu?.()
    closeGalleryImageContextMenu()
    imageDetailContextMenu.value = { x: event.clientX, y: event.clientY }
  }

  function closeAllContextMenus() {
    closeImageDetailContextMenu()
    closeGalleryImageContextMenu()
  }

  return {
    imageDetailContextMenu,
    galleryImageContextMenu,
    imageDetailContextMenuStyle,
    galleryImageContextMenuStyle,
    closeImageDetailContextMenu,
    closeGalleryImageContextMenu,
    closeAllContextMenus,
    openGalleryImageMenu,
    openImageDetailMenu,
  }
}
