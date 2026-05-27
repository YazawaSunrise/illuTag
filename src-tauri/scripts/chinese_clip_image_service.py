import argparse
import json
import os
import sys

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
            }
        ),
        flush=True,
    )
    sys.exit(2)


def parse_args():
    parser = argparse.ArgumentParser(description="Chinese-CLIP image encoder service")
    parser.add_argument("--model-dir", required=True)
    parser.add_argument("--provider", choices=["cpu", "cuda"], default="cpu")
    return parser.parse_args()


def pick_providers(provider: str):
    available = ort.get_available_providers()
    if provider == "cuda" and "CUDAExecutionProvider" in available:
        return ["CUDAExecutionProvider", "CPUExecutionProvider"]
    return ["CPUExecutionProvider"]


def l2_normalize(x: np.ndarray) -> np.ndarray:
    denom = np.linalg.norm(x, axis=-1, keepdims=True)
    denom = np.clip(denom, 1e-12, None)
    return x / denom


def load_image_preprocessor_config(model_dir: str):
    config_path = os.path.join(model_dir, "preprocessor_config.json")
    if not os.path.isfile(config_path):
        return {
            "size": {"height": 224, "width": 224},
            "image_mean": [0.48145466, 0.4578275, 0.40821073],
            "image_std": [0.26862954, 0.26130258, 0.27577711],
            "resample": 3,
        }
    with open(config_path, "r", encoding="utf-8") as f:
        return json.load(f)


def preprocess_image(image_path: str, pre_cfg: dict) -> np.ndarray:
    image = Image.open(image_path).convert("RGB")
    size = pre_cfg.get("size", {})
    width = int(size.get("width", 224))
    height = int(size.get("height", 224))
    resample = pre_cfg.get("resample", 3)
    resample_map = {
        0: Image.Resampling.NEAREST,
        1: Image.Resampling.LANCZOS,
        2: Image.Resampling.BILINEAR,
        3: Image.Resampling.BICUBIC,
    }
    image = image.resize((width, height), resample=resample_map.get(resample, Image.Resampling.BICUBIC))
    arr = np.asarray(image, dtype=np.float32) / 255.0
    mean = np.asarray(pre_cfg.get("image_mean", [0.48145466, 0.4578275, 0.40821073]), dtype=np.float32)
    std = np.asarray(pre_cfg.get("image_std", [0.26862954, 0.26130258, 0.27577711]), dtype=np.float32)
    arr = (arr - mean) / std
    arr = np.transpose(arr, (2, 0, 1))
    arr = np.expand_dims(arr, axis=0).astype(np.float32)
    return arr


def main():
    args = parse_args()
    model_dir = args.model_dir
    image_onnx = os.path.join(model_dir, "onnx", "chinese_clip_image_encoder.onnx")
    if not os.path.isfile(image_onnx):
        raise RuntimeError(f"Image ONNX not found: {image_onnx}")

    providers = pick_providers(args.provider)
    pre_cfg = load_image_preprocessor_config(model_dir)
    session = ort.InferenceSession(image_onnx, providers=providers)

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
            image_path = str(request.get("image_path", "")).strip()
            if not image_path:
                print(json.dumps({"error": "image_path is empty"}, ensure_ascii=False), flush=True)
                continue
            if not os.path.isfile(image_path):
                print(json.dumps({"error": f"Image not found: {image_path}"}, ensure_ascii=False), flush=True)
                continue

            pixel_values = preprocess_image(image_path, pre_cfg)
            image_features = session.run(
                None,
                {"pixel_values": pixel_values.astype(np.float32)},
            )[0].astype(np.float32)
            image_features = l2_normalize(image_features)[0]
            print(
                json.dumps(
                    {
                        "embedding": image_features.astype(np.float32).tolist(),
                        "dimension": int(image_features.shape[0]),
                    },
                    ensure_ascii=False,
                ),
                flush=True,
            )
        except Exception as error:
            print(json.dumps({"error": str(error)}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(json.dumps({"error": str(e)}, ensure_ascii=False), flush=True)
        sys.exit(1)
