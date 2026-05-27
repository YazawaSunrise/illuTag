<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import SegmentedMasonry from './SegmentedMasonry.vue'
import type { GalleryLayoutItem } from '../types/gallery'

type DragState = {
  imageId: string
}

type KnownAutoTagSuggestion = {
  tagEn: string
  tagZh?: string | null
  imageCount: number
}

const props = defineProps<{
  previewDragOverDeleteZone: boolean
  visibleImages: Array<{ id: string }>
  searchPanelStyle: Record<string, string>
  searchRevealMode: 'inline' | 'hidden' | 'floating'
  isSearchFocused: boolean
  searchZhInput: string
  searchZhSelected: KnownAutoTagSuggestion[]
  searchZhSuggestions: KnownAutoTagSuggestion[]
  searchZhOpen: boolean
  searchEnQuery: string
  searchFileNameQuery: string
  searchNaturalLanguageQuery: string
  searchConfidenceMin: number
  searchConfidenceMax: number
  searchRunning: boolean
  searchNeedsApply: boolean
  searchError: string
  isLoading: boolean
  layoutItems: GalleryLayoutItem[]
  totalHeight: number
  contentWidth: number
  dragState: DragState | null
  handlers: Record<string, (...args: any[]) => any>
}>()

const gallerySectionEl = ref<HTMLElement | null>(null)
const searchPanelEl = ref<HTMLElement | null>(null)
const gallerySectionResizeObserver = ref<ResizeObserver | null>(null)
const searchPanelResizeObserver = ref<ResizeObserver | null>(null)
const searchHideTimer = ref<number | null>(null)
const lastPointerClient = ref<{ x: number; y: number } | null>(null)
const viewportSyncRaf = ref<number | null>(null)
const suppressOutsideClickOnce = ref(false)
const searchChipsEl = ref<HTMLElement | null>(null)
const searchChipsDragState = ref<{
  pointerId: number
  startX: number
  startScrollLeft: number
} | null>(null)

function clearSearchHideTimer() {
  if (searchHideTimer.value !== null) {
    window.clearTimeout(searchHideTimer.value)
    searchHideTimer.value = null
  }
}

function clearViewportSyncRaf() {
  if (viewportSyncRaf.value !== null) {
    window.cancelAnimationFrame(viewportSyncRaf.value)
    viewportSyncRaf.value = null
  }
}

function scheduleSearchViewportSync() {
  clearViewportSyncRaf()
  viewportSyncRaf.value = window.requestAnimationFrame(() => {
    viewportSyncRaf.value = null
    syncSearchViewportState()
  })
}

function syncSearchViewportState() {
  const section = gallerySectionEl.value
  const panel = searchPanelEl.value
  if (!section || !panel) return
  props.handlers.setSearchViewportState(
    section.scrollTop,
    section.clientHeight,
    panel.offsetHeight,
    panel.offsetTop,
  )
}

onMounted(() => {
  props.handlers.setGalleryElement(gallerySectionEl.value)
  if (gallerySectionEl.value) {
    props.handlers.onGalleryScroll(gallerySectionEl.value.scrollTop, gallerySectionEl.value.clientHeight)
  }
  syncSearchViewportState()
  if (gallerySectionEl.value && typeof ResizeObserver !== 'undefined') {
    gallerySectionResizeObserver.value = new ResizeObserver(() => {
      scheduleSearchViewportSync()
    })
    gallerySectionResizeObserver.value.observe(gallerySectionEl.value)
  }
  if (searchPanelEl.value && typeof ResizeObserver !== 'undefined') {
    searchPanelResizeObserver.value = new ResizeObserver(() => {
      scheduleSearchViewportSync()
    })
    searchPanelResizeObserver.value.observe(searchPanelEl.value)
  }
  window.addEventListener('resize', scheduleSearchViewportSync, { passive: true })
  window.addEventListener('pointermove', trackPointerPosition, { passive: true })
})

onBeforeUnmount(() => {
  clearSearchHideTimer()
  clearViewportSyncRaf()
  finishSearchChipsDrag()
  gallerySectionResizeObserver.value?.disconnect()
  gallerySectionResizeObserver.value = null
  searchPanelResizeObserver.value?.disconnect()
  searchPanelResizeObserver.value = null
  window.removeEventListener('resize', scheduleSearchViewportSync)
  window.removeEventListener('pointermove', trackPointerPosition)
  props.handlers.setGalleryElement(null)
  props.handlers.setSearchPointerInside(false)
  props.handlers.setSearchFocus(false)
})

