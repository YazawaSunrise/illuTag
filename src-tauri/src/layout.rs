use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMeta {
    pub id: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutOptions {
    pub viewport_width: f32,
    pub column_width: f32,
    pub gap: f32,
    pub min_item_height: f32,
    pub max_item_height: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutItem {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub column_index: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutResult {
    pub items: Vec<LayoutItem>,
    pub total_height: f32,
    pub column_count: usize,
}

pub fn layout_segmented_masonry(images: &[ImageMeta], options: &LayoutOptions) -> LayoutResult {
    let column_count = column_count(options.viewport_width, options.column_width, options.gap);
    let mut items = Vec::with_capacity(images.len());
    let mut column_heights = vec![0.0_f32; column_count];

    for image in images {
        let column_index = shortest_column_index(&column_heights);
        let display_height = display_height(image, options);
        let x = column_index as f32 * (options.column_width + options.gap);
        let y = column_heights[column_index];

        items.push(LayoutItem {
            id: image.id.clone(),
            x,
            y,
            width: options.column_width,
            height: display_height,
            column_index,
        });

        column_heights[column_index] += display_height + options.gap;
    }

    LayoutResult {
        items,
        total_height: (column_heights
            .iter()
            .copied()
            .fold(0.0_f32, f32::max)
            .max(0.0)
            - options.gap)
            .max(0.0),
        column_count,
    }
}

fn column_count(viewport_width: f32, column_width: f32, gap: f32) -> usize {
    if viewport_width <= 0.0 || column_width <= 0.0 {
        return 1;
    }

    ((viewport_width + gap) / (column_width + gap))
        .floor()
        .max(1.0) as usize
}

fn display_height(image: &ImageMeta, options: &LayoutOptions) -> f32 {
    if image.width == 0 || image.height == 0 {
        return options.min_item_height.max(1.0);
    }

    let natural = image.height as f32 / image.width as f32 * options.column_width;
    natural.clamp(options.min_item_height, options.max_item_height)
}

fn shortest_column_index(column_heights: &[f32]) -> usize {
    column_heights
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> LayoutOptions {
        LayoutOptions {
            viewport_width: 650.0,
            column_width: 200.0,
            gap: 16.0,
            min_item_height: 80.0,
            max_item_height: 420.0,
        }
    }

    #[test]
    fn keeps_item_order_while_using_compact_columns() {
        let images = vec![
            image("1", 100, 200),
            image("2", 100, 100),
            image("3", 100, 150),
            image("4", 100, 100),
            image("5", 100, 200),
            image("6", 100, 100),
            image("7", 100, 100),
        ];

        let result = layout_segmented_masonry(&images, &options());

        assert_eq!(result.column_count, 3);
        assert_eq!(result.items[0].id, "1");
        assert_eq!(result.items[5].id, "6");
        assert_eq!(result.items[6].id, "7");
        assert_eq!(result.items[6].column_index, 1);
        assert_eq!(result.items[6].y, 432.0);
    }

    #[test]
    fn clamps_extreme_item_heights() {
        let images = vec![image("long", 100, 1000), image("wide", 1000, 100)];

        let result = layout_segmented_masonry(&images, &options());

        assert_eq!(result.items[0].height, 420.0);
        assert_eq!(result.items[1].height, 80.0);
    }

    #[test]
    fn places_next_item_in_shortest_column() {
        let images = vec![
            image("1", 100, 200),
            image("2", 100, 100),
            image("3", 100, 100),
            image("4", 100, 100),
        ];

        let result = layout_segmented_masonry(&images, &options());

        assert_eq!(result.items[3].column_index, 1);
        assert_eq!(result.items[3].y, 216.0);
    }

    fn image(id: &str, width: u32, height: u32) -> ImageMeta {
        ImageMeta {
            id: id.to_string(),
            width,
            height,
        }
    }
}
