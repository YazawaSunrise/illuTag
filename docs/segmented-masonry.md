# Continuous Masonry Layout

This gallery mode keeps browsing compact by always appending the next image to the current
shortest column.

The input image list should already be sorted by the active gallery order, usually `imported_at desc`.

1. Keep one running height for each visible column.
2. For each image in sorted order, place it into the currently shortest column.
3. Increase that column height by the rendered image height plus the gap.

This keeps the masonry surface continuous. Images stay in the source order for placement decisions,
but later images can appear higher than earlier images when they fit into a shorter column.

The layout result is derived from image dimensions and viewport settings. It should be cached by query, sort, viewport width, column width, gap, and height clamp settings, but it should not be stored as permanent image metadata.