function trackPointerPosition(event: PointerEvent) {
  lastPointerClient.value = { x: event.clientX, y: event.clientY }
}

function isPointerInSearchSafeArea() {
  const pointer = lastPointerClient.value
  if (!pointer) return false
  const searchPanel = searchPanelEl.value
  if (searchPanel) {
    const rect = searchPanel.getBoundingClientRect()
    const inHorizontalBridge = pointer.x >= rect.left - 12 && pointer.x <= rect.right + 12
    const inVerticalBridge = pointer.y >= 0 && pointer.y <= rect.bottom
    if (inHorizontalBridge && inVerticalBridge) return true
  }
  if (pointer.x < 0 || pointer.y < 0 || pointer.x > window.innerWidth || pointer.y > window.innerHeight) {
    return true
  }
  const element = document.elementFromPoint(pointer.x, pointer.y) as HTMLElement | null
  if (!element) return false
  if (element.closest('.gallery-search')) return true
  if (element.closest('.gallery-search-hotspot')) return true
  if (element.closest('.app-titlebar')) return true
  if (element.closest('.app-titlebar-hotspot')) return true
  return false
}

function onSearchFocusIn() {
  suppressOutsideClickOnce.value = false
  props.handlers.setSearchFocus(true)
}

function onSearchFocusOut(event: FocusEvent) {
  const current = event.currentTarget as HTMLElement | null
  const next = event.relatedTarget as Node | null
  if (current && next && current.contains(next)) return
  props.handlers.setSearchFocus(false)
}

function onSearchPointerEnter() {
  suppressOutsideClickOnce.value = false
  clearSearchHideTimer()
  props.handlers.setSearchPointerInside(true)
}

function onSearchPointerLeave() {
  props.handlers.setSearchPointerInside(false)
  clearSearchHideTimer()
  searchHideTimer.value = window.setTimeout(() => {
    if (props.isSearchFocused) {
      searchHideTimer.value = null
      return
    }
    if (isPointerInSearchSafeArea()) {
      searchHideTimer.value = null
      return
    }
    props.handlers.hideSearchPanel()
    searchHideTimer.value = null
  }, 60)
}

function onGalleryScrollEvent(event: Event) {
  const element = event.target as HTMLElement
  props.handlers.onGalleryScroll(element.scrollTop, element.clientHeight)
  syncSearchViewportState()
}

function onSearchHotspotEnter() {
  clearSearchHideTimer()
  props.handlers.triggerSearchRevealByHotspot()
}

function isTargetInsideSearch(target: EventTarget | null) {
  return target instanceof HTMLElement && Boolean(target.closest('.gallery-search'))
}

function dismissSearchFocus() {
  const active = document.activeElement as HTMLElement | null
  if (active && searchPanelEl.value?.contains(active)) {
    active.blur()
  }
  props.handlers.setSearchFocus(false)
  props.handlers.setSearchPointerInside(false)
}

function onGalleryPointerDownCapture(event: PointerEvent) {
  if (!props.isSearchFocused) return
  if (isTargetInsideSearch(event.target)) return
  dismissSearchFocus()
  suppressOutsideClickOnce.value = true
  event.preventDefault()
  event.stopPropagation()
}

function onGalleryClickCapture(event: MouseEvent) {
  if (!suppressOutsideClickOnce.value) return
  if (isTargetInsideSearch(event.target)) {
    suppressOutsideClickOnce.value = false
    return
  }
  suppressOutsideClickOnce.value = false
  event.preventDefault()
  event.stopPropagation()
}

function onGalleryWheelCapture(event: WheelEvent) {
  if (!props.isSearchFocused) return
  if (isTargetInsideSearch(event.target)) return
  dismissSearchFocus()
  suppressOutsideClickOnce.value = false
}

function onSearchPointerDown() {
  suppressOutsideClickOnce.value = false
}

function finishSearchChipsDrag() {
  const container = searchChipsEl.value
  const state = searchChipsDragState.value
  if (container && state && container.hasPointerCapture(state.pointerId)) {
    container.releasePointerCapture(state.pointerId)
  }
  if (container) {
    container.classList.remove('is-dragging')
  }
  searchChipsDragState.value = null
}

function onSearchChipsPointerDown(event: PointerEvent) {
  if (event.button !== 0) return
  const target = event.target as HTMLElement | null
  if (target?.closest('.gallery-search__chip-remove')) return
  const container = searchChipsEl.value
  if (!container) return
  searchChipsDragState.value = {
    pointerId: event.pointerId,
    startX: event.clientX,
    startScrollLeft: container.scrollLeft,
  }
  container.classList.add('is-dragging')
  container.setPointerCapture(event.pointerId)
  event.preventDefault()
}

