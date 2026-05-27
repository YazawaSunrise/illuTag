import argparse
from pathlib import Path

import onnx
import torch
from transformers import ChineseCLIPModel


class ChineseClipTextEncoder(torch.nn.Module):
    def __init__(self, model: ChineseCLIPModel):
        super().__init__()
        self.model = model

    def forward(self, input_ids: torch.Tensor, attention_mask: torch.Tensor):
        outputs = self.model.get_text_features(input_ids=input_ids, attention_mask=attention_mask, return_dict=True)
        return outputs.pooler_output


class ChineseClipImageEncoder(torch.nn.Module):
    def __init__(self, model: ChineseCLIPModel):
        super().__init__()
        self.model = model

    def forward(self, pixel_values: torch.Tensor):
        outputs = self.model.get_image_features(pixel_values=pixel_values, return_dict=True)
        return outputs.pooler_output


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Export Chinese-CLIP model to ONNX")
    parser.add_argument("--model-dir", required=True, help="Local model directory")
    parser.add_argument("--output-dir", required=True, help="Output directory for ONNX files")
    parser.add_argument("--opset", type=int, default=17, help="ONNX opset version")
    parser.add_argument("--seq-len", type=int, default=52, help="Dummy text sequence length")
    parser.add_argument("--image-size", type=int, default=224, help="Dummy image size")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    model_dir = Path(args.model_dir).resolve()
    output_dir = Path(args.output_dir).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    if not model_dir.exists():
        raise FileNotFoundError(f"Model directory not found: {model_dir}")

    print(f"[1/4] Loading model from: {model_dir}")
    model = ChineseCLIPModel.from_pretrained(str(model_dir), local_files_only=True)
    model.eval()

    text_encoder = ChineseClipTextEncoder(model).eval()
    image_encoder = ChineseClipImageEncoder(model).eval()

    text_onnx_path = output_dir / "chinese_clip_text_encoder.onnx"
    image_onnx_path = output_dir / "chinese_clip_image_encoder.onnx"

    input_ids = torch.ones((1, args.seq_len), dtype=torch.long)
    attention_mask = torch.ones((1, args.seq_len), dtype=torch.long)
    pixel_values = torch.randn((1, 3, args.image_size, args.image_size), dtype=torch.float32)

    print(f"[2/4] Exporting text encoder -> {text_onnx_path}")
    torch.onnx.export(
        text_encoder,
        (input_ids, attention_mask),
        str(text_onnx_path),
        input_names=["input_ids", "attention_mask"],
        output_names=["text_features"],
        dynamic_axes={
            "input_ids": {0: "batch", 1: "sequence"},
            "attention_mask": {0: "batch", 1: "sequence"},
            "text_features": {0: "batch"},
        },
        opset_version=args.opset,
        do_constant_folding=True,
        export_params=True,
        dynamo=False,
    )

    print(f"[3/4] Exporting image encoder -> {image_onnx_path}")
    torch.onnx.export(
        image_encoder,
        (pixel_values,),
        str(image_onnx_path),
        input_names=["pixel_values"],
        output_names=["image_features"],
        dynamic_axes={
            "pixel_values": {0: "batch"},
            "image_features": {0: "batch"},
        },
        opset_version=args.opset,
        do_constant_folding=True,
        export_params=True,
        dynamo=False,
    )

    print("[4/4] Validating ONNX files")
    onnx.checker.check_model(str(text_onnx_path))
    onnx.checker.check_model(str(image_onnx_path))

    print("Done.")
    print(f"Text ONNX : {text_onnx_path}")
    print(f"Image ONNX: {image_onnx_path}")


if __name__ == "__main__":
    main()
