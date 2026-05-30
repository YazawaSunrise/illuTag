<script setup lang="ts">
import {
  AllApplication,
  Delete,
  FolderClose,
  FolderFocusOne,
  FolderOpen,
  Like,
  SettingTwo,
  ShuffleOne,
  Tag,
  Triangle,
  WaterfallsV,
} from '@icon-park/vue-next'

type ViewMode = 'gallery' | 'settings' | 'board'

type FolderTreeItem = {
  id: number
  depth: number
  hasChildren: boolean
  isExpanded: boolean
  name: string
}

type FolderContextMenu =
  | { kind: 'space'; x: number; y: number }
  | { kind: 'folder'; folderId: number; x: number; y: number }
  | null

type FolderDraft = {
  parentId: number | null
  x: number
  y: number
}

defineProps<{
  visible: boolean
  sidebarPinned: boolean
  viewMode: ViewMode
  activeUserFolderId: number | 'all' | 'random' | 'favorites' | 'trash'
  tagManagerOpen: boolean
  folderTree: FolderTreeItem[]
  folderDragOverId: number | null
  draggedFolderId: number | null
  renamingUserFolderId: number | null
  renamingUserFolderName: string
  folderContextMenu: FolderContextMenu
  contextMenuStyle: Record<string, string | undefined>
  folderDraft: FolderDraft | null
  folderDraftStyle: Record<string, string | undefined>
  newFolderName: string
  handlers: Record<string, (...args: any[]) => any>
}>()
</script>

