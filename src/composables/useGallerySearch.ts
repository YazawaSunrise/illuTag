import { computed, onUnmounted, ref, watch, type Ref } from 'vue'
import type { GalleryImage, GalleryLayoutItem } from '../types/gallery'

type KnownAutoTagSuggestion = {
  tagEn: string
  tagZh?: string | null
  imageCount: number
}

type GallerySearchFilters = {
  chineseTagEns: string[]
  englishQuery: string
  fileNameQuery: string
  confidenceMin: number
  confidenceMax: number
}

type ImageTagRecord = {
  tagEn: string
  tagZh?: string | null
  confidence: number
  category?: string | null
}

type LibraryStoreLike = {
  images: GalleryImage[]
}

type UseGallerySearchOptions<TLibraryStore extends LibraryStoreLike> = {
  library: Ref<TLibraryStore>
  folderScopedImages: Ref<GalleryImage[]>
  activeUserFolderId: Ref<number | 'all' | 'trash'>
  lastImageDragEndedAt: Ref<number>
  formatError: (error: unknown) => string
  clamp: (value: number, min: number, max: number) => number
  onOpenImageDetail?: () => void
  onCloseImageDetail?: () => void
}

export function useGallerySearch<TLibraryStore extends LibraryStoreLike>(
  options: UseGallerySearchOptions<TLibraryStore>,
) {
  const searchZhInput = ref('')
  const searchZhSelected = ref<KnownAutoTagSuggestion[]>([])
  const searchZhSuggestions = ref<KnownAutoTagSuggestion[]>([])
  const searchZhOpen = ref(false)
  const searchEnQuery = ref('')
  const searchFileNameQuery = ref('')
  const searchConfidenceMin = ref(0)
  const searchConfidenceMax = ref(1)
  const searchRunning = ref(false)
  const searchError = ref('')
  const isSearchFocused = ref(false)
  const isSearchPointerInside = ref(false)
  const searchRevealMode = ref<'inline' | 'hidden' | 'floating'>('inline')
  const searchRevealProgress = ref(1)
  const searchRevealThreshold = 420
  const searchTopOffset = ref(0)
  const searchViewportTop = ref(0)
  const searchViewportHeight = ref(0)
  const searchPanelHeight = ref(0)
  const searchFloatingArmed = ref(false)

  const activeImageDetailId = ref<string | null>(null)
  const activeImageTagRows = ref<ImageTagRecord[]>([])
  const searchResultImageIds = ref<Set<string> | null>(null)
  const searchRequestToken = ref(0)
  const searchSuggestRequestToken = ref(0)
  const searchTimer = ref<number | null>(null)
  const searchSuggestTimer = ref<number | null>(null)
  const searchHideCommitTimer = ref<number | null>(null)

  const hasSearchFilters = computed(
    () =>
      searchZhSelected.value.length > 0 ||
      searchEnQuery.value.trim().length > 0 ||
      searchFileNameQuery.value.trim().length > 0 ||
      searchConfidenceMin.value > 0.0001 ||
      searchConfidenceMax.value < 0.9999,
  )

  const visibleImages = computed(() => {
    if (options.activeUserFolderId.value === 'trash') return options.folderScopedImages.value
    if (!hasSearchFilters.value || !searchResultImageIds.value) return options.folderScopedImages.value
    return options.folderScopedImages.value.filter((image) => searchResultImageIds.value?.has(image.id))
  })

  const activeImageDetail = computed(() => {
    if (!activeImageDetailId.value) return null
    return options.library.value.images.find((image) => image.id === activeImageDetailId.value) ?? null
  })

  const groupedImageTags = computed(() => {
    if (activeImageTagRows.value.length === 0) return []
    const orderedKeys = ['character', 'copyright', 'artist', 'general', 'meta', 'rating']
    const buckets = new Map<string, ImageTagRecord[]>()
    for (const row of activeImageTagRows.value) {
      const key = (row.category ?? 'other').trim() || 'other'
      const group = buckets.get(key) ?? []
      group.push(row)
      buckets.set(key, group)
    }

    const ordered: Array<{ key: string; label: string; rows: ImageTagRecord[] }> = []
    for (const key of orderedKeys) {
      const rows = buckets.get(key)
      if (!rows || rows.length === 0) continue
      ordered.push({ key, label: categoryLabel(key), rows })
      buckets.delete(key)
    }

    const rest = [...buckets.entries()].sort(([a], [b]) => a.localeCompare(b, 'zh-Hans-CN'))
    for (const [key, rows] of rest) {
      ordered.push({ key, label: categoryLabel(key), rows })
    }
    return ordered
  })

  watch(
    () => searchZhInput.value,
    () => {
      if (searchSuggestTimer.value !== null) window.clearTimeout(searchSuggestTimer.value)
      searchSuggestTimer.value = window.setTimeout(() => {
        void refreshSearchZhSuggestions()
      }, 140)
    },
  )

  watch(
    () => [
      searchZhSelected.value.map((item) => item.tagEn).sort().join('\u0000'),
      searchEnQuery.value,
      searchFileNameQuery.value,
      searchConfidenceMin.value,
      searchConfidenceMax.value,
    ],
    () => {
      if (searchTimer.value !== null) window.clearTimeout(searchTimer.value)
      searchTimer.value = window.setTimeout(() => {
        void runGallerySearch()
      }, 150)
    },
    { immediate: true },
  )

  onUnmounted(() => {
    if (searchTimer.value !== null) {
      window.clearTimeout(searchTimer.value)
      searchTimer.value = null
    }
    if (searchSuggestTimer.value !== null) {
      window.clearTimeout(searchSuggestTimer.value)
      searchSuggestTimer.value = null
    }
    if (searchHideCommitTimer.value !== null) {
      window.clearTimeout(searchHideCommitTimer.value)
      searchHideCommitTimer.value = null
    }
  })

  function setSearchPointerInside(next: boolean) {
    isSearchPointerInside.value = next
    updateSearchRevealMode()
  }

  function setSearchFocus(next: boolean) {
    isSearchFocused.value = next
    updateSearchRevealMode()
  }

  function setSearchViewportState(scrollTop: number, clientHeight: number, panelHeight: number, topOffset = 0) {
    searchTopOffset.value = topOffset
    searchViewportTop.value = scrollTop
    searchViewportHeight.value = clientHeight
    searchPanelHeight.value = panelHeight
    updateSearchRevealMode()
  }

  function triggerSearchRevealByHotspot() {
    const distance = Math.max(0, searchViewportTop.value - searchTopOffset.value)
    const panelBottom = searchTopOffset.value + searchPanelHeight.value
    const viewportTop = searchViewportTop.value
    const viewportBottom = viewportTop + searchViewportHeight.value
    const panelVisible = panelBottom > viewportTop && searchTopOffset.value < viewportBottom
    if (panelVisible) return
    if (distance <= searchRevealThreshold) return
    searchFloatingArmed.value = true
    updateSearchRevealMode()
  }

  function hideSearchPanel() {
    if (isSearchFocused.value || isSearchPointerInside.value) return
    if (searchHideCommitTimer.value !== null) {
      window.clearTimeout(searchHideCommitTimer.value)
      searchHideCommitTimer.value = null
    }

    if (searchRevealMode.value === 'floating') {
      searchFloatingArmed.value = false
      searchRevealProgress.value = 0
      searchHideCommitTimer.value = window.setTimeout(() => {
        searchHideCommitTimer.value = null
        updateSearchRevealMode()
      }, 380)
      return
    }

    searchFloatingArmed.value = false
    updateSearchRevealMode()
  }

  function updateSearchRevealMode() {
    if (searchHideCommitTimer.value !== null && (isSearchFocused.value || isSearchPointerInside.value)) {
      window.clearTimeout(searchHideCommitTimer.value)
      searchHideCommitTimer.value = null
    }

    const panelBottom = searchTopOffset.value + searchPanelHeight.value
    const viewportTop = searchViewportTop.value
    const viewportBottom = viewportTop + searchViewportHeight.value

    const panelVisible = panelBottom > viewportTop && searchTopOffset.value < viewportBottom
    if (isSearchFocused.value || isSearchPointerInside.value) {
      searchRevealMode.value = panelVisible ? 'inline' : 'floating'
      searchRevealProgress.value = 1
      return
    }

    if (panelVisible) {
      searchRevealMode.value = 'inline'
      searchRevealProgress.value = 1
      searchFloatingArmed.value = false
      return
    }

    const distance = Math.max(0, viewportTop - searchTopOffset.value)
    if (distance <= searchRevealThreshold) {
      searchRevealMode.value = 'hidden'
      searchRevealProgress.value = 0
      searchFloatingArmed.value = false
      return
    }

    searchRevealMode.value = searchFloatingArmed.value ? 'floating' : 'hidden'
    searchRevealProgress.value = searchFloatingArmed.value ? 1 : 0
  }

  function setSearchZhInput(value: string) {
    searchZhInput.value = value
  }

  function openSearchZhSuggestionPanel() {
    searchZhOpen.value = searchZhSuggestions.value.length > 0
  }

  function closeSearchZhSuggestionPanelDeferred() {
    window.setTimeout(() => {
      searchZhOpen.value = false
    }, 90)
  }

  function selectSearchZhSuggestion(item: KnownAutoTagSuggestion) {
    if (searchZhSelected.value.some((existing) => existing.tagEn === item.tagEn)) {
      searchZhInput.value = ''
      searchZhOpen.value = false
      return
    }
    searchZhSelected.value = [...searchZhSelected.value, item]
    searchZhInput.value = ''
    searchZhOpen.value = false
  }

  function removeSearchZhSuggestion(tagEn: string) {
    searchZhSelected.value = searchZhSelected.value.filter((item) => item.tagEn !== tagEn)
  }

  function setSearchEnQuery(value: string) {
    searchEnQuery.value = value
  }

  function setSearchFileNameQuery(value: string) {
    searchFileNameQuery.value = value
  }

  function setSearchConfidenceMin(value: number) {
    searchConfidenceMin.value = options.clamp(value, 0, searchConfidenceMax.value)
  }

  function setSearchConfidenceMax(value: number) {
    searchConfidenceMax.value = options.clamp(value, searchConfidenceMin.value, 1)
  }

  function searchBySingleTag(tagEn: string, tagZh?: string | null) {
    const normalized = tagEn.trim()
    if (!normalized) return
    searchZhInput.value = ''
    searchZhOpen.value = false
    searchZhSuggestions.value = []
    searchEnQuery.value = ''
    searchFileNameQuery.value = ''
    searchConfidenceMin.value = 0
    searchConfidenceMax.value = 1
    const exists = searchZhSelected.value.some((tag) => tag.tagEn === normalized)
    if (exists) return
    searchZhSelected.value = [
      ...searchZhSelected.value,
      {
        tagEn: normalized,
        tagZh: tagZh ?? null,
        imageCount: 0,
      },
    ]
  }

  async function refreshSearchZhSuggestions() {
    const keyword = searchZhInput.value.trim()
    if (!keyword) {
      searchZhSuggestions.value = []
      searchZhOpen.value = false
      return
    }

    const token = searchSuggestRequestToken.value + 1
    searchSuggestRequestToken.value = token
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const rows = await invoke<Array<Record<string, unknown>>>('suggest_known_auto_tags_command', {
        query: keyword,
        limit: 30,
      })
      if (token !== searchSuggestRequestToken.value) return
      const selected = new Set(searchZhSelected.value.map((item) => item.tagEn))
      searchZhSuggestions.value = rows
        .map((item) => ({
          tagEn: String(item.tagEn ?? item.tag_en ?? ''),
          tagZh: (item.tagZh ?? item.tag_zh ?? null) as string | null,
          imageCount: Number(item.imageCount ?? item.image_count ?? 0),
        }))
        .filter((item) => item.tagEn.length > 0 && !selected.has(item.tagEn))
      searchZhOpen.value = searchZhSuggestions.value.length > 0
    } catch {
      if (token !== searchSuggestRequestToken.value) return
      searchZhSuggestions.value = []
      searchZhOpen.value = false
    }
  }

  async function runGallerySearch() {
    if (!hasSearchFilters.value) {
      searchResultImageIds.value = null
      searchRunning.value = false
      searchError.value = ''
      return
    }

    const filters: GallerySearchFilters = {
      chineseTagEns: searchZhSelected.value.map((item) => item.tagEn),
      englishQuery: searchEnQuery.value,
      fileNameQuery: searchFileNameQuery.value,
      confidenceMin: searchConfidenceMin.value,
      confidenceMax: searchConfidenceMax.value,
    }

    const token = searchRequestToken.value + 1
    searchRequestToken.value = token
    searchRunning.value = true
    searchError.value = ''
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const ids = await invoke<string[]>('search_gallery_image_ids_command', {
        filters,
      })
      if (token !== searchRequestToken.value) return
      searchResultImageIds.value = new Set(ids)
    } catch (error) {
      if (token !== searchRequestToken.value) return
      searchError.value = options.formatError(error)
    } finally {
      if (token === searchRequestToken.value) {
        searchRunning.value = false
      }
    }
  }

  async function loadImageAutoTags(imageId: string) {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const summary = await invoke<Record<string, unknown>>('list_image_auto_tags_command', {
        imageId,
      })
      const rows = Object.values(summary)
        .flatMap((value) => (Array.isArray(value) ? value : []))
        .filter((item): item is Record<string, unknown> => typeof item === 'object' && item !== null)
        .map((item) => ({
          tagEn: String(item.tagEn ?? item.tag_en ?? ''),
          tagZh: (item.tagZh ?? item.tag_zh ?? null) as string | null,
          confidence: Number(item.confidence ?? item.score ?? 0),
          category: (item.category ?? null) as string | null,
        }))
        .filter((item) => item.tagEn.length > 0)
        .sort((a, b) => b.confidence - a.confidence)
      activeImageTagRows.value = rows
    } catch {
      activeImageTagRows.value = []
    }
  }

  function closeImageDetail() {
    activeImageDetailId.value = null
    activeImageTagRows.value = []
    options.onCloseImageDetail?.()
  }

  function openGalleryImageDetail(item: GalleryLayoutItem) {
    if (Date.now() - options.lastImageDragEndedAt.value < 260) return
    activeImageDetailId.value = item.id
    options.onOpenImageDetail?.()
    void loadImageAutoTags(item.id)
  }

  return {
    searchZhInput,
    searchZhSelected,
    searchZhSuggestions,
    searchZhOpen,
    searchEnQuery,
    searchFileNameQuery,
    searchConfidenceMin,
    searchConfidenceMax,
    searchRunning,
    searchError,
    isSearchFocused,
    isSearchPointerInside,
    searchRevealMode,
    searchRevealProgress,
    setSearchViewportState,
    triggerSearchRevealByHotspot,
    hideSearchPanel,
    visibleImages,
    activeImageDetailId,
    activeImageDetail,
    activeImageTagRows,
    groupedImageTags,
    setSearchPointerInside,
    setSearchFocus,
    setSearchZhInput,
    openSearchZhSuggestionPanel,
    closeSearchZhSuggestionPanelDeferred,
    selectSearchZhSuggestion,
    removeSearchZhSuggestion,
    setSearchEnQuery,
    setSearchFileNameQuery,
    setSearchConfidenceMin,
    setSearchConfidenceMax,
    searchBySingleTag,
    closeImageDetail,
    openGalleryImageDetail,
  }
}

function categoryLabel(key: string) {
  switch (key) {
    case 'character':
      return '人物标签'
    case 'copyright':
      return '作品标签'
    case 'artist':
      return '作者标签'
    case 'general':
      return '通用标签'
    case 'meta':
      return '元信息'
    case 'rating':
      return '分级标签'
    default:
      return '其他标签'
  }
}
