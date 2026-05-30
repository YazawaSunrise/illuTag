<script setup lang="ts">
type LibraryFolder = {
  path: string
}

type ThemeMode = 'light' | 'dark'

defineProps<{
  sidebarPinned: boolean
  autoHideTitlebarInWindowMode: boolean
  autoFixRightSidebarOnPreview: boolean
  thumbnailCacheEnabled: boolean
  isThumbnailGenerationRunning: boolean
  isThumbnailGenerationPaused: boolean
  thumbnailProgressText: string
  thumbnailProgressPercent: number
  thumbnailRecentErrors: string[]
  isAtmosphereGenerationRunning: boolean
  isAtmosphereGenerationPaused: boolean
  atmosphereProgressText: string
  atmosphereProgressPercent: number
  atmosphereRecentErrors: string[]
  isColorSignatureGenerationRunning: boolean
  isColorSignatureGenerationPaused: boolean
  colorSignatureProgressText: string
  colorSignatureProgressPercent: number
  colorSignatureRecentErrors: string[]
  autoScanOnStartup: boolean
  isOneClickScanRunning: boolean
  isBackgroundScanRunning: boolean
  isBackgroundScanPaused: boolean
  scanProgressText: string
  scanRecentErrors: string[]
  isNaturalLanguageScanRunning: boolean
  isNaturalLanguageScanPaused: boolean
  naturalLanguageScanProgressText: string
  naturalLanguageScanRecentErrors: string[]
  themeMode: ThemeMode
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
        <h2>Tips</h2>
        <p>推荐按从上往下的顺序进行生成缩略图、自然语言等工作</p>
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
        :checked="autoHideTitlebarInWindowMode"
        type="checkbox"
        @change="handlers.setAutoHideTitlebarInWindowMode(($event.target as HTMLInputElement).checked)"
      />
      <span>窗口模式下自动隐藏标题行</span>
    </label>

    <label class="setting-toggle">
      <input
        :checked="themeMode === 'dark'"
        type="checkbox"
        @change="handlers.setThemeMode(($event.target as HTMLInputElement).checked ? 'dark' : 'light')"
      />
      <span>深色模式（实验）</span>
    </label>

    <label class="setting-toggle">
      <input
        :checked="autoFixRightSidebarOnPreview"
        type="checkbox"
        @change="handlers.setAutoFixRightSidebarOnPreview(($event.target as HTMLInputElement).checked)"
      />
      <span>开启预览参考板时自动固定右侧栏</span>
    </label>

    <label class="setting-toggle">
      <input
        :checked="thumbnailCacheEnabled"
        type="checkbox"
        @change="handlers.setThumbnailCacheEnabled(($event.target as HTMLInputElement).checked)"
      />
      <span>启用缩略图缓存</span>
    </label>

    <label class="setting-toggle">
      <input
        :checked="autoScanOnStartup"
        type="checkbox"
        @change="handlers.setAutoScanOnStartup(($event.target as HTMLInputElement).checked)"
      />
      <span>启动时自动扫描</span>
    </label>

    <div class="settings__thumbnail-actions">
      <button
        class="secondary-button"
        type="button"
        :disabled="isOneClickScanRunning"
        @click="handlers.runOneClickScan()"
      >
        {{ isOneClickScanRunning ? '扫描/创建索引文件中…' : '扫描并创建所有索引文件' }}
      </button>
    </div>

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

    <div class="settings__thumbnail-actions">
      <button
        class="secondary-button"
        type="button"
        :disabled="isNaturalLanguageScanRunning"
        @click="handlers.startNaturalLanguageScan()"
      >
        {{ isNaturalLanguageScanRunning ? '自然语言扫描中…' : '开始自然语言扫描' }}
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="!isNaturalLanguageScanRunning || isNaturalLanguageScanPaused"
        @click="handlers.pauseNaturalLanguageScan()"
      >
        暂停
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="!isNaturalLanguageScanRunning || !isNaturalLanguageScanPaused"
        @click="handlers.resumeNaturalLanguageScan()"
      >
        继续
      </button>
      <button
        class="danger-button"
        type="button"
        :disabled="!isNaturalLanguageScanRunning"
        @click="handlers.stopNaturalLanguageScan()"
      >
        停止
      </button>
    </div>
    <p v-if="naturalLanguageScanProgressText" class="settings__progress">{{ naturalLanguageScanProgressText }}</p>
    <div v-if="naturalLanguageScanRecentErrors.length > 0" class="settings__scan-errors">
      <p class="settings__scan-errors-title">最近自然语言扫描错误</p>
      <ul>
        <li v-for="(entry, index) in naturalLanguageScanRecentErrors" :key="`nl-${index}-${entry}`">
          {{ entry }}
        </li>
      </ul>
    </div>

    <div class="settings__thumbnail-actions">
      <button
        class="secondary-button"
        type="button"
        :disabled="isAtmosphereGenerationRunning"
        @click="handlers.startAtmosphereGeneration()"
      >
        开始生成氛围特征
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="!isAtmosphereGenerationRunning || isAtmosphereGenerationPaused"
        @click="handlers.pauseAtmosphereGeneration()"
      >
        暂停
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="!isAtmosphereGenerationRunning || !isAtmosphereGenerationPaused"
        @click="handlers.resumeAtmosphereGeneration()"
      >
        继续
      </button>
      <button
        class="danger-button"
        type="button"
        :disabled="!isAtmosphereGenerationRunning"
        @click="handlers.stopAtmosphereGeneration()"
      >
        停止
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="isAtmosphereGenerationRunning"
        @click="handlers.rebuildAtmosphereSignatureCache()"
      >
        重建氛围特征
      </button>
    </div>

    <div v-if="atmosphereProgressText" class="settings__progress-group">
      <p class="settings__progress">{{ atmosphereProgressText }}</p>
      <div
        class="settings__progressbar"
        role="progressbar"
        :aria-valuenow="atmosphereProgressPercent"
        aria-valuemin="0"
        aria-valuemax="100"
      >
        <div class="settings__progressbar-fill" :style="{ width: `${atmosphereProgressPercent}%` }" />
      </div>
    </div>
    <div v-if="atmosphereRecentErrors.length > 0" class="settings__scan-errors">
      <p class="settings__scan-errors-title">氛围特征最近错误</p>
      <ul>
        <li v-for="(entry, index) in atmosphereRecentErrors" :key="`atm-${index}-${entry}`">{{ entry }}</li>
      </ul>
    </div>

    <div class="settings__thumbnail-actions">
      <button
        class="secondary-button"
        type="button"
        :disabled="isColorSignatureGenerationRunning"
        @click="handlers.startColorSignatureGeneration()"
      >
        开始生成配色特征
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="!isColorSignatureGenerationRunning || isColorSignatureGenerationPaused"
        @click="handlers.pauseColorSignatureGeneration()"
      >
        暂停
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="!isColorSignatureGenerationRunning || !isColorSignatureGenerationPaused"
        @click="handlers.resumeColorSignatureGeneration()"
      >
        继续
      </button>
      <button
        class="danger-button"
        type="button"
        :disabled="!isColorSignatureGenerationRunning"
        @click="handlers.stopColorSignatureGeneration()"
      >
        停止
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="isColorSignatureGenerationRunning"
        @click="handlers.rebuildColorSignatureCache()"
      >
        重建配色特征
      </button>
    </div>

    <div v-if="colorSignatureProgressText" class="settings__progress-group">
      <p class="settings__progress">{{ colorSignatureProgressText }}</p>
      <div
        class="settings__progressbar"
        role="progressbar"
        :aria-valuenow="colorSignatureProgressPercent"
        aria-valuemin="0"
        aria-valuemax="100"
      >
        <div class="settings__progressbar-fill" :style="{ width: `${colorSignatureProgressPercent}%` }" />
      </div>
    </div>
    <div v-if="colorSignatureRecentErrors.length > 0" class="settings__scan-errors">
      <p class="settings__scan-errors-title">配色特征最近错误</p>
      <ul>
        <li v-for="(entry, index) in colorSignatureRecentErrors" :key="`color-${index}-${entry}`">{{ entry }}</li>
      </ul>
    </div>

    <div class="settings__thumbnail-actions">
      <button
        class="secondary-button"
        type="button"
        :disabled="isBackgroundScanRunning"
        @click="handlers.startScanAllFolders()"
      >
        {{ isBackgroundScanRunning ? '扫描/标注中...' : '开始标注标签' }}
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="!isBackgroundScanRunning || isBackgroundScanPaused"
        @click="handlers.pauseScanAllFolders()"
      >
        暂停
      </button>
      <button
        class="secondary-button"
        type="button"
        :disabled="!isBackgroundScanRunning || !isBackgroundScanPaused"
        @click="handlers.resumeScanAllFolders()"
      >
        继续
      </button>
      <button
        class="danger-button"
        type="button"
        :disabled="!isBackgroundScanRunning"
        @click="handlers.stopScanAllFolders()"
      >
        停止
      </button>
    </div>
    <p v-if="scanProgressText" class="settings__progress">{{ scanProgressText }}</p>
    <div v-if="scanRecentErrors.length > 0" class="settings__scan-errors">
      <p class="settings__scan-errors-title">最近扫描错误</p>
      <ul>
        <li v-for="(entry, index) in scanRecentErrors" :key="`${index}-${entry}`">{{ entry }}</li>
      </ul>
    </div>

    <form class="folder-form" @submit.prevent>
      <input
        :value="folderPathInput"
        class="folder-input"
        type="text"
        placeholder="例如 C:\Users\ASUS\Pictures\Reference"
        autocomplete="off"
        @input="handlers.setFolderPathInput(($event.target as HTMLInputElement).value)"
      />
      <button class="primary-button" type="button" :disabled="isPickingFolder || isAddingFolder" @click="handlers.pickFolder()">
        选择并添加图库文件夹
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
