<script setup lang="ts">
import { ref } from 'vue'

type LibraryFolder = {
  path: string
}

type ThemeMode = 'light' | 'dark'
type ExportDialogMode = 'migration' | 'organized' | null

const exportDialogMode = ref<ExportDialogMode>(null)

function openExportDialog(mode: Exclude<ExportDialogMode, null>) {
  exportDialogMode.value = mode
}

function closeExportDialog() {
  exportDialogMode.value = null
}

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
  <section class="settings" @scroll.passive="handlers.onSettingsScroll?.($event)">
    <div class="settings__header">
      <div>
        <h2>Tips</h2>
        <p>推荐按从上往下的顺序进行生成缩略图、自然语言等工作，或直接点击“扫描并创建所有索引文件”</p>
        
      </div>
    </div>

    <label class="setting-toggle">
      <input
        :checked="sidebarPinned"
        type="checkbox"
        @change="handlers.setSidebarPinned(($event.target as HTMLInputElement).checked)"
      />
      <span>左侧栏常开</span>
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
        开始计算氛围特征
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
        开始计算配色特征
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

    <div class="settings__export-actions">
      <button class="secondary-button" type="button" @click="openExportDialog('migration')">
        数据迁移备份
      </button>
      <button class="secondary-button" type="button" @click="openExportDialog('organized')">
        整理结果导出
      </button>
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

    <div v-if="exportDialogMode" class="settings-export-dialog-layer" @click="closeExportDialog()">
      <article class="settings-export-dialog" @click.stop>
        <header class="settings-export-dialog__header">
          <h3>{{ exportDialogMode === 'migration' ? '数据迁移备份' : '整理结果导出' }}</h3>
          <button type="button" class="settings-export-dialog__close" @click="closeExportDialog()">×</button>
        </header>

        <div v-if="exportDialogMode === 'migration'" class="settings-export-dialog__body">
          <p class="settings-export-dialog__lead">
            用于换电脑、重装系统或迁移绿色版目录后继续使用 illuTag。该备份只导出软件配置和索引，不导出原图。
          </p>
          <section class="settings-export-dialog__section">
            <h4>导出内容</h4>
            <p>
              图库路径、图片索引、用户文件夹树、图片-文件夹关系、收藏/回收站状态、自定义标签、标签管理、文件夹规则、参考板和设置。
            </p>
          </section>
          <section class="settings-export-dialog__section">
            <h4>导入方式</h4>
            <p>
              导入时会检查原图库路径是否仍然存在；如果路径变化，需要把旧路径重映射到新电脑上的实际图库路径。
            </p>
          </section>
          <div class="settings-export-dialog__actions">
            <button class="secondary-button" type="button" @click="handlers.importMigrationBackup()">
              导入备份
            </button>
            <button class="primary-button" type="button" @click="handlers.exportMigrationBackup()">
              导出备份
            </button>
          </div>
        </div>

        <div v-else class="settings-export-dialog__body">
          <p class="settings-export-dialog__lead">
            用于不继续使用 illuTag 时，导出一份普通文件夹结构，方便在系统文件管理器或其他软件中继续整理。
          </p>
          <section class="settings-export-dialog__section">
            <h4>导出规则</h4>
            <p>
              根据软件内用户文件夹树创建本地目录，并把对应图片复制到目录。默认复制，不移动原图。
            </p>
          </section>
          <section class="settings-export-dialog__section">
            <h4>重复图片与清单</h4>
            <p>
              同一图片属于多个文件夹时默认复制多份；同时生成 _illuTag_export_manifest.json，记录原始路径、图片 id 和导出路径。
            </p>
          </section>
          <div class="settings-export-dialog__actions">
            <button class="primary-button" type="button" @click="handlers.exportOrganizedFolderResult()">
              选择导出目录
            </button>
          </div>
        </div>
      </article>
    </div>
  </section>
</template>
