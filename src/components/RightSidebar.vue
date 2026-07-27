<script setup lang="ts">
import FolderClose from '@icon-park/vue-next/es/icons/FolderClose'
import FolderOpen from '@icon-park/vue-next/es/icons/FolderOpen'
import Notepad from '@icon-park/vue-next/es/icons/Notepad'
import Pin from '@icon-park/vue-next/es/icons/Pin'

type ReferenceBoardRow =
  | { kind: 'folder'; id: number; name: string; hasBoards: boolean; isExpanded: boolean }
  | { kind: 'board'; id: number; folderId: number | null; name: string; depth: number }

type BoardContextMenu =
  | { kind: 'space'; folderId: number | null; x: number; y: number }
  | { kind: 'folder'; folderId: number; x: number; y: number }
  | { kind: 'board'; boardId: number; x: number; y: number }
  | null

type BoardDraft = {
  kind: 'board' | 'folder'
  folderId: number | null
}

type ReferenceBoardPreviewBlock = {
  boardId: number
  name: string
  thumbnails: Array<{ itemId: number; imageId: string; thumbnailUrl: string }>
}

type PreviewBoardItemDragState = {
  itemId: number
  imageId: string
  sourceBoardId: number
  thumbnailUrl: string
  x: number
  y: number
  targetBoardId: number | null
  targetKind: 'preview' | 'board' | 'gallery' | null
  mode: 'copy' | 'move'
}

type GalleryImageDragState = {
  overBoardId: number | null
}

const props = defineProps<{
  visible: boolean
  rightSidebarPinned: boolean
  referenceBoardRows: ReferenceBoardRow[]
  activeReferenceBoardId: number | null
  previewBoardItemDrag: PreviewBoardItemDragState | null
  galleryImageDragState: GalleryImageDragState | null
  previewReferenceBoardIds: number[]
  referenceBoardPreviewBlocks: ReferenceBoardPreviewBlock[]
  draggedReferenceBoardId: number | null
  draggedReferenceBoardFolderId: number | null
  referenceBoardDragOverKind: 'board' | 'folder' | 'space' | null
  referenceBoardDragOverId: number | null
  boardContextMenu: BoardContextMenu
  boardContextMenuStyle: Record<string, string | undefined>
  boardDraft: BoardDraft | null
  boardDraftStyle: Record<string, string | undefined>
  newBoardName: string
  renamingReferenceBoardFolderId: number | null
  renamingReferenceBoardFolderName: string
  renamingReferenceBoardId: number | null
  renamingReferenceBoardName: string
  handlers: Record<string, (...args: any[]) => any>
}>()

function isPreviewed(boardId: number) {
  return props.previewReferenceBoardIds.includes(boardId)
}

function onBoardDragOver(boardId: number, event: DragEvent) {
  if (props.draggedReferenceBoardId !== null || props.draggedReferenceBoardFolderId !== null) {
    props.handlers.onReferenceBoardDragOverBoard(boardId, event)
    return
  }
  if (props.previewBoardItemDrag) {
    props.handlers.onPreviewBoardItemDragOverBoard(boardId, event)
    return
  }
  props.handlers.onReferenceBoardDragOverBoard(boardId, event)
}

function onBoardDrop(boardId: number, event: DragEvent) {
  if (props.draggedReferenceBoardId !== null || props.draggedReferenceBoardFolderId !== null) {
    props.handlers.dropOnReferenceBoard(boardId, event)
    return
  }
  if (props.previewBoardItemDrag) {
    props.handlers.dropPreviewBoardItem(boardId, event)
    return
  }
  props.handlers.dropOnReferenceBoard(boardId, event)
}

function onBoardClick(boardId: number) {
  if (props.renamingReferenceBoardId === boardId) return
  props.handlers.showReferenceBoard(boardId)
}
</script>

