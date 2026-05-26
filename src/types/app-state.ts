import type { GalleryImage } from './gallery'

export type LibraryFolder = {
  id: number
  path: string
  addedAt: number
  lastScannedAt?: number | null
}

export type UserFolder = {
  id: number
  parentId?: number | null
  name: string
  sortOrder: number
  createdAt: number
  updatedAt: number
}

export type ImageFolderAssignment = {
  imageId: string
  folderId: number
}

export type ReferenceBoardFolder = {
  id: number
  name: string
  sortOrder: number
  createdAt: number
  updatedAt: number
}

export type ReferenceBoard = {
  id: number
  folderId?: number | null
  name: string
  sortOrder: number
  createdAt: number
  updatedAt: number
}

export type ReferenceBoardItem = {
  id: number
  boardId: number
  imageId: string
  x: number
  y: number
  width: number
  height: number
  rotation: number
  zIndex: number
  createdAt: number
}

export type LibraryStore = {
  folders: LibraryFolder[]
  images: GalleryImage[]
  userFolders: UserFolder[]
  imageFolders: ImageFolderAssignment[]
  referenceBoardFolders: ReferenceBoardFolder[]
  referenceBoards: ReferenceBoard[]
  referenceBoardItems: ReferenceBoardItem[]
}

export type ViewMode = 'gallery' | 'settings' | 'board'

export type BoardWorldBounds = {
  minX: number
  minY: number
  maxX: number
  maxY: number
}
