import { invoke } from '@tauri-apps/api/core'

type DevFakeGalleryOptions = {
  count: number
  sourceFolder?: string | null
  sampleImageIds?: string[] | null
  randomizeDimensions?: boolean
  randomizeFileNames?: boolean
  randomizeImportedAt?: boolean
  randomizeFolders?: boolean
  randomizeFavorites?: boolean
  randomizeTags?: boolean
}

type DevSmallFileSetOptions = {
  count: number
  rootDir?: string | null
  format?: 'png' | 'jpg' | 'jpeg' | string | null
  subfolderCount?: number | null
}

type DevStressToolResult = {
  createdImages: number
  createdFiles: number
  deletedImages: number
  deletedFiles: number
  rootPath?: string | null
  databasePath: string
}

export type IlluTagDevStressTools = {
  createFakeGalleryData: (options: DevFakeGalleryOptions) => Promise<DevStressToolResult>
  createSmallFileTestSet: (options: DevSmallFileSetOptions) => Promise<DevStressToolResult>
  cleanupStressTestData: () => Promise<DevStressToolResult>
}

export function installDevStressTools() {
  const tools: IlluTagDevStressTools = {
    createFakeGalleryData(options) {
      return invoke<DevStressToolResult>('dev_create_fake_gallery_data_command', { options })
    },
    createSmallFileTestSet(options) {
      return invoke<DevStressToolResult>('dev_create_small_file_test_set_command', { options })
    },
    cleanupStressTestData() {
      return invoke<DevStressToolResult>('dev_cleanup_stress_test_data_command')
    },
  }

  ;(window as Window & { illuTagDevStress?: IlluTagDevStressTools }).illuTagDevStress = tools
  console.info('[dev-stress] installed window.illuTagDevStress')
  return tools
}
