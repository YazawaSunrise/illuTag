<script setup lang="ts">
import Like from '@icon-park/vue-next/es/icons/Like'
import Left from '@icon-park/vue-next/es/icons/Left'
import Logout from '@icon-park/vue-next/es/icons/Logout'
import Pause from '@icon-park/vue-next/es/icons/Pause'
import PlayOne from '@icon-park/vue-next/es/icons/PlayOne'
import Right from '@icon-park/vue-next/es/icons/Right'
import ShuffleOne from '@icon-park/vue-next/es/icons/ShuffleOne'
import SortTwo from '@icon-park/vue-next/es/icons/SortTwo'
import Time from '@icon-park/vue-next/es/icons/Time'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { GalleryImage } from '../types/gallery'

const props = defineProps<{
  images: GalleryImage[]
  initialIndex: number
  toFileSrc: (path: string) => string
}>()

const emit = defineEmits<{
  close: []
  favoriteToggle: [imageId: string, favorite: boolean]
}>()

const autoplaySecondsStorageKey = 'illutag.slideshowAutoplaySeconds'

function normalizeAutoplaySeconds(value: unknown) {
  const numberValue = Number(value)
  return Number.isFinite(numberValue) ? Math.min(60, Math.max(1, Math.round(numberValue))) : 5
}

function readStoredAutoplaySeconds() {
  try {
    return normalizeAutoplaySeconds(localStorage.getItem(autoplaySecondsStorageKey))
  } catch {
    return 5
  }
}

function writeStoredAutoplaySeconds(value: number) {
  try {
    localStorage.setItem(autoplaySecondsStorageKey, String(value))
  } catch {}
}

const initialAutoplaySeconds = readStoredAutoplaySeconds()
const currentIndex = ref(0)
const isPlaying = ref(true)
const isRandom = ref(false)
const autoplaySeconds = ref(initialAutoplaySeconds)
const autoplaySecondsDraft = ref(String(initialAutoplaySeconds))
const controlsVisible = ref(true)
const pointerInControls = ref(false)
const controlsHideTimer = ref<number | null>(null)
const autoplayTimer = ref<number | null>(null)
const mediaScale = ref(1)
const mediaPan = ref({ x: 0, y: 0 })
const mediaDrag = ref<null | {
  pointerId: number
  startX: number
  startY: number
  panX: number
  panY: number
}>(null)

const totalImages = computed(() => props.images.length)
const currentImage = computed(() => props.images[currentIndex.value] ?? null)
const currentImageSrc = computed(() => {
  const image = currentImage.value
  return image ? props.toFileSrc(image.path) : ''
})
const imageStyle = computed(() => ({
  transform: `translate3d(${mediaPan.value.x}px, ${mediaPan.value.y}px, 0) scale(${mediaScale.value})`,
  cursor: !isPlaying.value && mediaScale.value > 1 ? (mediaDrag.value ? 'grabbing' : 'grab') : 'default',
}))

function normalizeIndex(index: number) {
  const total = totalImages.value
  if (total <= 0) return 0
  return ((index % total) + total) % total
}

function syncCurrentIndex(index = currentIndex.value) {
  currentIndex.value = normalizeIndex(index)
}

function clearAutoplayTimer() {
  if (autoplayTimer.value !== null) {
    window.clearInterval(autoplayTimer.value)
    autoplayTimer.value = null
  }
}

function resetAutoplayTimer() {
  clearAutoplayTimer()
  if (!isPlaying.value || totalImages.value <= 1) return
  autoplayTimer.value = window.setInterval(() => {
    goNext()
  }, Math.max(1, autoplaySeconds.value) * 1000)
}

function clearControlsHideTimer() {
  if (controlsHideTimer.value !== null) {
    window.clearTimeout(controlsHideTimer.value)
    controlsHideTimer.value = null
  }
}

function scheduleControlsHide() {
  if (pointerInControls.value) return
  clearControlsHideTimer()
  controlsHideTimer.value = window.setTimeout(() => {
    if (pointerInControls.value) return
    controlsVisible.value = false
    controlsHideTimer.value = null
  }, 1800)
}

