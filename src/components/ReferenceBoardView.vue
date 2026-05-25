<script setup lang="ts">
type ReferenceBoard = {
  id: number
  name: string
}

type ReferenceBoardItem = {
  id: number
  x: number
  y: number
  width: number
  height: number
  rotation: number
  zIndex: number
}

type ReferenceBoardImage = {
  id: string
  path: string
}

defineProps<{
  activeReferenceBoard: ReferenceBoard | null
  activeReferenceBoardItems: Array<{ item: ReferenceBoardItem; image: ReferenceBoardImage }>
  boardPan: { x: number; y: number }
  boardScale: number
  selectedReferenceBoardItemId: number | null
  handlers: Record<string, (...args: any[]) => any>
}>()
</script>

<template>
  <section
    class="reference-board-page"
    @wheel="handlers.zoomReferenceBoard($event)"
    @pointerdown="handlers.startBoardPan($event)"
    @contextmenu="handlers.openReferenceBoardCanvasMenu($event)"
  >
    <div v-if="!activeReferenceBoard" class="empty-panel">
      <h2>还没有打开参考板</h2>
      <p>从右侧栏选择一个参考板，或右键新建参考板。</p>
    </div>
    <div v-else-if="activeReferenceBoardItems.length === 0" class="empty-panel">
      <h2>参考板是空的</h2>
      <p>回到图库后，把图片拖到右侧参考板里。</p>
    </div>
    <div
      v-else
      class="reference-board-canvas"
      :style="{
        transform: `translate3d(${boardPan.x}px, ${boardPan.y}px, 0) scale(${boardScale})`,
      }"
    >
      <div
        v-for="{ item, image } in activeReferenceBoardItems"
        :key="item.id"
        class="reference-board-card"
        :data-reference-board-item-id="item.id"
        :class="{ 'is-selected': selectedReferenceBoardItemId === item.id }"
        :style="{
          transform: `translate3d(${item.x}px, ${item.y}px, 0) rotate(${item.rotation}deg)`,
          width: `${item.width}px`,
          height: `${item.height}px`,
          zIndex: item.zIndex,
        }"
        @pointerdown="handlers.startBoardItemMove(item, $event)"
        @contextmenu="handlers.openReferenceBoardItemMenu(item.id, $event)"
      >
        <img :src="handlers.convertFileSrc(image.path)" alt="" draggable="false" />
        <span
          class="reference-board-card__resize"
          @pointerdown="handlers.startBoardItemResize(item, $event)"
        />
      </div>
    </div>
  </section>
</template>
