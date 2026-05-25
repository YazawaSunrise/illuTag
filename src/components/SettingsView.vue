<script setup lang="ts">
type LibraryFolder = {
  path: string
}

type ThemeMode = 'light' | 'dark'

defineProps<{
  sidebarPinned: boolean
  autoFixRightSidebarOnPreview: boolean
  autoScanOnStartup: boolean
  isBackgroundScanRunning: boolean
  scanProgressText: string
  scanRecentErrors: string[]
  themeMode: ThemeMode
  folderPathInput: string
  isLoading: boolean
  errorText: string
  folders: LibraryFolder[]
  handlers: Record<string, (...args: any[]) => any>
}>()
</script>

<template>
  <section class="settings">
    <div class="settings__header">
      <div>
        <h2>图库文件夹</h2>
        <p>输入或选择一个本地图片文件夹后，主页会按修改时间显示瀑布流。</p>
      </div>
    </div>

    <label class="setting-toggle">
      <input
        :checked="sidebarPinned"
        type="checkbox"
        @change="handlers.setSidebarPinned(($event.target as HTMLInputElement).checked)"
      />
      <span>侧边栏常开</span>
    </label>

    <label class="setting-toggle">
      <input
        :checked="themeMode === 'dark'"
        type="checkbox"
        @change="handlers.setThemeMode(($event.target as HTMLInputElement).checked ? 'dark' : 'light')"
      />
      <span>深色模式</span>
    </label>

    <label class="setting-toggle">
      <input
        :checked="autoFixRightSidebarOnPreview"
        type="checkbox"
        @change="handlers.setAutoFixRightSidebarOnPreview(($event.target as HTMLInputElement).checked)"
      />
      <span>预览参考板时自动固定右侧栏</span>
    </label>

    <label class="setting-toggle">
      <input
        :checked="autoScanOnStartup"
        type="checkbox"
        @change="handlers.setAutoScanOnStartup(($event.target as HTMLInputElement).checked)"
      />
      <span>启动时自动扫描</span>
    </label>

    <button
      class="secondary-button"
      type="button"
      :disabled="isBackgroundScanRunning"
      @click="handlers.startScanAllFolders()"
    >
      {{ isBackgroundScanRunning ? '后台扫描中...' : '开始扫描所有文件夹' }}
    </button>
    <p v-if="scanProgressText" class="settings__progress">{{ scanProgressText }}</p>
    <div v-if="scanRecentErrors.length > 0" class="settings__scan-errors">
      <p class="settings__scan-errors-title">最近扫描错误</p>
      <ul>
        <li v-for="(entry, index) in scanRecentErrors" :key="`${index}-${entry}`">{{ entry }}</li>
      </ul>
    </div>

    <form class="folder-form" @submit.prevent="handlers.addFolder()">
      <input
        :value="folderPathInput"
        class="folder-input"
        type="text"
        placeholder="例如 C:\Users\ASUS\Pictures\Reference"
        autocomplete="off"
        @input="handlers.setFolderPathInput(($event.target as HTMLInputElement).value)"
      />
      <button class="secondary-button" type="button" :disabled="isLoading" @click="handlers.pickFolder()">
        选择
      </button>
      <button class="primary-button" type="submit" :disabled="isLoading">
        添加图库文件夹
      </button>
    </form>

    <p v-if="errorText" class="error">{{ errorText }}</p>

    <div v-if="folders.length > 0" class="folder-list">
      <div v-for="folder in folders" :key="folder.path" class="folder-row">
        <span>{{ folder.path }}</span>
        <button
          class="danger-button"
          type="button"
          :disabled="isLoading"
          @click="handlers.removeFolder(folder.path)"
        >
          移除索引
        </button>
      </div>
    </div>
    <div v-else class="empty-panel">还没有图库文件夹。</div>
  </section>
</template>
