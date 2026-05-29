<script setup lang="ts">
import { computed, ref } from 'vue'
import { Like } from '@icon-park/vue-next'
import type { GalleryLayoutItem } from '../types/gallery'

const props = defineProps<{
  items: GalleryLayoutItem[]
  favoriteImageIds?: string[]
  totalHeight: number
  contentWidth?: number
  activeDragImageId?: string | null
}>()

const dragImageId = computed(() => props.activeDragImageId ?? null)
const favoriteImageIdSet = computed(() => new Set(props.favoriteImageIds ?? []))
const animatingFavoriteImageId = ref<string | null>(null)
let favoriteAnimationTimer: number | null = null

const emit = defineEmits<{
  imagePointerDown: [item: GalleryLayoutItem, event: PointerEvent]
  imagePointerUp: []
  imageFavoriteToggle: [imageId: string, favorite: boolean]
  imageClick: [item: GalleryLayoutItem]
  imageContextMenu: [item: GalleryLayoutItem, event: MouseEvent]
}>()

function onItemClick(item: GalleryLayoutItem, event: MouseEvent) {
  if (dragImageId.value === item.id) {
    event.preventDefault()
    event.stopPropagation()
    return
  }
  emit('imageClick', item)
}

function onFavoriteClick(item: GalleryLayoutItem) {
  const nextFavorite = !favoriteImageIdSet.value.has(item.id)
  if (favoriteAnimationTimer !== null) {
    window.clearTimeout(favoriteAnimationTimer)
    favoriteAnimationTimer = null
  }
  animatingFavoriteImageId.value = item.id
  favoriteAnimationTimer = window.setTimeout(() => {
    animatingFavoriteImageId.value = null
    favoriteAnimationTimer = null
  }, 240)
  emit('imageFavoriteToggle', item.id, nextFavorite)
}
</script>

<template>
  <div
    class="masonry"
    :style="{ height: `${totalHeight}px`, width: `${contentWidth ?? 0}px`, margin: '0 auto' }"
  >
    <button
      v-for="item in items"
      :key="item.id"
      class="masonry__item"
      :data-gallery-image-id="item.id"
      type="button"
      :class="{ 'is-being-sorted': dragImageId === item.id }"
      :style="{
        transform: `translate3d(${item.x}px, ${item.y}px, 0)`,
        width: `${item.width}px`,
        height: `${item.height}px`,
      }"
      @pointerdown="$emit('imagePointerDown', item, $event)"
      @pointerup="$emit('imagePointerUp')"
      @click="onItemClick(item, $event)"
      @contextmenu="$emit('imageContextMenu', item, $event)"
    >
      <img :src="item.thumbnailUrl" alt="" draggable="false" />
      <button
        class="masonry__favorite"
        type="button"
        :class="{
          'is-active': favoriteImageIdSet.has(item.id),
          'is-animating': animatingFavoriteImageId === item.id,
        }"
        :aria-label="favoriteImageIdSet.has(item.id) ? '取消喜欢' : '标记为喜欢'"
        @pointerdown.stop.prevent
        @pointerup.stop.prevent
        @click.stop="onFavoriteClick(item)"
      >
        <Like
          class="masonry__favorite-icon"
          theme="filled"
          :size="14"
          :fill="['currentColor']"
          aria-hidden="true"
        />
      </button>
    </button>
  </div>
</template>

<style scoped>
.masonry {
  position: relative;
  width: 100%;
}

.masonry__item {
  position: absolute;
  left: 0;
  top: 0;
  display: block;
  overflow: hidden;
  padding: 0;
  border: 0;
  border-radius: 6px;
  background:
    linear-gradient(135deg, rgb(255 255 255 / 0.04), rgb(255 255 255 / 0)),
    #20242a;
  cursor: pointer;
  touch-action: none;
}

.masonry__favorite {
  position: absolute;
  right: 8px;
  top: 8px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 0;
  border-radius: 6px;
  background: rgb(26 28 31 / 0.4);
  opacity: 0;
  transform: translateY(-1px);
  transition:
    opacity 120ms ease,
    background-color 140ms ease;
}

.masonry__item:hover .masonry__favorite,
.masonry__favorite.is-active {
  opacity: 1;
}

.masonry__favorite-icon {
  color: rgb(154 160 168 / 0.95);
  transition:
    color 160ms ease,
    filter 160ms ease;
}

.masonry__favorite.is-active .masonry__favorite-icon {
  color: #ea4343;
  filter: drop-shadow(0 0 6px rgb(234 67 67 / 0.42));
}

.masonry__favorite.is-animating .masonry__favorite-icon {
  animation: masonry-favorite-pop 220ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.masonry__favorite:focus-visible {
  outline: 2px solid rgb(235 110 110 / 0.8);
  outline-offset: 1px;
}

.masonry__item:focus-visible {
  outline: 2px solid #4da3ff;
  outline-offset: 2px;
}

.masonry__item img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
  user-select: none;
  transition: filter 120ms ease, opacity 120ms ease;
}

.masonry__item:hover img,
.masonry__item.is-being-sorted img {
  opacity: 0.8;
  filter: brightness(0.84);
}

@keyframes masonry-favorite-pop {
  0% {
    transform: scale(0.9);
  }

  55% {
    transform: scale(1.12);
  }

  100% {
    transform: scale(1);
  }
}
</style>
