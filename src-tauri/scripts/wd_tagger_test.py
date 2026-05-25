import argparse
import csv
import json
import os
import sys
import time

try:
    import numpy as np
    import onnxruntime as ort
    from PIL import Image
except ModuleNotFoundError as e:
    missing = getattr(e, "name", "unknown")
    print(
        f"Missing Python module: {missing}. Install with: pip install onnxruntime numpy pillow",
        file=sys.stderr,
    )
    sys.exit(2)


def parse_args():
    parser = argparse.ArgumentParser(description="WD SwinV2 tagger single-image test")
    parser.add_argument("--image", required=True, help="Source image path")
    parser.add_argument("--model", required=True, help="model.onnx path")
    parser.add_argument("--tags", required=True, help="selected_tags.csv path")
    parser.add_argument("--general-threshold", type=float, default=0.35)
    parser.add_argument("--character-threshold", type=float, default=0.85)
    parser.add_argument("--image-id", default="")
    return parser.parse_args()


def load_tags(csv_path):
    tags = []
    with open(csv_path, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for row in reader:
            name = row.get("name", "").strip()
            category_raw = row.get("category", "0").strip()
            if not name:
                continue
            try:
                category = int(category_raw)
            except ValueError:
                category = 0
            tags.append((name, category))
    if not tags:
        raise RuntimeError("No tags loaded from selected_tags.csv")
    return tags


def to_rgb_with_white_bg(image):
    rgba = image.convert("RGBA")
    base = Image.new("RGBA", rgba.size, (255, 255, 255, 255))
    base.alpha_composite(rgba)
    return base.convert("RGB")


def pad_to_square(image_rgb):
    w, h = image_rgb.size
    side = max(w, h)
    canvas = Image.new("RGB", (side, side), (255, 255, 255))
    canvas.paste(image_rgb, ((side - w) // 2, (side - h) // 2))
    return canvas


def preprocess(image_path, input_height, input_width):
    image = Image.open(image_path)
    image = to_rgb_with_white_bg(image)
    image = pad_to_square(image)
    image = image.resize((input_width, input_height), Image.Resampling.BICUBIC)
    arr = np.asarray(image, dtype=np.float32)
    arr = arr[:, :, ::-1]
    arr = np.expand_dims(arr, axis=0)
    return arr


def main():
    args = parse_args()
    if not os.path.isfile(args.image):
        raise RuntimeError(f"Image not found: {args.image}")
    if not os.path.isfile(args.model):
        raise RuntimeError(f"Model not found: {args.model}")
    if not os.path.isfile(args.tags):
        raise RuntimeError(f"selected_tags.csv not found: {args.tags}")

    session = ort.InferenceSession(args.model, providers=["CPUExecutionProvider"])
    input_info = session.get_inputs()[0]
    output_info = session.get_outputs()[0]
    input_name = input_info.name
    output_name = output_info.name

    shape = input_info.shape
    if len(shape) != 4:
        raise RuntimeError(f"Unexpected input shape: {shape}")

    input_height = shape[1] if isinstance(shape[1], int) else 448
    input_width = shape[2] if isinstance(shape[2], int) else 448

    tensor = preprocess(args.image, int(input_height), int(input_width))
    start = time.perf_counter()
    outputs = session.run([output_name], {input_name: tensor})
    elapsed_ms = (time.perf_counter() - start) * 1000.0

    probs = outputs[0][0].astype(np.float32).tolist()
    tags = load_tags(args.tags)
    if len(probs) != len(tags):
        raise RuntimeError(
            f"Output length mismatch: got {len(probs)} probs, expected {len(tags)} tags"
        )

    ratings = []
    general_tags = []
    character_tags = []

    for score, (name, category) in zip(probs, tags):
        item = {"tag": name, "score": float(score)}
        if category == 9:
            ratings.append(item)
        elif category == 0 and score >= args.general_threshold:
            general_tags.append(item)
        elif category == 4 and score >= args.character_threshold:
            character_tags.append(item)

    ratings.sort(key=lambda x: x["score"], reverse=True)
    general_tags.sort(key=lambda x: x["score"], reverse=True)
    character_tags.sort(key=lambda x: x["score"], reverse=True)

    result = {
        "imageId": args.image_id,
        "ratings": ratings,
        "generalTags": general_tags,
        "characterTags": character_tags,
        "generalThreshold": float(args.general_threshold),
        "characterThreshold": float(args.character_threshold),
        "elapsedMs": float(elapsed_ms),
    }
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(str(e), file=sys.stderr)
        sys.exit(1)