function revealControls() {
  controlsVisible.value = true
  scheduleControlsHide()
}

function onControlsPointerEnter() {
  pointerInControls.value = true
  controlsVisible.value = true
  clearControlsHideTimer()
}

function onControlsPointerLeave() {
  pointerInControls.value = false
  scheduleControlsHide()
}

function goPrevious() {
  if (totalImages.value <= 0) return
  if (isRandom.value && totalImages.value > 1) {
    goRandom()
    return
  }
  currentIndex.value = normalizeIndex(currentIndex.value - 1)
  resetAutoplayTimer()
}

function goNext() {
  if (totalImages.value <= 0) return
  if (isRandom.value && totalImages.value > 1) {
    goRandom()
    return
  }
  currentIndex.value = normalizeIndex(currentIndex.value + 1)
  resetAutoplayTimer()
}

function goRandom() {
  const total = totalImages.value
  if (total <= 1) return
  let nextIndex = currentIndex.value
  while (nextIndex === currentIndex.value) {
    nextIndex = Math.floor(Math.random() * total)
  }
  currentIndex.value = nextIndex
  resetAutoplayTimer()
}

function togglePlaying() {
  isPlaying.value = !isPlaying.value
  resetAutoplayTimer()
}

function toggleRandom() {
  isRandom.value = !isRandom.value
}

function toggleFavorite() {
  const image = currentImage.value
  if (!image) return
  emit('favoriteToggle', image.id, !image.isFavorite)
}

function commitAutoplaySeconds() {
  autoplaySeconds.value = normalizeAutoplaySeconds(autoplaySecondsDraft.value)
  autoplaySecondsDraft.value = String(autoplaySeconds.value)
  writeStoredAutoplaySeconds(autoplaySeconds.value)
  resetAutoplayTimer()
}

function commitAutoplaySecondsAndBlur(event: KeyboardEvent) {
  commitAutoplaySeconds()
  ;(event.currentTarget as HTMLInputElement | null)?.blur()
}

function resetMediaTransform() {
  mediaScale.value = 1
  mediaPan.value = { x: 0, y: 0 }
  mediaDrag.value = null
}

function zoomMedia(event: WheelEvent) {
  if (isPlaying.value || !currentImage.value) return
  event.preventDefault()
  const nextScale = Math.min(8, Math.max(1, mediaScale.value * (event.deltaY < 0 ? 1.12 : 0.88)))
  mediaScale.value = nextScale
  if (nextScale === 1) {
    mediaPan.value = { x: 0, y: 0 }
  }
}

function startMediaDrag(event: PointerEvent) {
  if (isPlaying.value || event.button !== 0 || mediaScale.value <= 1) return
  event.preventDefault()
  mediaDrag.value = {
    pointerId: event.pointerId,
    startX: event.clientX,
    startY: event.clientY,
    panX: mediaPan.value.x,
    panY: mediaPan.value.y,
  }
  ;(event.currentTarget as HTMLElement | null)?.setPointerCapture?.(event.pointerId)
}

function moveMediaDrag(event: PointerEvent) {
  const drag = mediaDrag.value
  if (!drag || drag.pointerId !== event.pointerId) return
  mediaPan.value = {
    x: drag.panX + event.clientX - drag.startX,
    y: drag.panY + event.clientY - drag.startY,
  }
}

function finishMediaDrag(event: PointerEvent) {
  const drag = mediaDrag.value
  if (!drag || drag.pointerId !== event.pointerId) return
  mediaDrag.value = null
  ;(event.currentTarget as HTMLElement | null)?.releasePointerCapture?.(event.pointerId)
}

function onKeydown(event: KeyboardEvent) {
  if (event.target instanceof HTMLInputElement) return
  if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
    return
  }
  if (event.key === ' ') {
    event.preventDefault()
    togglePlaying()
    revealControls()
    return
  }
  if (event.key === 'ArrowLeft') {
    event.preventDefault()
    goPrevious()
    revealControls()
    return
  }
  if (event.key === 'ArrowRight') {
    event.preventDefault()
    goNext()
    revealControls()
  }
}

