<script setup lang="ts">
type ReferenceBoardCanvasMenu =
  | { kind: 'item'; itemId: number; x: number; y: number }
  | { kind: 'canvas'; x: number; y: number; worldX: number; worldY: number }
  | null

type DragState = {
  imageId: string
  thumbnailUrl: string
  x: number
  y: number
  overBoardId: number | null
  overRightSidebar: boolean
}

defineProps<{
  referenceBoardCanvasMenu: ReferenceBoardCanvasMenu
  referenceBoardCanvasMenuStyle: Record<string, string | undefined>
  referenceBoardSidebarsDisabled: boolean
  dragState: DragState | null
  handlers: Record<string, (...args: any[]) => any>
}>()
</script>

<template>
  <div
    v-if="referenceBoardCanvasMenu"
    class="context-menu"
    :style="referenceBoardCanvasMenuStyle"
    @click.stop
    @contextmenu.prevent
  >
    <template v-if="referenceBoardCanvasMenu.kind === 'item'">
      <button type="button" @click="handlers.copyReferenceBoardItemToClipboard(referenceBoardCanvasMenu.itemId)">
        复制
      </button>
      <button
        v-if="handlers.canImportReferenceBoardItemToLibrary(referenceBoardCanvasMenu.itemId)"
        type="button"
        @click="handlers.importSelectedReferenceItemToLibrary(referenceBoardCanvasMenu.itemId)"
      >
        加入图库
      </button>
      <button type="button" @click="handlers.exportReferenceBoardItem(referenceBoardCanvasMenu.itemId)">
        导出到本地
      </button>
      <button type="button" @click="handlers.flipReferenceBoardItemHorizontal(referenceBoardCanvasMenu.itemId)">
        水平翻转
      </button>
      <button type="button" @click="handlers.flipReferenceBoardItemVertical(referenceBoardCanvasMenu.itemId)">
        垂直翻转
      </button>
      <button class="is-danger" type="button" @click="handlers.removeReferenceBoardItem(referenceBoardCanvasMenu.itemId)">
        删除
      </button>
    </template>
    <template v-else>
      <button type="button" @click="handlers.pasteReferenceBoardContent(referenceBoardCanvasMenu.worldX, referenceBoardCanvasMenu.worldY)">
        粘贴
      </button>
      <button type="button" @click="handlers.autoArrangeActiveReferenceBoard()">自动排列图片</button>
    </template>
    <button type="button" @click="handlers.toggleReferenceBoardSidebarsDisabled()">
      {{ referenceBoardSidebarsDisabled ? '启用侧栏' : '禁用侧栏' }}
    </button>
  </div>

  <template v-if="dragState">
    <div class="image-drag-preview" :style="{ left: `${dragState.x}px`, top: `${dragState.y}px` }">
      <img :src="dragState.thumbnailUrl" alt="" draggable="false" />
      <span v-if="dragState.overBoardId !== null" class="image-drag-preview__copy-icon">+</span>
    </div>
  </template>
</template>
