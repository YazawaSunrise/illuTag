<script setup lang="ts">
type LibraryFolder = {
  path: string
}

type ThemeMode = 'light' | 'dark'

defineProps<{
  sidebarPinned: boolean
  autoFixRightSidebarOnPreview: boolean
  thumbnailCacheEnabled: boolean
  isThumbnailGenerationRunning: boolean
  isThumbnailGenerationPaused: boolean
  thumbnailProgressText: string
  thumbnailProgressPercent: number
  thumbnailRecentErrors: string[]
  autoScanOnStartup: boolean
  isBackgroundScanRunning: boolean
  scanProgressText: string
  scanRecentErrors: string[]
  isNaturalLanguageScanRunning: boolean
  naturalLanguageScanProgressText: string
  naturalLanguageScanRecentErrors: string[]
  themeMode: ThemeMode
  clipTestImagePathInput: string
  clipTestTextsInput: string
  clipTestResultText: string
  clipTestRunning: boolean
  folderPathInput: string
  isPickingFolder: boolean
  isAddingFolder: boolean
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
        <p>添加本地图片文件夹后，主页会按修改时间展示瀑布流。</p>
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
        :checked="thumbnailCacheEnabled"
        type="checkbox"
        @change="handlers.setThumbnailCacheEnabled(($event.target as HTMLInputElement).checked)"
      />
      <span>启用缩略图缓存（WebP 768）</span>
    </label>

    <div class="settings__thumbnail-actions">
      <button
        class="secondary-button"
        type="button"
        :disabled="!thumbnailCacheEnabled || isThumbnailGenerationRunning"
        @click="handlers.startThumbnailGeneration()"
      >
        开始生成缩略图
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="!thumbnailCacheEnabled || !isThumbnailGenerationRunning || isThumbnailGenerationPaused"
        @click="handlers.pauseThumbnailGeneration()"
      >
        暂停
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="!thumbnailCacheEnabled || !isThumbnailGenerationRunning || !isThumbnailGenerationPaused"
        @click="handlers.resumeThumbnailGeneration()"
      >
        继续
      </button>
      <button
        class="danger-button"
        type="button"
        :disabled="!thumbnailCacheEnabled || !isThumbnailGenerationRunning"
        @click="handlers.stopThumbnailGeneration()"
      >
        停止
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="!thumbnailCacheEnabled || isThumbnailGenerationRunning"
        @click="handlers.rebuildThumbnailCache()"
      >
        重建缩略图缓存
      </button>
      <button
        class="danger-button"
        type="button"
        :disabled="!thumbnailCacheEnabled || isThumbnailGenerationRunning"
        @click="handlers.clearThumbnailCache()"
      >
        清空缩略图缓存
      </button>
    </div>

    <div v-if="thumbnailProgressText" class="settings__progress-group">
      <p class="settings__progress">{{ thumbnailProgressText }}</p>
      <div
        class="settings__progressbar"
        role="progressbar"
        :aria-valuenow="thumbnailProgressPercent"
        aria-valuemin="0"
        aria-valuemax="100"
      >
        <div class="settings__progressbar-fill" :style="{ width: `${thumbnailProgressPercent}%` }" />
      </div>
    </div>
    <div v-if="thumbnailRecentErrors.length > 0" class="settings__scan-errors">
      <p class="settings__scan-errors-title">缩略图最近错误</p>
      <ul>
        <li v-for="(entry, index) in thumbnailRecentErrors" :key="`thumb-${index}-${entry}`">{{ entry }}</li>
      </ul>
    </div>

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

    <button
      class="secondary-button"
      type="button"
      :disabled="isNaturalLanguageScanRunning"
      @click="handlers.startNaturalLanguageScan()"
    >
      {{ isNaturalLanguageScanRunning ? '自然语言扫描中…' : '开始自然语言扫描（生成图片向量）' }}
    </button>
    <p v-if="naturalLanguageScanProgressText" class="settings__progress">{{ naturalLanguageScanProgressText }}</p>
    <div v-if="naturalLanguageScanRecentErrors.length > 0" class="settings__scan-errors">
      <p class="settings__scan-errors-title">最近自然语言扫描错误</p>
      <ul>
        <li v-for="(entry, index) in naturalLanguageScanRecentErrors" :key="`nl-${index}-${entry}`">
          {{ entry }}
        </li>
      </ul>
    </div>

    <section class="settings__clip-test">
      <h3>Chinese-CLIP 临时检索测试</h3>
      <p>手动选择一张图片，输入候选文本（每行一条），点击测试查看相似度排序。</p>
      <div class="settings__clip-test-row">
        <input
          :value="clipTestImagePathInput"
          class="folder-input"
          type="text"
          placeholder="测试图片路径"
          autocomplete="off"
          @input="handlers.setClipTestImagePathInput(($event.target as HTMLInputElement).value)"
        />
        <button class="secondary-button" type="button" :disabled="clipTestRunning" @click="handlers.pickClipTestImage()">
          选择图片
        </button>
      </div>
      <textarea
        :value="clipTestTextsInput"
        class="settings__clip-test-textarea"
        rows="4"
        placeholder="每行一条候选文本"
        @input="handlers.setClipTestTextsInput(($event.target as HTMLTextAreaElement).value)"
      />
      <div class="settings__clip-test-actions">
        <button class="secondary-button" type="button" :disabled="clipTestRunning" @click="handlers.runClipSearchSmokeTest()">
          {{ clipTestRunning ? '测试中...' : '运行临时检索测试' }}
        </button>
      </div>
      <pre v-if="clipTestResultText" class="settings__clip-test-result">{{ clipTestResultText }}</pre>
    </section>

    <form class="folder-form" @submit.prevent="handlers.addFolder()">
      <input
        :value="folderPathInput"
        class="folder-input"
        type="text"
        placeholder="例如 C:\Users\ASUS\Pictures\Reference"
        autocomplete="off"
        @input="handlers.setFolderPathInput(($event.target as HTMLInputElement).value)"
      />
      <button
        class="secondary-button"
        type="button"
        :disabled="isPickingFolder || isAddingFolder"
        @click="handlers.pickFolder()"
      >
        选择
      </button>
      <button class="primary-button" type="submit" :disabled="isPickingFolder || isAddingFolder">
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
