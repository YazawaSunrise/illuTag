import argparse
import json
import os
import sys
import time
import unicodedata

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
    parser = argparse.ArgumentParser(description="Chinese-CLIP ONNX smoke test")
    parser.add_argument("--model-dir", required=True, help="Model directory containing vocab/config files")
    parser.add_argument("--text-onnx", default="", help="Text encoder ONNX path")
    parser.add_argument("--image-onnx", default="", help="Image encoder ONNX path")
    parser.add_argument("--image", required=True, help="Input image path")
    parser.add_argument("--text", action="append", required=True, help="Candidate text. Pass multiple --text entries")
    parser.add_argument("--top-k", type=int, default=5, help="Top-k results to print")
    parser.add_argument("--provider", choices=["cpu", "cuda"], default="cpu", help="Preferred execution provider")
    parser.add_argument("--max-text-length", type=int, default=52, help="Maximum token length")
    parser.add_argument(
        "--softmax-temperature",
        type=float,
        default=12.0,
        help="Temperature used to convert cosine scores into relative softmax percentages",
    )
    return parser.parse_args()


def l2_normalize(x: np.ndarray) -> np.ndarray:
    denom = np.linalg.norm(x, axis=-1, keepdims=True)
    denom = np.clip(denom, 1e-12, None)
    return x / denom


def to_softmax(values: np.ndarray) -> np.ndarray:
    shifted = values - np.max(values)
    exp_values = np.exp(shifted)
    denom = np.sum(exp_values)
    if denom <= 0:
        return np.full_like(values, fill_value=1.0 / max(1, values.size))
    return exp_values / denom


def resolve_onnx_paths(model_dir: str, text_onnx: str, image_onnx: str):
    if text_onnx:
        text_path = text_onnx
    else:
        text_path = os.path.join(model_dir, "onnx", "chinese_clip_text_encoder.onnx")

    if image_onnx:
        image_path = image_onnx
    else:
        image_path = os.path.join(model_dir, "onnx", "chinese_clip_image_encoder.onnx")

    return text_path, image_path


def pick_providers(provider: str):
    available = ort.get_available_providers()
    if provider == "cuda" and "CUDAExecutionProvider" in available:
        return ["CUDAExecutionProvider", "CPUExecutionProvider"]
    return ["CPUExecutionProvider"]


def load_vocab(vocab_path: str):
    vocab = {}
    with open(vocab_path, "r", encoding="utf-8") as f:
        for idx, line in enumerate(f):
            token = line.rstrip("\n")
            if token:
                vocab[token] = idx
    if not vocab:
        raise RuntimeError("Failed to load vocab.txt")
    return vocab


def is_whitespace(ch: str) -> bool:
    if ch in (" ", "\t", "\n", "\r"):
        return True
    return unicodedata.category(ch) == "Zs"


def is_control(ch: str) -> bool:
    if ch in ("\t", "\n", "\r"):
        return False
    return unicodedata.category(ch).startswith("C")


def is_punctuation(ch: str) -> bool:
    cp = ord(ch)
    if (33 <= cp <= 47) or (58 <= cp <= 64) or (91 <= cp <= 96) or (123 <= cp <= 126):
        return True
    return unicodedata.category(ch).startswith("P")


def is_chinese_char(cp: int) -> bool:
    return (
        (0x4E00 <= cp <= 0x9FFF)
        or (0x3400 <= cp <= 0x4DBF)
        or (0x20000 <= cp <= 0x2A6DF)
        or (0x2A700 <= cp <= 0x2B73F)
        or (0x2B740 <= cp <= 0x2B81F)
        or (0x2B820 <= cp <= 0x2CEAF)
        or (0xF900 <= cp <= 0xFAFF)
        or (0x2F800 <= cp <= 0x2FA1F)
    )


def clean_text(text: str) -> str:
    output = []
    for ch in text:
        cp = ord(ch)
        if cp == 0 or cp == 0xFFFD or is_control(ch):
            continue
        if is_whitespace(ch):
            output.append(" ")
        else:
            output.append(ch)
    return "".join(output)


def tokenize_chinese_chars(text: str) -> str:
    output = []
    for ch in text:
        cp = ord(ch)
        if is_chinese_char(cp):
            output.extend([" ", ch, " "])
        else:
            output.append(ch)
    return "".join(output)


def strip_accents(text: str) -> str:
    output = []
    for ch in unicodedata.normalize("NFD", text):
        if unicodedata.category(ch) == "Mn":
            continue
        output.append(ch)
    return "".join(output)


def split_on_punc(text: str):
    if not text:
        return []
    output = []
    current = []
    for ch in text:
        if is_punctuation(ch):
            if current:
                output.append("".join(current))
                current = []
            output.append(ch)
        else:
            current.append(ch)
    if current:
        output.append("".join(current))
    return output


def whitespace_tokenize(text: str):
    text = text.strip()
    if not text:
        return []
    return text.split()


def wordpiece_tokenize(token: str, vocab: dict, unk_token: str = "[UNK]", max_input_chars_per_word: int = 100):
    if len(token) > max_input_chars_per_word:
        return [unk_token]

    sub_tokens = []
    start = 0
    while start < len(token):
        end = len(token)
        cur_substr = None
        while start < end:
            substr = token[start:end]
            if start > 0:
                substr = "##" + substr
            if substr in vocab:
                cur_substr = substr
                break
            end -= 1
        if cur_substr is None:
            return [unk_token]
        sub_tokens.append(cur_substr)
        start = end
    return sub_tokens


