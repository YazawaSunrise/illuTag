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
  toFileSrc?: (path: string) => string
  pickExternalImagePath?: () => Promise<string | null>
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
  const searchNaturalLanguageQuery = ref('')
  const searchMode = ref<'text' | 'image'>('text')
  const externalImageQueryPath = ref<string | null>(null)
  const externalImageQueryBytes = ref<number[] | null>(null)
  const externalImageQueryUrl = ref('')
  const externalImageQueryPreviewUrl = ref<string>('')
  const externalImageQueryLabel = ref('')
  const externalImageQueryObjectUrl = ref<string | null>(null)
  const externalImageRankedImageIds = ref<string[] | null>(null)
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
  const naturalLanguageRankedImageIds = ref<string[] | null>(null)
  const searchRequestToken = ref(0)
  const searchSuggestRequestToken = ref(0)
  const searchSuggestTimer = ref<number | null>(null)
  const searchExecuteTimer = ref<number | null>(null)
  const searchHideCommitTimer = ref<number | null>(null)

  const hasSearchFilters = computed(
    () =>
      searchZhSelected.value.length > 0 ||
      searchEnQuery.value.trim().length > 0 ||
      searchFileNameQuery.value.trim().length > 0 ||
      searchNaturalLanguageQuery.value.trim().length > 0,
  )

  const visibleImages = computed(() => {
    if (options.activeUserFolderId.value === 'trash') return options.folderScopedImages.value
    const scoped = options.folderScopedImages.value
    if (searchMode.value === 'image') {
      if (!externalImageRankedImageIds.value) return scoped
      const rank = new Map(externalImageRankedImageIds.value.map((imageId, index) => [imageId, index]))
      return scoped
        .filter((image) => rank.has(image.id))
        .sort((a, b) => (rank.get(a.id) ?? Number.MAX_SAFE_INTEGER) - (rank.get(b.id) ?? Number.MAX_SAFE_INTEGER))
    }
    const filtered =
      !hasSearchFilters.value || !searchResultImageIds.value
        ? scoped
        : scoped.filter((image) => searchResultImageIds.value?.has(image.id))
    if (!naturalLanguageRankedImageIds.value) return filtered
    const rank = new Map(naturalLanguageRankedImageIds.value.map((imageId, index) => [imageId, index]))
    return filtered
      .filter((image) => rank.has(image.id))
      .sort((a, b) => (rank.get(a.id) ?? Number.MAX_SAFE_INTEGER) - (rank.get(b.id) ?? Number.MAX_SAFE_INTEGER))
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
    () => searchZhSelected.value.map((item) => item.tagEn).sort().join('\u0000'),
    () => {
      queueGallerySearchExecution(120)
    },
    { immediate: true },
  )

  onUnmounted(() => {
    if (searchSuggestTimer.value !== null) {
      window.clearTimeout(searchSuggestTimer.value)
      searchSuggestTimer.value = null
    }
    if (searchHideCommitTimer.value !== null) {
      window.clearTimeout(searchHideCommitTimer.value)
      searchHideCommitTimer.value = null
    }
    if (searchExecuteTimer.value !== null) {
      window.clearTimeout(searchExecuteTimer.value)
      searchExecuteTimer.value = null
    }
    clearExternalImageQueryPreview()
  })

  function clearExternalImageQueryPreview() {
    if (externalImageQueryObjectUrl.value) {
      URL.revokeObjectURL(externalImageQueryObjectUrl.value)
      externalImageQueryObjectUrl.value = null
    }
    externalImageQueryPreviewUrl.value = ''
  }

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
    switchToTextModeByMutualExclusion()
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
    switchToTextModeByMutualExclusion()
    if (searchZhSelected.value.some((existing) => existing.tagEn === item.tagEn)) {
      searchZhInput.value = ''
      searchZhOpen.value = false
      return
    }
    searchZhSelected.value = [item, ...searchZhSelected.value]
    searchZhInput.value = ''
    searchZhOpen.value = false
  }

  function removeSearchZhSuggestion(tagEn: string) {
    switchToTextModeByMutualExclusion()
    searchZhSelected.value = searchZhSelected.value.filter((item) => item.tagEn !== tagEn)
  }

  function setSearchEnQuery(value: string) {
    switchToTextModeByMutualExclusion()
    searchEnQuery.value = value
  }

  function setSearchFileNameQuery(value: string) {
    switchToTextModeByMutualExclusion()
    searchFileNameQuery.value = value
  }

  function setSearchNaturalLanguageQuery(value: string) {
    switchToTextModeByMutualExclusion()
    searchNaturalLanguageQuery.value = value
  }

  function setSearchMode(mode: 'text' | 'image') {
    if (mode === 'text') {
      clearImageSearchInputs()
    } else {
      clearTextSearchInputs()
    }
    searchMode.value = mode
    searchError.value = ''
    if (mode === 'text') {
      externalImageRankedImageIds.value = null
    }
  }

  function setSearchConfidenceMin(value: number) {
    switchToTextModeByMutualExclusion()
    searchConfidenceMin.value = options.clamp(value, 0, searchConfidenceMax.value)
  }

  function setSearchConfidenceMax(value: number) {
    switchToTextModeByMutualExclusion()
    searchConfidenceMax.value = options.clamp(value, searchConfidenceMin.value, 1)
  }

  function queueExternalImageSearchImmediately() {
    searchMode.value = 'image'
    queueGallerySearchExecution(0)
  }

  function clearImageSearchInputs() {
    clearExternalImageQueryPreview()
    externalImageQueryPath.value = null
    externalImageQueryBytes.value = null
    externalImageQueryUrl.value = ''
    externalImageQueryLabel.value = ''
    externalImageRankedImageIds.value = null
  }

  function clearTextSearchInputs() {
    searchZhInput.value = ''
    searchZhSelected.value = []
    searchZhSuggestions.value = []
    searchZhOpen.value = false
    searchEnQuery.value = ''
    searchFileNameQuery.value = ''
    searchNaturalLanguageQuery.value = ''
    searchConfidenceMin.value = 0
    searchConfidenceMax.value = 1
    searchResultImageIds.value = null
    naturalLanguageRankedImageIds.value = null
  }

  function switchToTextModeByMutualExclusion() {
    if (searchMode.value !== 'text') {
      clearImageSearchInputs()
      searchMode.value = 'text'
    }
  }

  function setExternalImageQueryFromPath(path: string, autoSearch = false) {
    const normalized = path.trim()
    if (!normalized) return
    clearExternalImageQueryPreview()
    externalImageQueryPath.value = normalized
    externalImageQueryBytes.value = null
    externalImageQueryUrl.value = ''
    clearTextSearchInputs()
    externalImageQueryPreviewUrl.value = options.toFileSrc?.(normalized) ?? ''
    externalImageQueryLabel.value = normalized
    searchMode.value = 'image'
    searchError.value = ''
    if (autoSearch) {
      queueExternalImageSearchImmediately()
    }
  }

  async function setExternalImageQueryFromBlob(blob: Blob, label?: string, autoSearch = false) {
    clearExternalImageQueryPreview()
    const objectUrl = URL.createObjectURL(blob)
    externalImageQueryObjectUrl.value = objectUrl
    externalImageQueryPreviewUrl.value = objectUrl
    externalImageQueryPath.value = null
    externalImageQueryBytes.value = []
    externalImageQueryUrl.value = ''
    clearTextSearchInputs()
    externalImageQueryLabel.value = label?.trim() || `剪贴板图片 ${Math.round(blob.size / 1024)}KB`
    searchMode.value = 'image'
    searchError.value = ''
    const buffer = await blob.arrayBuffer()
    externalImageQueryBytes.value = Array.from(new Uint8Array(buffer))
    if (autoSearch) {
      queueExternalImageSearchImmediately()
    }
  }

  function normalizeExternalImagePath(text: string) {
    const trimmed = text.trim().replace(/^["']|["']$/g, '')
    if (trimmed.startsWith('file://')) {
      return decodeURIComponent(trimmed.replace(/^file:\/\//i, ''))
    }
    return trimmed
  }

  function looksLikeHttpUrl(value: string) {
    return /^https?:\/\//i.test(value)
  }

  function setExternalImageQueryFromText(text: string, autoSearch = false) {
    const normalized = normalizeExternalImagePath(text)
    if (!normalized) return false
    if (looksLikeHttpUrl(normalized)) {
      setExternalImageQueryUrl(normalized, autoSearch)
      return true
    }
    setExternalImageQueryFromPath(normalized, autoSearch)
    return true
  }

  function isLikelyImageFile(file: File) {
    if (file.type?.startsWith('image/')) return true
    const name = file.name?.toLowerCase() ?? ''
    return /\.(png|jpe?g|webp|bmp|gif|ico|tiff?|avif)$/i.test(name)
  }

  async function setExternalImageSearchFromFile(file: File) {
    try {
      if (!isLikelyImageFile(file)) {
        throw new Error('Dropped file is not an image')
      }
      await setExternalImageQueryFromBlob(file, file.name || undefined, true)
      return true
    } catch (error) {
      searchError.value = options.formatError(error)
      return false
    }
  }

  function setExternalImageQueryUrl(value: string, autoSearch = false) {
    const next = value.trim()
    externalImageQueryUrl.value = next
    if (next.length > 0) {
      clearExternalImageQueryPreview()
      externalImageQueryPath.value = null
      externalImageQueryBytes.value = null
      externalImageQueryPreviewUrl.value = next
      externalImageQueryLabel.value = next
      clearTextSearchInputs()
      searchMode.value = 'image'
      searchError.value = ''
      if (autoSearch) {
        queueExternalImageSearchImmediately()
      }
    } else if (!externalImageQueryPreviewUrl.value) {
      externalImageQueryLabel.value = ''
      externalImageRankedImageIds.value = null
    }
  }

  function clearExternalImageSearch() {
    clearImageSearchInputs()
    searchMode.value = 'text'
    searchError.value = ''
  }

  async function selectExternalImageSearchFile() {
    if (!options.pickExternalImagePath) return
    const selected = await options.pickExternalImagePath()
    if (!selected) return
    setExternalImageQueryFromPath(selected, true)
  }

  async function pasteExternalImageSearchFromClipboard() {
    try {
      if (!navigator.clipboard) {
        throw new Error('当前环境不支持系统剪贴板读取')
      }
      const clipboardWithRead = navigator.clipboard as Clipboard & { read?: () => Promise<ClipboardItem[]> }
      if (typeof clipboardWithRead.read === 'function') {
        const items = await clipboardWithRead.read()
        for (const item of items) {
          const imageType = item.types.find((type) => type.startsWith('image/'))
          if (!imageType) continue
          const blob = await item.getType(imageType)
          await setExternalImageQueryFromBlob(blob, undefined, true)
          return true
        }
      }

      const text = await navigator.clipboard.readText()
      if (!text.trim()) {
        throw new Error('剪贴板中没有可用图片')
      }
      if (!setExternalImageQueryFromText(text, true)) {
        throw new Error('剪贴板中没有可用图片')
      }
      return true
    } catch (error) {
      searchError.value = options.formatError(error)
      return false
    }
  }

  async function pasteExternalImageSearchFromPasteEvent(event: ClipboardEvent) {
    try {
      const clipboardData = event.clipboardData
      if (!clipboardData) {
        return pasteExternalImageSearchFromClipboard()
      }
      const items = Array.from(clipboardData.items ?? [])
      for (const item of items) {
        if (item.kind !== 'file' || !item.type.startsWith('image/')) continue
        const file = item.getAsFile()
        if (!file) continue
        const ok = await setExternalImageSearchFromFile(file)
        if (ok) return true
      }
      const text = clipboardData.getData('text/plain')
      if (setExternalImageQueryFromText(text, true)) return true
      throw new Error('剪贴板中没有可用图片')
    } catch (error) {
      searchError.value = options.formatError(error)
      return false
    }
  }

  function searchBySingleTag(tagEn: string, tagZh?: string | null) {
    const normalized = tagEn.trim()
    if (!normalized) return
    searchMode.value = 'text'
    clearExternalImageQueryPreview()
    externalImageQueryPath.value = null
    externalImageQueryBytes.value = null
    externalImageQueryUrl.value = ''
    externalImageQueryLabel.value = ''
    externalImageRankedImageIds.value = null
    searchZhInput.value = ''
    searchZhOpen.value = false
    searchZhSuggestions.value = []
    searchEnQuery.value = ''
    searchFileNameQuery.value = ''
    searchNaturalLanguageQuery.value = ''
    searchConfidenceMin.value = 0
    searchConfidenceMax.value = 1
    const exists = searchZhSelected.value.some((tag) => tag.tagEn === normalized)
    if (exists) return
    searchZhSelected.value = [
      {
        tagEn: normalized,
        tagZh: tagZh ?? null,
        imageCount: 0,
      },
      ...searchZhSelected.value,
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
    if (searchMode.value === 'image') {
      const token = searchRequestToken.value + 1
      searchRequestToken.value = token
      searchRunning.value = true
      searchError.value = ''
      try {
        const queryPath = externalImageQueryPath.value?.trim() ?? ''
        const queryBytes = externalImageQueryBytes.value
        const queryUrl = externalImageQueryUrl.value.trim()
        if (!queryPath && !queryUrl && (!queryBytes || queryBytes.length === 0)) {
          externalImageRankedImageIds.value = null
          searchRunning.value = false
          return
        }
        const { invoke } = await import('@tauri-apps/api/core')
        const candidateImageIds =
          options.activeUserFolderId.value === 'trash'
            ? []
            : options.folderScopedImages.value.map((image) => image.id)
        const rankedIds = await invoke<string[]>('search_gallery_image_ids_by_external_image_command', {
          imagePath: queryPath || null,
          imageUrl: queryPath ? null : queryUrl || null,
          imageBytes: queryPath ? null : queryBytes,
          imageBase64: null,
          candidateImageIds,
          limit: 600,
        })
        if (token !== searchRequestToken.value) return
        externalImageRankedImageIds.value = rankedIds
        searchResultImageIds.value = null
        naturalLanguageRankedImageIds.value = null
      } catch (error) {
        if (token !== searchRequestToken.value) return
        searchError.value = options.formatError(error)
      } finally {
        if (token === searchRequestToken.value) {
          searchRunning.value = false
        }
      }
      return
    }

    const hasStructuredFilters =
      searchZhSelected.value.length > 0 ||
      searchEnQuery.value.trim().length > 0 ||
      searchFileNameQuery.value.trim().length > 0
    const naturalLanguageQuery = searchNaturalLanguageQuery.value.trim()
    const hasNaturalLanguageQuery = naturalLanguageQuery.length > 0

    if (!hasStructuredFilters && !hasNaturalLanguageQuery) {
      searchResultImageIds.value = null
      naturalLanguageRankedImageIds.value = null
      searchRunning.value = false
      searchError.value = ''
      return
    }

    const filters: GallerySearchFilters = {
      chineseTagEns: searchZhSelected.value.map((item) => item.tagEn),
      englishQuery: searchEnQuery.value,
      fileNameQuery: searchFileNameQuery.value,
      confidenceMin: 0,
      confidenceMax: 1,
    }

    const token = searchRequestToken.value + 1
    searchRequestToken.value = token
    searchRunning.value = true
    searchError.value = ''
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      let structuredIds: string[] | null = null
      if (hasStructuredFilters) {
        structuredIds = await invoke<string[]>('search_gallery_image_ids_command', {
          filters,
        })
      }
      if (token !== searchRequestToken.value) return
      searchResultImageIds.value = structuredIds ? new Set(structuredIds) : null

      if (hasNaturalLanguageQuery) {
        const rankedIds = await invoke<string[]>('search_gallery_image_ids_by_natural_language_command', {
          query: naturalLanguageQuery,
          candidateImageIds: structuredIds,
        })
        if (token !== searchRequestToken.value) return
        naturalLanguageRankedImageIds.value = rankedIds
      } else {
        naturalLanguageRankedImageIds.value = null
      }
    } catch (error) {
      if (token !== searchRequestToken.value) return
      searchError.value = options.formatError(error)
    } finally {
      if (token === searchRequestToken.value) {
        searchRunning.value = false
      }
    }
  }

  async function executeGallerySearch() {
    queueGallerySearchExecution(180)
  }

  function queueGallerySearchExecution(delayMs = 180) {
    if (searchExecuteTimer.value !== null) {
      window.clearTimeout(searchExecuteTimer.value)
      searchExecuteTimer.value = null
    }
    searchExecuteTimer.value = window.setTimeout(() => {
      searchExecuteTimer.value = null
      void runGallerySearch()
    }, Math.max(0, delayMs))
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
    searchNaturalLanguageQuery,
    searchMode,
    externalImageQueryUrl,
    externalImageQueryPreviewUrl,
    externalImageQueryLabel,
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
    setSearchNaturalLanguageQuery,
    setSearchMode,
    setSearchConfidenceMin,
    setSearchConfidenceMax,
    executeGallerySearch,
    clearExternalImageSearch,
    setExternalImageQueryUrl,
    pasteExternalImageSearchFromPasteEvent,
    setExternalImageSearchFromFile,
    selectExternalImageSearchFile,
    pasteExternalImageSearchFromClipboard,
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