<template>
  <Transition name="sidebar-slide-right">
    <aside
      v-if="visible"
      class="right-sidebar"
      :class="{ 'is-pinned': rightSidebarPinned }"
      @mouseleave="handlers.closeHover($event)"
      @contextmenu="handlers.openBoardSpaceMenu(null, $event)"
    >
      <div class="right-sidebar__header">
        <div class="right-sidebar__section-title">
          <Notepad class="sidebar__section-icon" theme="outline" :size="14" :stroke-width="3" :fill="['currentColor']" />
          <span class="sidebar__section-label">参考板</span>
        </div>
        <button
          class="right-sidebar__pin"
          :class="{ 'is-active': rightSidebarPinned }"
          type="button"
          :aria-pressed="rightSidebarPinned"
          :aria-label="rightSidebarPinned ? '取消固定右侧栏' : '固定右侧栏'"
          :title="rightSidebarPinned ? '取消固定' : '固定右侧栏'"
          @click="handlers.setRightSidebarPinned(!rightSidebarPinned)"
        >
          <Pin
            class="right-sidebar__pin-icon"
            theme="outline"
            :size="14"
            :stroke-width="3"
            :fill="['currentColor']"
            aria-hidden="true"
          />
        </button>
      </div>

      <div v-if="referenceBoardPreviewBlocks.length > 0" class="reference-board-preview">
        <div
          v-for="preview in referenceBoardPreviewBlocks"
          :key="preview.boardId"
          class="reference-board-preview__block"
          :data-reference-board-id="preview.boardId"
          :class="{
            'is-preview-drop-choice':
              previewBoardItemDrag?.targetKind === 'preview' &&
              previewBoardItemDrag?.targetBoardId === preview.boardId &&
              previewBoardItemDrag?.sourceBoardId !== preview.boardId,
            'is-preview-copy-target': galleryImageDragState?.overBoardId === preview.boardId,
          }"
          @click="handlers.showReferenceBoard(preview.boardId)"
          @dragover="handlers.onPreviewBoardItemDragOverPreview(preview.boardId, $event)"
          @drop="handlers.dropPreviewBoardItem(preview.boardId, $event)"
          @contextmenu="handlers.openReferenceBoardMenu(preview.boardId, $event)"
        >
          <div style="display:flex;justify-content:space-between;align-items:center;gap:8px;">
            <span class="reference-board-preview__name">
              {{ preview.name }}
            </span>
          </div>
          <div
            v-if="
              previewBoardItemDrag?.targetKind === 'preview' &&
              previewBoardItemDrag?.targetBoardId === preview.boardId &&
              previewBoardItemDrag?.sourceBoardId !== preview.boardId
            "
            class="preview-drop-choice"
          >
            <div
              class="preview-drop-choice__half"
              :class="{ 'is-active': previewBoardItemDrag.mode === 'move' }"
            >
              移动
            </div>
            <div
              class="preview-drop-choice__half"
              :class="{ 'is-active': previewBoardItemDrag.mode === 'copy' }"
            >
              复制
            </div>
          </div>
          <div
            v-if="preview.thumbnails.length > 0"
            class="reference-board-preview__grid"
            style="--preview-columns: 3;"
          >
            <button
              v-for="thumb in preview.thumbnails"
              :key="thumb.itemId"
              class="reference-board-preview__thumb"
              :class="{ 'is-preview-drag-source': previewBoardItemDrag?.itemId === thumb.itemId }"
              type="button"
              draggable="false"
              @click.stop="handlers.onPreviewReferenceThumbClick(preview.boardId)"
              @pointerdown="
                handlers.startPreviewBoardItemPointerDrag(
                  thumb.itemId,
                  thumb.imageId,
                  preview.boardId,
                  thumb.thumbnailUrl,
                  $event,
                )
              "
            >
              <img :src="thumb.thumbnailUrl" alt="" draggable="false" />
            </button>
          </div>
          <div v-else class="reference-board-preview__empty">参考板为空</div>
        </div>
      </div>

      <div
        class="board-section"
        @contextmenu="handlers.openBoardSpaceMenu(null, $event)"
        @dragover="handlers.onReferenceBoardDragOverSpace($event)"
        @drop="handlers.dropOnReferenceBoardSpace($event)"
        @dragend="handlers.endReferenceBoardDrag($event)"
      >
        <div v-if="referenceBoardRows.length === 0" class="board-section__empty">右键新建参考板</div>
        <template v-for="row in referenceBoardRows" :key="`${row.kind}-${row.id}`">
          <div
            v-if="row.kind === 'folder'"
            class="board-folder-row"
            :data-reference-board-folder-id="row.id"
            :class="{
              'is-dragging': draggedReferenceBoardFolderId === row.id,
              'is-drag-over':
                draggedReferenceBoardFolderId === row.id ||
                (referenceBoardDragOverKind === 'folder' && referenceBoardDragOverId === row.id),
            }"
            draggable="true"
            @dragstart="handlers.startReferenceBoardFolderDrag(row.id, $event)"
            @dragend="handlers.endReferenceBoardDrag($event)"
            @dragover="handlers.onReferenceBoardDragOverFolder(row.id, $event)"
            @drop="handlers.dropOnReferenceBoardFolder(row.id, $event)"
            @contextmenu="handlers.openReferenceBoardFolderMenu(row.id, $event)"
            @click="handlers.onReferenceBoardFolderRowClick(row.id)"
            @dblclick.stop="handlers.startReferenceBoardFolderRename(row.id)"
          >
            <div class="board-folder-row__content">
              <component
                :is="row.isExpanded ? FolderOpen : FolderClose"
                class="board-folder-row__folder-icon"
                theme="outline"
                :size="16"
                :stroke-width="3"
                :fill="['currentColor']"
                aria-hidden="true"
              />
              <span v-if="renamingReferenceBoardFolderId !== row.id" class="board-folder-row__label">
                {{ row.name }}
              </span>
              <input
                v-else
                :data-reference-board-folder-rename-id="row.id"
                class="board-folder-row__rename-input"
                type="text"
                :value="renamingReferenceBoardFolderName"
                autocomplete="off"
                @pointerdown.stop
                @click.stop
                @input="handlers.setRenamingReferenceBoardFolderName(($event.target as HTMLInputElement).value)"
                @keydown.enter="handlers.onReferenceBoardFolderRenameEnter($event)"
                @keydown.esc.prevent="handlers.cancelReferenceBoardFolderRename()"
                @blur="handlers.commitReferenceBoardFolderRename()"
                @compositionstart="handlers.startComposingReferenceBoardFolderRename()"
                @compositionend="handlers.endComposingReferenceBoardFolderRename()"
              />
            </div>
          </div>
          <button
            v-else
            class="board-row"
            type="button"
            :data-reference-board-id="row.id"
            :class="{
              'is-active': activeReferenceBoardId === row.id,
              'is-dragging': draggedReferenceBoardId === row.id,
              'is-drag-over':
                draggedReferenceBoardId === row.id ||
                (referenceBoardDragOverKind === 'board' && referenceBoardDragOverId === row.id),
              'is-preview-drop-choice':
                previewBoardItemDrag?.targetKind === 'board' &&
                previewBoardItemDrag?.targetBoardId === row.id &&
                previewBoardItemDrag?.sourceBoardId !== row.id,
            }"
            :style="{
              marginLeft: `${row.depth * 18}px`,
              width: `calc(100% - ${row.depth * 18}px)`,
            }"
            draggable="true"
            @click="onBoardClick(row.id)"
            @dblclick.stop="handlers.startReferenceBoardRename(row.id)"
            @dragstart="handlers.startReferenceBoardDrag(row.id, $event)"
            @dragend="handlers.endReferenceBoardDrag($event)"
            @dragover="onBoardDragOver(row.id, $event)"
            @drop="onBoardDrop(row.id, $event)"
            @contextmenu="handlers.openReferenceBoardMenu(row.id, $event)"
          >
            <span v-if="renamingReferenceBoardId !== row.id">{{ row.name }}</span>
            <input
              v-else
              :data-reference-board-rename-id="row.id"
              class="board-row__rename-input"
              type="text"
              :value="renamingReferenceBoardName"
              autocomplete="off"
              @pointerdown.stop
              @click.stop
              @input="handlers.setRenamingReferenceBoardName(($event.target as HTMLInputElement).value)"
              @keydown.enter="handlers.onReferenceBoardRenameEnter($event)"
              @keydown.esc.prevent="handlers.cancelReferenceBoardRename()"
              @blur="handlers.commitReferenceBoardRename()"
              @compositionstart="handlers.startComposingReferenceBoardRename()"
              @compositionend="handlers.endComposingReferenceBoardRename()"
            />
            <div
              v-if="
                previewBoardItemDrag?.targetKind === 'board' &&
                previewBoardItemDrag?.targetBoardId === row.id &&
                previewBoardItemDrag?.sourceBoardId !== row.id
              "
              class="board-row__drop-choice"
            >
              <div
                class="board-row__drop-option"
                :class="{ 'is-active': previewBoardItemDrag.mode === 'move' }"
              >
                移动
              </div>
              <div
                class="board-row__drop-option"
                :class="{ 'is-active': previewBoardItemDrag.mode === 'copy' }"
              >
                复制
              </div>
            </div>
          </button>
        </template>
      </div>

      <div
        v-if="boardContextMenu"
        class="context-menu right-sidebar__context-menu"
        :style="boardContextMenuStyle"
        @click.stop
        @contextmenu.stop.prevent
      >
        <template v-if="boardContextMenu.kind === 'space'">
          <button
            type="button"
            @click="
              handlers.openBoardDraft('board', boardContextMenu.folderId, boardContextMenu.x, boardContextMenu.y)
            "
          >
            新建参考板
          </button>
          <button
            v-if="boardContextMenu.folderId === null"
            type="button"
            @click="handlers.openBoardDraft('folder', null, boardContextMenu.x, boardContextMenu.y)"
          >
            新建参考板文件夹
          </button>
        </template>
        <template v-else-if="boardContextMenu.kind === 'folder'">
          <button
            type="button"
            @click="
              handlers.openBoardDraft('board', boardContextMenu.folderId, boardContextMenu.x, boardContextMenu.y)
            "
          >
            新建参考板
          </button>
          <button type="button" @click="handlers.renameReferenceBoardFolder(boardContextMenu.folderId)">
            重命名文件夹
          </button>
          <button class="is-danger" type="button" @click="handlers.deleteReferenceBoardFolder(boardContextMenu.folderId)">
            删除文件夹
          </button>
        </template>
        <template v-else>
          <button type="button" @click="handlers.toggleReferenceBoardPreview(boardContextMenu.boardId)">
            {{ isPreviewed(boardContextMenu.boardId) ? '取消预览参考板' : '预览参考板' }}
          </button>
          <button type="button" @click="handlers.renameReferenceBoard(boardContextMenu.boardId)">
            重命名参考板
          </button>
          <button class="is-danger" type="button" @click="handlers.deleteReferenceBoard(boardContextMenu.boardId)">
            删除参考板
          </button>
        </template>
      </div>

      <form
        v-if="boardDraft"
        class="folder-name-popover"
        :style="boardDraftStyle"
        @submit.prevent="handlers.commitBoardDraft()"
        @click.stop
        @contextmenu.stop.prevent
      >
        <input
          data-board-draft-input
          :value="newBoardName"
          type="text"
          :placeholder="boardDraft.kind === 'folder' ? '输入文件夹名称' : '输入参考板名称'"
          autocomplete="off"
          @input="handlers.setNewBoardName(($event.target as HTMLInputElement).value)"
          @keydown.enter.prevent="handlers.commitBoardDraft()"
          @keydown.esc.prevent="handlers.closeBoardDraft()"
          @compositionstart="handlers.setComposingBoardName(true)"
          @compositionend="handlers.setComposingBoardName(false)"
        />
      </form>
    </aside>
  </Transition>
</template>
