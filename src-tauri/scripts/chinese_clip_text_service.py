import argparse
import json
import os
import sys
import unicodedata

try:
    import numpy as np
    import onnxruntime as ort
except ModuleNotFoundError as e:
    missing = getattr(e, "name", "unknown")
    print(json.dumps({"error": f"Missing Python module: {missing}. Install with: pip install onnxruntime numpy"}), flush=True)
    sys.exit(2)


def parse_args():
    parser = argparse.ArgumentParser(description="Chinese-CLIP text encoder service")
    parser.add_argument("--model-dir", required=True)
    parser.add_argument("--provider", choices=["cpu", "cuda"], default="cpu")
    parser.add_argument("--max-text-length", type=int, default=52)
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


def wordpiece_tokenize(token: str, vocab: dict, unk_token="[UNK]", max_input_chars_per_word: int = 100):
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


def basic_tokenize(text: str, do_lower_case=True):
    text = clean_text(text)
    text = tokenize_chinese_chars(text)
    orig_tokens = whitespace_tokenize(text)
    split_tokens = []
    for token in orig_tokens:
        if do_lower_case:
            token = strip_accents(token.lower())
        split_tokens.extend(split_on_punc(token))
    return whitespace_tokenize(" ".join(split_tokens))


def encode_text(text: str, vocab: dict, max_length: int):
    cls_token = "[CLS]"
    sep_token = "[SEP]"
    pad_token = "[PAD]"
    unk_token = "[UNK]"
    required = [cls_token, sep_token, pad_token, unk_token]
    if any(token not in vocab for token in required):
        raise RuntimeError("vocab.txt missing required special tokens [CLS]/[SEP]/[PAD]/[UNK]")

    cls_id = vocab[cls_token]
    sep_id = vocab[sep_token]
    pad_id = vocab[pad_token]
    unk_id = vocab[unk_token]
    input_ids = np.full((1, max_length), pad_id, dtype=np.int64)
    attention_mask = np.zeros((1, max_length), dtype=np.int64)

    tokens = []
    for token in basic_tokenize(text, do_lower_case=True):
        tokens.extend(wordpiece_tokenize(token, vocab, unk_token=unk_token))
    tokens = tokens[: max(0, max_length - 2)]

    token_ids = [cls_id]
    token_ids.extend(vocab.get(token, unk_id) for token in tokens)
    token_ids.append(sep_id)
    seq_len = min(len(token_ids), max_length)
    input_ids[0, :seq_len] = np.asarray(token_ids[:seq_len], dtype=np.int64)
    attention_mask[0, :seq_len] = 1
    return input_ids, attention_mask


def main():
    args = parse_args()
    model_dir = args.model_dir
    text_onnx = os.path.join(model_dir, "onnx", "chinese_clip_text_encoder.onnx")
    vocab_path = os.path.join(model_dir, "vocab.txt")
    if not os.path.isfile(text_onnx):
        raise RuntimeError(f"Text ONNX not found: {text_onnx}")
    if not os.path.isfile(vocab_path):
        raise RuntimeError(f"vocab.txt not found: {vocab_path}")

    vocab = load_vocab(vocab_path)
    providers = pick_providers(args.provider)
    session = ort.InferenceSession(text_onnx, providers=providers)

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
            text = str(request.get("text", "")).strip()
            if not text:
                print(json.dumps({"error": "text is empty"}, ensure_ascii=False), flush=True)
                continue
            input_ids, attention_mask = encode_text(text, vocab, max(4, int(args.max_text_length)))
            text_features = session.run(
                None,
                {
                    "input_ids": input_ids.astype(np.int64),
                    "attention_mask": attention_mask.astype(np.int64),
                },
            )[0].astype(np.float32)
            text_features = l2_normalize(text_features)[0]
            print(
                json.dumps(
                    {
                        "embedding": text_features.astype(np.float32).tolist(),
                        "dimension": int(text_features.shape[0]),
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
