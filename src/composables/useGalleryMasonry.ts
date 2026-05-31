import { computed, ref, watch, type Ref } from 'vue'
import type { GalleryImage, GalleryLayoutItem } from '../types/gallery'

type UseGalleryMasonryOptions = {
  visibleImages: Ref<GalleryImage[]>
  convertFileSrc: (path: string) => string
  clamp: (value: number, min: number, max: number) => number
  gap?: number
}

export function useGalleryMasonry(options: UseGalleryMasonryOptions) {
  const gap = options.gap ?? 12
  const startupFallbackMaxItems = 90

  const galleryEl = ref<HTMLElement | null>(null)
  const viewportWidth = ref(960)
  const viewportHeight = ref(720)
  const galleryScrollTop = ref(0)
  const galleryViewportHeight = ref(0)
  const scrollTopByScope = new Map<string, number>()
  const activeScrollScopeKey = ref('all')
  const firstRenderedLogged = ref(false)
  const firstLayoutLogged = ref(false)
  const firstRenderedTimingLogged = ref(false)

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
    const started = performance.now()
    const visibleImageCount = options.visibleImages.value.length
    const result: GalleryLayoutItem[] = []
    const columnHeights = Array.from({ length: columnCount.value }, () => 0)

    for (const image of options.visibleImages.value) {
      const columnIndex = shortestColumnIndex(columnHeights)
      const naturalHeight =
        image.width > 0 ? (image.height / image.width) * columnWidth.value : minItemHeight.value
      const height = options.clamp(naturalHeight, minItemHeight.value, maxItemHeight.value)

      result.push({
        id: image.id,
        thumbnailUrl: options.convertFileSrc(image.thumbnailPath || image.path),
        x: columnIndex * (columnWidth.value + gap),
        y: columnHeights[columnIndex],
        width: columnWidth.value,
        height,
        columnIndex,
      })

      columnHeights[columnIndex] += height + gap
    }

    if (!firstLayoutLogged.value) {
      firstLayoutLogged.value = true
      console.info(
        `[startup-prof] galleryLayout first_visible_images=${visibleImageCount} first_layout_count=${result.length} ms=${(performance.now() - started).toFixed(2)} columns=${columnCount.value}`,
      )
    }

    return result
  })

  const totalHeight = computed(() => {
    if (galleryLayout.value.length === 0) return 0
    return Math.max(...galleryLayout.value.map((item) => item.y + item.height))
  })

  const renderedLayoutItems = computed(() => {
    const started = performance.now()
    if (galleryLayout.value.length === 0) return galleryLayout.value
    if (galleryViewportHeight.value <= 0) {
      const fallbackRows = 3
      const fallbackCount = Math.min(
        startupFallbackMaxItems,
        Math.max(columnCount.value * fallbackRows, columnCount.value),
      )
      const fallbackItems = galleryLayout.value.slice(0, fallbackCount)
      if (!firstRenderedTimingLogged.value) {
        firstRenderedTimingLogged.value = true
        console.info(
          `[startup-prof] renderedLayoutItems first_ms=${(performance.now() - started).toFixed(2)} first_count=${fallbackItems.length} total_layout=${galleryLayout.value.length} viewport_height=${galleryViewportHeight.value} scroll_top=${galleryScrollTop.value} fallback=true`,
        )
        console.info(
          `[startup-prof] renderedLayoutItems viewport_not_ready returned_count=${fallbackItems.length} max_fallback=${startupFallbackMaxItems}`,
        )
      }
      return fallbackItems
    }

    const buffer = Math.max(480, galleryViewportHeight.value * 0.8)
    const viewportTop = galleryScrollTop.value - buffer
    const viewportBottom = galleryScrollTop.value + galleryViewportHeight.value + buffer

    const items = galleryLayout.value.filter(
      (item) => item.y + item.height >= viewportTop && item.y <= viewportBottom,
    )
    if (!firstRenderedTimingLogged.value) {
      firstRenderedTimingLogged.value = true
      console.info(
        `[startup-prof] renderedLayoutItems first_ms=${(performance.now() - started).toFixed(2)} first_count=${items.length} total_layout=${galleryLayout.value.length} viewport_height=${galleryViewportHeight.value} scroll_top=${galleryScrollTop.value} fallback=false`,
      )
    }
    return items
  })

  watch(
    renderedLayoutItems,
    (items) => {
      if (firstRenderedLogged.value || items.length === 0) return
      firstRenderedLogged.value = true
      console.info(
        `[startup-prof] renderedLayoutItems first_count=${items.length} total_layout=${galleryLayout.value.length} viewport_height=${galleryViewportHeight.value} scroll_top=${galleryScrollTop.value}`,
      )
    },
    { flush: 'post' },
  )

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
    const savedTop = scrollTopByScope.get(activeScrollScopeKey.value) ?? galleryScrollTop.value
    galleryScrollTop.value = savedTop
    if (element) {
      element.scrollTop = savedTop
    }
    updateViewportSize()
  }

  function onGalleryScroll(scrollTop: number, clientHeight: number) {
    galleryScrollTop.value = scrollTop
    galleryViewportHeight.value = clientHeight
    scrollTopByScope.set(activeScrollScopeKey.value, Math.max(0, scrollTop))
  }

  function onGalleryWheel(_event: WheelEvent) {}

  function saveGalleryScrollPosition(scopeKey = activeScrollScopeKey.value) {
    const nextTop = Math.max(0, galleryEl.value?.scrollTop ?? galleryScrollTop.value)
    scrollTopByScope.set(scopeKey, nextTop)
    if (scopeKey === activeScrollScopeKey.value) {
      galleryScrollTop.value = nextTop
    }
  }

  function restoreGalleryScrollPosition(scopeKey: string) {
    activeScrollScopeKey.value = scopeKey
    const restoredTop = Math.max(0, scrollTopByScope.get(scopeKey) ?? 0)
    galleryScrollTop.value = restoredTop
    if (galleryEl.value) {
      galleryEl.value.scrollTop = restoredTop
      galleryViewportHeight.value = galleryEl.value.clientHeight
    }
  }

  function scrollGalleryToTop(scopeKey = activeScrollScopeKey.value) {
    activeScrollScopeKey.value = scopeKey
    scrollTopByScope.set(scopeKey, 0)
    galleryScrollTop.value = 0
    if (galleryEl.value) {
      galleryEl.value.scrollTop = 0
      galleryViewportHeight.value = galleryEl.value.clientHeight
    }
  }

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
    saveGalleryScrollPosition,
    restoreGalleryScrollPosition,
    scrollGalleryToTop,
  }
}

function shortestColumnIndex(heights: number[]) {
  let index = 0
  for (let i = 1; i < heights.length; i += 1) {
    if (heights[i] < heights[index]) index = i
  }
  return index
}