function onSearchChipsPointerMove(event: PointerEvent) {
  const state = searchChipsDragState.value
  const container = searchChipsEl.value
  if (!state || !container || state.pointerId !== event.pointerId) return
  const deltaX = event.clientX - state.startX
  container.scrollLeft = state.startScrollLeft - deltaX
  event.preventDefault()
}

function onSearchChipsPointerUp(event: PointerEvent) {
  const state = searchChipsDragState.value
  if (!state || state.pointerId !== event.pointerId) return
  finishSearchChipsDrag()
}

function onSearchChipsPointerCancel(event: PointerEvent) {
  const state = searchChipsDragState.value
  if (!state || state.pointerId !== event.pointerId) return
  finishSearchChipsDrag()
}

function onSearchChipsLostPointerCapture() {
  if (!searchChipsDragState.value) return
  finishSearchChipsDrag()
}

function canScrollContainer(container: HTMLElement, deltaY: number) {
  if (container.scrollHeight <= container.clientHeight + 1) return false
  if (deltaY < 0) return container.scrollTop > 0
  if (deltaY > 0) return container.scrollTop + container.clientHeight < container.scrollHeight - 1
  return false
}

function onSearchWheel(event: WheelEvent) {
  const target = event.target as HTMLElement | null
  if (!target) {
    event.preventDefault()
    event.stopPropagation()
    return
  }

  const scrollable = target.closest<HTMLElement>('.gallery-search__suggestions, .gallery-search__chips')
  if (scrollable) {
    if (!canScrollContainer(scrollable, event.deltaY)) {
      event.preventDefault()
    }
    event.stopPropagation()
    return
  }

  event.preventDefault()
  event.stopPropagation()
}
</script>

