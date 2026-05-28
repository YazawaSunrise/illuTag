<script setup lang="ts">
type FolderTreeItem = {
  id: number
  depth: number
  hasChildren: boolean
  isExpanded: boolean
  name: string
}

type ReferenceBoardCanvasMenu =
  | { kind: 'item'; itemId: number; x: number; y: number }
  | { kind: 'canvas'; x: number; y: number; worldX: number; worldY: number }
  | null

type DragState = {
  imageId: string
  thumbnailUrl: string
  x: number
  y: number
  panelX: number
  panelY: number
  overFolderId: number | null
  overBoardId: number | null
  overRightSidebar: boolean
}

defineProps<{
  referenceBoardCanvasMenu: ReferenceBoardCanvasMenu
  referenceBoardCanvasMenuStyle: Record<string, string | undefined>
  dragState: DragState | null
  dropFolderTree: FolderTreeItem[]
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
      <button v-if="handlers.canImportReferenceBoardItemToLibrary(referenceBoardCanvasMenu.itemId)" type="button" @click="handlers.importSelectedReferenceItemToLibrary(referenceBoardCanvasMenu.itemId)">
        加入图库
      </button>
      <button type="button" @click="handlers.exportReferenceBoardItem(referenceBoardCanvasMenu.itemId)">
        导出到本地
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
  </div>

  <template v-if="dragState">
    <div
      v-if="!dragState.overRightSidebar"
      class="folder-drop-panel"
      :style="{ left: `${dragState.panelX}px`, top: `${dragState.panelY}px` }"
    >
      <div class="folder-drop-panel__title">放入文件夹</div>
      <button
        v-for="folder in dropFolderTree"
        :key="folder.id"
        :data-folder-id="folder.id"
        type="button"
        :class="{
          'is-drop-target': dragState.overFolderId === folder.id,
          'is-folder-parent': folder.hasChildren,
        }"
        :style="{ paddingLeft: `${12 + folder.depth * 14}px` }"
      >
        <span v-if="folder.hasChildren" class="folder-drop-panel__twist">
          {{ folder.isExpanded ? '▼' : '▶' }}
        </span>
        {{ folder.name }}
      </button>
      <div v-if="dropFolderTree.length === 0" class="folder-drop-panel__empty">先在左侧栏创建文件夹</div>
    </div>

    <div class="image-drag-preview" :style="{ left: `${dragState.x}px`, top: `${dragState.y}px` }">
      <img :src="dragState.thumbnailUrl" alt="" draggable="false" />
      <span v-if="dragState.overBoardId !== null" class="image-drag-preview__copy-icon">+</span>
    </div>
  </template>
</template>
