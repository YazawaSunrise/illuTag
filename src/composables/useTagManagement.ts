import { ref } from 'vue'

type TagManagementFolder = {
  id: number
  name: string
  sortOrder: number
  tags: string[]
}

type TagManagementState = {
  folders: TagManagementFolder[]
  unclassifiedTags: string[]
}

type UseTagManagementOptions = {
  formatError: (error: unknown) => string
  setErrorText: (value: string) => void
}

export function useTagManagement(options: UseTagManagementOptions) {
  const tagManagerOpen = ref(false)
  const isTagManagerLoading = ref(false)
  const tagManagerFolders = ref<TagManagementFolder[]>([])
  const activeTagManagerFolderId = ref<number | null>(null)
  const tagManagerUnclassifiedTags = ref<string[]>([])
  const newTagManagerFolderName = ref('')
  const newTagManagerTagText = ref('')

  function normalizeState(raw: Record<string, unknown>): TagManagementState {
    const foldersRaw = Array.isArray(raw.folders) ? raw.folders : []
    const unclassifiedRaw = Array.isArray(raw.unclassifiedTags ?? raw.unclassified_tags)
      ? (raw.unclassifiedTags ?? raw.unclassified_tags)
      : []
    const folders = foldersRaw
      .filter((item): item is Record<string, unknown> => typeof item === 'object' && item !== null)
      .map((item) => ({
        id: Number(item.id ?? 0),
        name: String(item.name ?? ''),
        sortOrder: Number(item.sortOrder ?? item.sort_order ?? 0),
        tags: Array.isArray(item.tags)
          ? item.tags.map((value) => String(value ?? '').trim()).filter((value) => value.length > 0)
          : [],
      }))
      .filter((item) => Number.isFinite(item.id) && item.id > 0 && item.name.trim().length > 0)
    const unclassifiedTags: string[] = (unclassifiedRaw as unknown[])
      .map((item) => String(item ?? '').trim())
      .filter((value) => value.length > 0)
    return { folders, unclassifiedTags }
  }

  function applyState(next: TagManagementState) {
    tagManagerFolders.value = next.folders
    tagManagerUnclassifiedTags.value = next.unclassifiedTags
    if (
      activeTagManagerFolderId.value !== null &&
      !next.folders.some((folder) => folder.id === activeTagManagerFolderId.value)
    ) {
      activeTagManagerFolderId.value = null
    }
    if (activeTagManagerFolderId.value === null && next.folders.length > 0) {
      activeTagManagerFolderId.value = next.folders[0].id
    }
  }

  async function reloadTagManagementState() {
    try {
      isTagManagerLoading.value = true
      const { invoke } = await import('@tauri-apps/api/core')
      const raw = await invoke<Record<string, unknown>>('list_tag_management_state_command')
      applyState(normalizeState(raw))
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      isTagManagerLoading.value = false
    }
  }

  async function openTagManager() {
    tagManagerOpen.value = true
    await reloadTagManagementState()
  }

  function closeTagManager() {
    tagManagerOpen.value = false
  }

  async function createTagManagerFolder() {
    const name = newTagManagerFolderName.value.trim()
    if (!name) return
    try {
      isTagManagerLoading.value = true
      const { invoke } = await import('@tauri-apps/api/core')
      const raw = await invoke<Record<string, unknown>>('create_user_tag_folder_command', { name })
      newTagManagerFolderName.value = ''
      applyState(normalizeState(raw))
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      isTagManagerLoading.value = false
    }
  }

  async function createTagManagerTag() {
    const tagText = newTagManagerTagText.value.trim()
    if (!tagText) return
    try {
      isTagManagerLoading.value = true
      const { invoke } = await import('@tauri-apps/api/core')
      const raw = await invoke<Record<string, unknown>>('create_user_custom_tag_command', { tagText })
      newTagManagerTagText.value = ''
      applyState(normalizeState(raw))
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      isTagManagerLoading.value = false
    }
  }

  async function deleteTagManagerTag(tagText: string) {
    const normalized = tagText.trim()
    if (!normalized) return
    try {
      isTagManagerLoading.value = true
      const { invoke } = await import('@tauri-apps/api/core')
      const raw = await invoke<Record<string, unknown>>('delete_user_custom_tag_command', { tagText: normalized })
      applyState(normalizeState(raw))
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      isTagManagerLoading.value = false
    }
  }

  async function assignTagToFolder(tagText: string, folderId: number | null = activeTagManagerFolderId.value) {
    if (!folderId) return
    const normalized = tagText.trim()
    if (!normalized) return
    try {
      isTagManagerLoading.value = true
      const { invoke } = await import('@tauri-apps/api/core')
      const raw = await invoke<Record<string, unknown>>('assign_user_tag_to_folder_command', {
        folderId,
        tagText: normalized,
      })
      applyState(normalizeState(raw))
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      isTagManagerLoading.value = false
    }
  }

  async function unassignTag(tagText: string) {
    const normalized = tagText.trim()
    if (!normalized) return
    try {
      isTagManagerLoading.value = true
      const { invoke } = await import('@tauri-apps/api/core')
      const raw = await invoke<Record<string, unknown>>('unassign_user_tag_from_folder_command', {
        tagText: normalized,
      })
      applyState(normalizeState(raw))
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      isTagManagerLoading.value = false
    }
  }

  async function deleteTagManagerFolder(folderId: number) {
    try {
      isTagManagerLoading.value = true
      const { invoke } = await import('@tauri-apps/api/core')
      const raw = await invoke<Record<string, unknown>>('delete_user_tag_folder_command', {
        folderId,
      })
      applyState(normalizeState(raw))
    } catch (error) {
      options.setErrorText(options.formatError(error))
    } finally {
      isTagManagerLoading.value = false
    }
  }

  return {
    tagManagerOpen,
    isTagManagerLoading,
    tagManagerFolders,
    activeTagManagerFolderId,
    tagManagerUnclassifiedTags,
    newTagManagerFolderName,
    newTagManagerTagText,
    openTagManager,
    closeTagManager,
    reloadTagManagementState,
    createTagManagerFolder,
    createTagManagerTag,
    deleteTagManagerTag,
    assignTagToFolder,
    unassignTag,
    deleteTagManagerFolder,
  }
}
