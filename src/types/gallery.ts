export type GalleryImage = {
  id: string
  path: string
  thumbnailPath?: string | null
  fileName: string
  ext: string
  width: number
  height: number
  fileSize: number
  modifiedAt: number
  importedAt: number
  folderId: number
  missing: boolean
  trashed: boolean
  isFavorite: boolean
  source: string
}

export type GalleryLayoutItem = {
  id: string
  thumbnailUrl: string
  x: number
  y: number
  width: number
  height: number
  columnIndex: number
}

export type GalleryLayoutResult = {
  items: GalleryLayoutItem[]
  totalHeight: number
  columnCount: number
}