watch(
  () => props.initialIndex,
  (index) => {
    syncCurrentIndex(index)
  },
  { immediate: true },
)

watch(totalImages, () => {
  syncCurrentIndex()
  resetAutoplayTimer()
})

watch(currentIndex, () => {
  resetMediaTransform()
})

watch(isPlaying, (playing) => {
  if (playing) resetMediaTransform()
})

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  resetAutoplayTimer()
  scheduleControlsHide()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  clearAutoplayTimer()
  clearControlsHideTimer()
})
</script>

<template>
  <div class="slideshow-overlay" @mousemove="revealControls">
    <div
      v-if="currentImage"
      class="slideshow-overlay__stage"
      :class="{ 'is-zoomable': !isPlaying }"
      @wheel="zoomMedia"
      @pointerdown="startMediaDrag"
      @pointermove="moveMediaDrag"
      @pointerup="finishMediaDrag"
      @pointercancel="finishMediaDrag"
      @dblclick="resetMediaTransform"
    >
      <img
        class="slideshow-overlay__image"
        :src="currentImageSrc"
        :alt="currentImage.fileName"
        :style="imageStyle"
        draggable="false"
      />
    </div>
    <div v-else class="slideshow-overlay__empty">No images</div>

    <div class="slideshow-overlay__counter" :class="{ 'is-hidden': !controlsVisible }">
      {{ totalImages > 0 ? currentIndex + 1 : 0 }} / {{ totalImages }}
    </div>

    <div
      class="slideshow-overlay__controls"
      :class="{ 'is-hidden': !controlsVisible }"
      @pointerenter="onControlsPointerEnter"
      @pointerleave="onControlsPointerLeave"
    >
      <button type="button" class="slideshow-overlay__button" aria-label="Previous" @click="goPrevious">
        <Left theme="outline" :size="20" :stroke-width="3" :fill="['currentColor']" />
      </button>
      <button
        type="button"
        class="slideshow-overlay__button"
        :aria-label="isPlaying ? 'Pause' : 'Play'"
        @click="togglePlaying"
      >
        <Pause v-if="isPlaying" theme="outline" :size="20" :stroke-width="3" :fill="['currentColor']" />
        <PlayOne v-else theme="outline" :size="20" :stroke-width="3" :fill="['currentColor']" />
      </button>
      <button type="button" class="slideshow-overlay__button" aria-label="Next" @click="goNext">
        <Right theme="outline" :size="20" :stroke-width="3" :fill="['currentColor']" />
      </button>
      <button
        type="button"
        class="slideshow-overlay__button"
        :class="{ 'is-active': currentImage?.isFavorite, 'is-favorite': true }"
        aria-label="Favorite"
        @click="toggleFavorite"
      >
        <Like theme="outline" :size="20" :stroke-width="3" :fill="['currentColor']" />
      </button>
      <button
        type="button"
        class="slideshow-overlay__button"
        :class="{ 'is-active': !isRandom }"
        aria-label="Sequential"
        @click="isRandom = false"
      >
        <SortTwo theme="outline" :size="20" :stroke-width="3" :fill="['currentColor']" />
      </button>
      <button
        type="button"
        class="slideshow-overlay__button"
        :class="{ 'is-active': isRandom }"
        aria-label="Random"
        @click="toggleRandom"
      >
        <ShuffleOne theme="outline" :size="20" :stroke-width="3" :fill="['currentColor']" />
      </button>
      <label class="slideshow-overlay__time" aria-label="Autoplay seconds">
        <Time theme="outline" :size="18" :stroke-width="3" :fill="['currentColor']" />
        <input
          v-model="autoplaySecondsDraft"
          type="number"
          min="1"
          max="60"
          @change="commitAutoplaySeconds"
          @keydown.enter.prevent="commitAutoplaySecondsAndBlur"
          @keydown.stop
        />
        <span>s</span>
      </label>
      <button type="button" class="slideshow-overlay__button" aria-label="Exit" @click="emit('close')">
        <Logout theme="outline" :size="20" :stroke-width="3" :fill="['currentColor']" />
      </button>
    </div>
  </div>
</template>