<template>
  <Transition name="sidebar-slide-left">
    <aside
      v-if="visible"
      class="sidebar"
      :class="{ 'is-pinned': sidebarPinned }"
      @mouseleave="handlers.closeHover($event)"
    >
      <div class="sidebar__header">
        <div class="sidebar__brand">
          <span class="sidebar__mark">iT</span>
          <span>illuTag</span>
        </div>
        <button
          v-if="false"
          class="sidebar__toggle"
          type="button"
          aria-label="隐藏侧边栏"
          @click="handlers.closeByToggle()"
        >
          <span />
          <span />
          <span />
        </button>
      </div>

      <div class="sidebar__body">
        <div class="sidebar-gallery-section__title">
          <WaterfallsV class="sidebar__section-icon" theme="outline" :size="14" :stroke-width="3" :fill="['currentColor']" />
          <span class="sidebar__section-label">瀑布流</span>
        </div>

        <button
          class="sidebar__nav-button"
          type="button"
          :class="{ 'is-active': viewMode === 'gallery' && activeUserFolderId === 'all' }"
          @click="handlers.showAllImages()"
        >
          <span class="sidebar__nav-content">
            <AllApplication class="sidebar__nav-icon" theme="outline" :size="15" :stroke-width="3" :fill="['currentColor']" />
            <span class="sidebar__nav-label">全部</span>
          </span>
        </button>

        <button
          class="sidebar__nav-button"
          type="button"
          :class="{ 'is-active': viewMode === 'gallery' && activeUserFolderId === 'random' }"
          @click="handlers.showRandomImages()"
        >
          <span class="sidebar__nav-content">
            <ShuffleOne class="sidebar__nav-icon" theme="outline" :size="15" :stroke-width="3" :fill="['currentColor']" />
            <span class="sidebar__nav-label">随机</span>
          </span>
        </button>

        <button
          class="sidebar__nav-button"
          type="button"
          :class="{ 'is-active': viewMode === 'gallery' && activeUserFolderId === 'favorites' }"
          @click="handlers.showFavoriteImages()"
        >
          <span class="sidebar__nav-content">
            <Like class="sidebar__nav-icon" theme="outline" :size="15" :stroke-width="3" :fill="['currentColor']" />
            <span class="sidebar__nav-label">我喜爱的</span>
          </span>
        </button>

        <button
          class="sidebar__nav-button"
          type="button"
          :class="{ 'is-active': viewMode === 'gallery' && activeUserFolderId === 'trash' }"
          @click="handlers.showTrashImages()"
        >
          <span class="sidebar__nav-content">
            <Delete class="sidebar__nav-icon" theme="outline" :size="15" :stroke-width="3" :fill="['currentColor']" />
            <span class="sidebar__nav-label">回收站</span>
          </span>
        </button>

        <div class="sidebar-tag-manager">
          <div class="sidebar-tag-manager__title">
            <Tag class="sidebar__section-icon" theme="outline" :size="14" :stroke-width="3" :fill="['currentColor']" />
            <span class="sidebar__section-label">标签</span>
          </div>
          <button
            class="sidebar__nav-button"
            type="button"
            :class="{ 'is-active': tagManagerOpen }"
            @click="handlers.openTagManager()"
          >
            <span class="sidebar__nav-content">
              <Tag class="sidebar__nav-icon" theme="outline" :size="15" :stroke-width="3" :fill="['currentColor']" />
              <span class="sidebar__nav-label">标签管理</span>
            </span>
          </button>
        </div>

        <div class="folder-section" @contextmenu="handlers.openFolderSectionMenu($event)">
          <div class="folder-section__title">
            <FolderFocusOne class="sidebar__section-icon" theme="outline" :size="14" :stroke-width="3" :fill="['currentColor']" />
            <span class="sidebar__section-label">文件夹</span>
          </div>
          <div class="folder-tree">
            <div
              v-for="folder in folderTree"
              :key="folder.id"
              class="folder-tree__row"
              :data-sidebar-folder-id="folder.id"
              :class="{
                'is-active': activeUserFolderId === folder.id,
                'is-drag-over': folderDragOverId === folder.id,
                'is-dragging': draggedFolderId === folder.id,
              }"
              :style="{ paddingLeft: `${8 + folder.depth * 16}px` }"
              @contextmenu="handlers.openFolderMenu(folder.id, $event)"
              @pointerdown="handlers.startFolderPointer(folder.id, $event)"
              @click="handlers.onUserFolderRowClick(folder)"
              @dblclick.stop="handlers.startUserFolderRename(folder.id)"
            >
              <button
                class="folder-tree__twist"
                type="button"
                :class="{ 'is-hidden': !folder.hasChildren, 'is-expanded': folder.isExpanded }"
                :aria-label="folder.isExpanded ? '收起文件夹' : '展开文件夹'"
                @click.stop="handlers.toggleFolderExpanded(folder.id)"
              >
                <Triangle
                  class="folder-tree__twist-icon"
                  theme="filled"
                  :size="9"
                  :fill="['currentColor']"
                  aria-hidden="true"
                />
              </button>
              <div class="folder-tree__content">
                <component
                  :is="folder.isExpanded ? FolderOpen : FolderClose"
                  class="folder-tree__folder-icon"
                  theme="outline"
                  :size="16"
                  :stroke-width="3"
                  :fill="['currentColor']"
                  aria-hidden="true"
                />
                <button
                  v-if="renamingUserFolderId !== folder.id"
                  class="folder-tree__item"
                  type="button"
                >
                  <span class="folder-tree__item-label">{{ folder.name }}</span>
                </button>
                <input
                  v-else
                  :data-user-folder-rename-id="folder.id"
                  class="folder-tree__rename-input"
                  type="text"
                  :value="renamingUserFolderName"
                  autocomplete="off"
                  @pointerdown.stop
                  @click.stop
                  @input="handlers.setRenamingUserFolderName(($event.target as HTMLInputElement).value)"
                  @keydown.enter="handlers.onUserFolderRenameEnter($event)"
                  @keydown.esc.prevent="handlers.cancelUserFolderRename()"
                  @blur="handlers.commitUserFolderRename()"
                  @compositionstart="handlers.startComposingUserFolderRename()"
                  @compositionend="handlers.endComposingUserFolderRename()"
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      <button
        class="sidebar__settings-button"
        type="button"
        :class="{ 'is-active': viewMode === 'settings' }"
        aria-label="设置"
        title="设置"
        @click="handlers.openSettings()"
      >
        <SettingTwo
          class="sidebar__settings-icon"
          theme="outline"
          :size="18"
          :stroke-width="3"
          :fill="['currentColor']"
          aria-hidden="true"
        />
      </button>

      <div
        v-if="folderContextMenu"
        class="context-menu"
        :style="contextMenuStyle"
        @click.stop
        @contextmenu.prevent
      >
        <template v-if="folderContextMenu.kind === 'space'">
          <button type="button" @click="handlers.openCreateFolderDraft(null, folderContextMenu.x, folderContextMenu.y)">
            新建文件夹
          </button>
        </template>
        <template v-else>
          <button
            type="button"
            @click="handlers.openCreateFolderDraft(folderContextMenu.folderId, folderContextMenu.x, folderContextMenu.y)"
          >
            新建子文件夹
          </button>
          <button type="button" @click="handlers.startUserFolderRename(folderContextMenu.folderId)">
            重命名
          </button>
          <button class="is-danger" type="button" @click="handlers.deleteUserFolder(folderContextMenu.folderId)">
            删除文件夹
          </button>
        </template>
      </div>

      <form
        v-if="folderDraft"
        class="folder-name-popover"
        :style="folderDraftStyle"
        @submit.prevent="handlers.commitFolderDraft()"
        @click.stop
        @contextmenu.prevent
      >
        <input
          data-folder-draft-input
          :value="newFolderName"
          type="text"
          placeholder="输入文件夹名称"
          autocomplete="off"
          @input="handlers.setNewFolderName(($event.target as HTMLInputElement).value)"
          @keydown.enter.prevent="handlers.commitFolderDraft()"
          @keydown.esc.prevent="handlers.closeCreateFolderDraft()"
          @compositionstart="handlers.setComposingFolderName(true)"
          @compositionend="handlers.setComposingFolderName(false)"
        />
      </form>
    </aside>
  </Transition>
</template>