<template>
  <section
    ref="gallerySectionEl"
    class="gallery-page"
    :class="{ 'is-preview-delete-target': previewDragOverDeleteZone }"
    @pointerdown.capture="onGalleryPointerDownCapture($event)"
    @click.capture="onGalleryClickCapture($event)"
    @wheel.capture="onGalleryWheelCapture($event)"
    @dragover="handlers.onGalleryPreviewBoardItemDragOver($event)"
    @drop="handlers.onGalleryPreviewBoardItemDrop($event)"
    @wheel.passive="handlers.onGalleryWheel($event as WheelEvent)"
    @scroll.passive="onGalleryScrollEvent($event)"
  >
    <div
      class="gallery-search-hotspot"
      :class="{ 'is-active': searchRevealMode === 'hidden' }"
      @mouseenter="onSearchHotspotEnter()"
    />

    <div
      ref="searchPanelEl"
      class="gallery-search"
      :class="{ 'is-floating': searchRevealMode === 'floating' }"
      :style="searchPanelStyle"
      @focusin="onSearchFocusIn"
      @focusout="onSearchFocusOut"
      @pointerdown="onSearchPointerDown"
      @mouseenter="onSearchPointerEnter"
      @mouseleave="onSearchPointerLeave"
      @wheel="onSearchWheel"
    >
      <div class="gallery-search__grid">
        <div class="gallery-search__cell gallery-search__cell--tags" :class="{ 'is-chip-dragging': Boolean(searchChipsDragState) }">
          <div class="gallery-search__label-row">
            <div class="gallery-search__label">中文标签联想</div>
            <div
              ref="searchChipsEl"
              class="gallery-search__chips"
              :class="{ 'is-dragging': Boolean(searchChipsDragState) }"
              @pointerdown="onSearchChipsPointerDown"
              @pointermove="onSearchChipsPointerMove"
              @pointerup="onSearchChipsPointerUp"
              @pointercancel="onSearchChipsPointerCancel"
              @lostpointercapture="onSearchChipsLostPointerCapture"
            >
              <span v-for="tag in searchZhSelected" :key="tag.tagEn" class="gallery-search__chip">
                <span class="gallery-search__chip-text">{{ tag.tagZh || tag.tagEn }}</span>
                <button
                  type="button"
                  class="gallery-search__chip-remove"
                  @click.stop="handlers.removeSearchZhSuggestion(tag.tagEn)"
                >
                  ×
                </button>
              </span>
            </div>
          </div>
          <input
            class="gallery-search__input"
            type="text"
            :value="searchZhInput"
            placeholder="输入中文关键词"
            autocomplete="off"
            @focus="handlers.openSearchZhSuggestionPanel()"
            @input="handlers.setSearchZhInput(($event.target as HTMLInputElement).value)"
            @blur="handlers.closeSearchZhSuggestionPanelDeferred()"
          />
          <div v-if="searchZhOpen" class="gallery-search__suggestions">
            <button
              v-for="item in searchZhSuggestions"
              :key="item.tagEn"
              class="gallery-search__suggestion"
              type="button"
              @mousedown.prevent="handlers.selectSearchZhSuggestion(item)"
            >
              <span>{{ item.tagZh || item.tagEn }}</span>
              <small>{{ item.tagEn }} · {{ item.imageCount }}</small>
            </button>
          </div>
        </div>

        <div class="gallery-search__cell">
          <div class="gallery-search__label">英文标签（空格分词）</div>
          <input
            class="gallery-search__input"
            type="text"
            :value="searchEnQuery"
            placeholder="如 black hair smile"
            autocomplete="off"
            @input="handlers.setSearchEnQuery(($event.target as HTMLInputElement).value)"
            @keydown.enter.prevent="handlers.executeGallerySearch()"
          />
        </div>

        <div class="gallery-search__cell">
          <div class="gallery-search__label">文件名模糊搜索</div>
          <input
            class="gallery-search__input"
            type="text"
            :value="searchFileNameQuery"
            placeholder="文件名关键词"
            autocomplete="off"
            @input="handlers.setSearchFileNameQuery(($event.target as HTMLInputElement).value)"
            @keydown.enter.prevent="handlers.executeGallerySearch()"
          />
        </div>

        <div class="gallery-search__cell">
          <div class="gallery-search__label">置信度最小值</div>
          <div class="gallery-search__range-row">
            <input
              class="gallery-search__range"
              type="range"
              min="0"
              max="1"
              step="0.01"
              :value="searchConfidenceMin"
              @input="handlers.setSearchConfidenceMin(Number(($event.target as HTMLInputElement).value))"
            />
            <div class="gallery-search__range-values">
              <span>0</span>
              <span>{{ searchConfidenceMin.toFixed(2) }}</span>
            </div>
          </div>
        </div>

        <div class="gallery-search__cell">
          <div class="gallery-search__label">置信度最大值</div>
          <div class="gallery-search__range-row">
            <input
              class="gallery-search__range"
              type="range"
              min="0"
              max="1"
              step="0.01"
              :value="searchConfidenceMax"
              @input="handlers.setSearchConfidenceMax(Number(($event.target as HTMLInputElement).value))"
            />
            <div class="gallery-search__range-values">
              <span>{{ searchConfidenceMax.toFixed(2) }}</span>
              <span>1</span>
            </div>
          </div>
        </div>

        <div class="gallery-search__cell">
          <div class="gallery-search__label">自然语言搜索</div>
          <input
            class="gallery-search__input"
            type="text"
            :value="searchNaturalLanguageQuery"
            placeholder="例如：白发女孩在夜景中"
            autocomplete="off"
            @input="handlers.setSearchNaturalLanguageQuery(($event.target as HTMLInputElement).value)"
            @keydown.enter.prevent="handlers.executeGallerySearch()"
          />
          <div class="gallery-search__footer">
            <button
              type="button"
              class="gallery-search__submit"
              :disabled="searchRunning"
              @click="handlers.executeGallerySearch()"
            >
              开始搜索
            </button>
            <div v-if="searchNeedsApply">有未应用的筛选条件</div>
            <div>{{ searchRunning ? '搜索中…' : '搜索就绪' }}</div>
            <div v-if="searchError">{{ searchError }}</div>
          </div>
        </div>
      </div>
    </div>

    <div v-if="previewDragOverDeleteZone" class="gallery-delete-overlay" aria-hidden="true">
      <span class="gallery-delete-overlay__icon">🗑</span>
    </div>
    <div v-if="visibleImages.length === 0" class="empty-panel">
      <h2>还没有图片</h2>
      <p>选择“所有”查看全部图片，或先在设置里添加本地图库文件夹。</p>
      <button class="primary-button" type="button" :disabled="isLoading" @click="handlers.openSettings()">
        去添加
      </button>
    </div>
    <SegmentedMasonry
      v-else
      :items="layoutItems"
      :total-height="totalHeight"
      :content-width="contentWidth"
      :active-drag-image-id="dragState?.imageId ?? null"
      @image-pointer-down="handlers.startImagePress"
      @image-pointer-up="handlers.clearImagePress"
      @image-click="handlers.openGalleryImageDetail"
      @image-context-menu="handlers.openGalleryImageMenu"
    />
  </section>
</template>
