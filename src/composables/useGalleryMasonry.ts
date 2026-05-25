import { computed, ref, type Ref } from 'vue'
import type { GalleryImage, GalleryLayoutItem } from '../types/gallery'

type UseGalleryMasonryOptions = {
  visibleImages: Ref<GalleryImage[]>
  convertFileSrc: (path: string) => string
  clamp: (value: number, min: number, max: number) => number
  gap?: number
}

export function useGalleryMasonry(options: UseGalleryMasonryOptions) {
  const gap = options.gap ?? 12

  const galleryEl = ref<HTMLElement | null>(null)
  const viewportWidth = ref(960)
  const viewportHeight = ref(720)
  const galleryScrollTop = ref(0)
  const galleryViewportHeight = ref(0)

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

  const galleryLayout = computed<GalleryLayoutItem[]>(() => {
    const result: GalleryLayoutItem[] = []
    const columnHeights = Array.from({ length: columnCount.value }, () => 0)

    for (const image of options.visibleImages.value) {
      const columnIndex = shortestColumnIndex(columnHeights)
      const naturalHeight =
        image.width > 0 ? (image.height / image.width) * columnWidth.value : minItemHeight.value
      const height = options.clamp(naturalHeight, minItemHeight.value, maxItemHeight.value)

      result.push({
        id: image.id,
        thumbnailUrl: options.convertFileSrc(image.path),
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
    if (galleryLayout.value.length === 0) return 0
    return Math.max(...galleryLayout.value.map((item) => item.y + item.height))
  })

  const renderedLayoutItems = computed(() => {
    if (galleryLayout.value.length === 0) return galleryLayout.value
    if (galleryViewportHeight.value <= 0) return galleryLayout.value

    const buffer = Math.max(480, galleryViewportHeight.value * 0.8)
    const viewportTop = galleryScrollTop.value - buffer
    const viewportBottom = galleryScrollTop.value + galleryViewportHeight.value + buffer

    return galleryLayout.value.filter(
      (item) => item.y + item.height >= viewportTop && item.y <= viewportBottom,
    )
  })

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

  return {
    galleryEl,
    galleryScrollTop,
    galleryViewportHeight,
    columnCount,
    galleryLayout,
    renderedLayoutItems,
    masonryContentWidth,
    totalHeight,
    setGalleryElement,
    onGalleryScroll,
    onGalleryWheel,
    updateViewportSize,
  }
}

function shortestColumnIndex(heights: number[]) {
  let index = 0
  for (let i = 1; i < heights.length; i += 1) {
    if (heights[i] < heights[index]) index = i
  }
  return index
}
