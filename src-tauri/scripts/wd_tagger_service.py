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
        json.dumps(
            {
                "error": f"Missing Python module: {missing}. Install with: pip install onnxruntime numpy pillow"
            },
            ensure_ascii=False,
        ),
        flush=True,
    )
    sys.exit(2)


def parse_args():
    parser = argparse.ArgumentParser(description="WD SwinV2 tagger service")
    parser.add_argument("--model", required=True, help="model.onnx path")
    parser.add_argument("--tags", required=True, help="selected_tags.csv path")
    parser.add_argument("--provider", choices=["cpu", "cuda"], default="cpu")
    return parser.parse_args()


def pick_providers(provider: str):
    available = ort.get_available_providers()
    if provider == "cuda" and "CUDAExecutionProvider" in available:
        return ["CUDAExecutionProvider", "CPUExecutionProvider"]
    return ["CPUExecutionProvider"]


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
    if not os.path.isfile(args.model):
        raise RuntimeError(f"Model not found: {args.model}")
    if not os.path.isfile(args.tags):
        raise RuntimeError(f"selected_tags.csv not found: {args.tags}")

    providers = pick_providers(args.provider)
    tags = load_tags(args.tags)
    session = ort.InferenceSession(args.model, providers=providers)
    input_info = session.get_inputs()[0]
    output_info = session.get_outputs()[0]
    input_name = input_info.name
    output_name = output_info.name
    shape = input_info.shape
    if len(shape) != 4:
        raise RuntimeError(f"Unexpected input shape: {shape}")
    input_height = int(shape[1]) if isinstance(shape[1], int) else 448
    input_width = int(shape[2]) if isinstance(shape[2], int) else 448

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        total_start = time.perf_counter()
        image_id = ""
        image_path = ""
        preprocess_ms = 0.0
        inference_ms = 0.0
        postprocess_ms = 0.0
        total_ms = 0.0
        try:
            request = json.loads(line)
            image_id = str(request.get("image_id", "")).strip()
            image_path = str(request.get("image_path", "")).strip()
            general_threshold = float(request.get("general_threshold", 0.35))
            character_threshold = float(request.get("character_threshold", 0.85))

            if not image_path:
                raise RuntimeError("image_path is empty")
            if not os.path.isfile(image_path):
                raise RuntimeError(f"Image not found: {image_path}")

            p0 = time.perf_counter()
            tensor = preprocess(image_path, input_height, input_width)
            preprocess_ms = (time.perf_counter() - p0) * 1000.0

            i0 = time.perf_counter()
            outputs = session.run([output_name], {input_name: tensor})
            inference_ms = (time.perf_counter() - i0) * 1000.0

            s0 = time.perf_counter()
            probs = outputs[0][0].astype(np.float32).tolist()
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
                elif category == 0 and score >= general_threshold:
                    general_tags.append(item)
                elif category == 4 and score >= character_threshold:
                    character_tags.append(item)

            ratings.sort(key=lambda x: x["score"], reverse=True)
            general_tags.sort(key=lambda x: x["score"], reverse=True)
            character_tags.sort(key=lambda x: x["score"], reverse=True)
            postprocess_ms = (time.perf_counter() - s0) * 1000.0
            total_ms = (time.perf_counter() - total_start) * 1000.0

            print(
                f"[wd-service] image_id={image_id or image_path} preprocess_ms={preprocess_ms:.2f} "
                f"inference_ms={inference_ms:.2f} postprocess_ms={postprocess_ms:.2f} total_ms={total_ms:.2f}",
                file=sys.stderr,
                flush=True,
            )

            print(
                json.dumps(
                    {
                        "imageId": image_id,
                        "ratings": ratings,
                        "generalTags": general_tags,
                        "characterTags": character_tags,
                        "generalThreshold": general_threshold,
                        "characterThreshold": character_threshold,
                        "elapsedMs": inference_ms,
                        "preprocessMs": preprocess_ms,
                        "inferenceMs": inference_ms,
                        "postprocessMs": postprocess_ms,
                        "totalMs": total_ms,
                    },
                    ensure_ascii=False,
                ),
                flush=True,
            )
        except Exception as error:
            total_ms = (time.perf_counter() - total_start) * 1000.0
            print(
                f"[wd-service] image_id={image_id or image_path or 'unknown'} error={error} total_ms={total_ms:.2f}",
                file=sys.stderr,
                flush=True,
            )
            print(
                json.dumps(
                    {"error": str(error), "imageId": image_id, "totalMs": total_ms},
                    ensure_ascii=False,
                ),
                flush=True,
            )


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False), flush=True)
        sys.exit(1)