def basic_tokenize(text: str, do_lower_case: bool = True):
    text = clean_text(text)
    text = tokenize_chinese_chars(text)
    orig_tokens = whitespace_tokenize(text)
    split_tokens = []
    for token in orig_tokens:
        if do_lower_case:
            token = strip_accents(token.lower())
        split_tokens.extend(split_on_punc(token))
    return whitespace_tokenize(" ".join(split_tokens))


def encode_texts(texts, vocab: dict, max_length: int):
    cls_token = "[CLS]"
    sep_token = "[SEP]"
    pad_token = "[PAD]"
    unk_token = "[UNK]"

    if cls_token not in vocab or sep_token not in vocab or pad_token not in vocab or unk_token not in vocab:
        raise RuntimeError("vocab.txt missing required special tokens [CLS]/[SEP]/[PAD]/[UNK]")

    cls_id = vocab[cls_token]
    sep_id = vocab[sep_token]
    pad_id = vocab[pad_token]
    unk_id = vocab[unk_token]

    input_ids = np.full((len(texts), max_length), pad_id, dtype=np.int64)
    attention_mask = np.zeros((len(texts), max_length), dtype=np.int64)

    for row_idx, text in enumerate(texts):
        tokens = []
        for token in basic_tokenize(text, do_lower_case=True):
            tokens.extend(wordpiece_tokenize(token, vocab, unk_token=unk_token))

        tokens = tokens[: max(0, max_length - 2)]
        token_ids = [cls_id]
        token_ids.extend(vocab.get(token, unk_id) for token in tokens)
        token_ids.append(sep_id)

        seq_len = min(len(token_ids), max_length)
        input_ids[row_idx, :seq_len] = np.asarray(token_ids[:seq_len], dtype=np.int64)
        attention_mask[row_idx, :seq_len] = 1

    return input_ids, attention_mask


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

    if not os.path.isdir(args.model_dir):
        raise RuntimeError(f"Model directory not found: {args.model_dir}")
    if not os.path.isfile(args.image):
        raise RuntimeError(f"Image not found: {args.image}")

    text_onnx, image_onnx = resolve_onnx_paths(args.model_dir, args.text_onnx, args.image_onnx)
    if not os.path.isfile(text_onnx):
        raise RuntimeError(f"Text ONNX not found: {text_onnx}")
    if not os.path.isfile(image_onnx):
        raise RuntimeError(f"Image ONNX not found: {image_onnx}")

    vocab_path = os.path.join(args.model_dir, "vocab.txt")
    if not os.path.isfile(vocab_path):
        raise RuntimeError(f"vocab.txt not found: {vocab_path}")

    vocab = load_vocab(vocab_path)
    input_ids, attention_mask = encode_texts(args.text, vocab, max_length=max(4, int(args.max_text_length)))
    pixel_values = preprocess_image(args.image, load_image_preprocessor_config(args.model_dir))

    providers = pick_providers(args.provider)
    text_session = ort.InferenceSession(text_onnx, providers=providers)
    image_session = ort.InferenceSession(image_onnx, providers=providers)

    t0 = time.perf_counter()
    text_features = text_session.run(
        None,
        {
            "input_ids": input_ids.astype(np.int64),
            "attention_mask": attention_mask.astype(np.int64),
        },
    )[0].astype(np.float32)
    t1 = time.perf_counter()

    image_features = image_session.run(
        None,
        {
            "pixel_values": pixel_values.astype(np.float32),
        },
    )[0].astype(np.float32)
    t2 = time.perf_counter()

    text_features = l2_normalize(text_features)
    image_features = l2_normalize(image_features)
    scores = (image_features @ text_features.T)[0]
    cosine_percent = np.clip((scores + 1.0) * 50.0, 0.0, 100.0)

    score_min = float(np.min(scores))
    score_max = float(np.max(scores))
    score_span = score_max - score_min
    if score_span <= 1e-12:
        relative_percent = np.full_like(scores, 100.0, dtype=np.float32)
    else:
        relative_percent = ((scores - score_min) / score_span) * 100.0

    temperature = max(1e-6, float(args.softmax_temperature))
    softmax_percent = to_softmax(scores * temperature) * 100.0

    ranked_indices = np.argsort(-scores)
    top_k = max(1, min(args.top_k, len(args.text)))
    top_results = []
    for rank, idx in enumerate(ranked_indices[:top_k], start=1):
        index = int(idx)
        top_results.append(
            {
                "rank": rank,
                "text": args.text[index],
                "score": float(scores[index]),
                "scoreCosine": float(scores[index]),
                "scoreCosinePercent": float(cosine_percent[index]),
                "scoreRelativePercent": float(relative_percent[index]),
                "scoreSoftmaxPercent": float(softmax_percent[index]),
            }
        )

    all_results = []
    for idx, text in enumerate(args.text):
        all_results.append(
            {
                "text": text,
                "scoreCosine": float(scores[idx]),
                "scoreCosinePercent": float(cosine_percent[idx]),
                "scoreRelativePercent": float(relative_percent[idx]),
                "scoreSoftmaxPercent": float(softmax_percent[idx]),
            }
        )

    result = {
        "image": args.image,
        "modelDir": args.model_dir,
        "textOnnx": text_onnx,
        "imageOnnx": image_onnx,
        "providers": providers,
        "textCount": len(args.text),
        "featureShapes": {
            "text": list(text_features.shape),
            "image": list(image_features.shape),
        },
        "elapsedMs": {
            "textEncode": float((t1 - t0) * 1000.0),
            "imageEncode": float((t2 - t1) * 1000.0),
            "total": float((t2 - t0) * 1000.0),
        },
        "scoreInfo": {
            "softmaxTemperature": temperature,
            "cosineRangeMin": score_min,
            "cosineRangeMax": score_max,
        },
        "allResults": all_results,
        "topResults": top_results,
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(str(e), file=sys.stderr)
        sys.exit(1)
