<script setup lang="ts">
import { computed } from 'vue'
import type { GalleryLayoutItem } from '../types/gallery'

const props = defineProps<{
  items: GalleryLayoutItem[]
  totalHeight: number
  contentWidth?: number
  activeDragImageId?: string | null
}>()

const dragImageId = computed(() => props.activeDragImageId ?? null)

const emit = defineEmits<{
  imagePointerDown: [item: GalleryLayoutItem, event: PointerEvent]
  imagePointerUp: []
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
</style>
