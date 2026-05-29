from __future__ import annotations

import argparse
import csv
import json
import os
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any


HAS_HAN = re.compile(r"[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]")
HAS_KANA = re.compile(r"[\u3040-\u30ff\uff66-\uff9f]")
HAS_HANGUL = re.compile(r"[\uac00-\ud7af]")
JP_ONLY_HINTS = set("兎辻込働峠畑姫凧匂躾込栃榊雫咲")
ZH_HINTS = set(
    "兔条纹饰装换衫裙裤袜发脸胸腿脚眼嘴唇牙耳尾角翼毛皮肤"
    "黑白红蓝绿黄紫粉橙棕灰金银色长短大小高低上下左右前后"
    "官方服衣帽带领结蝴蝶结泳装制服水手校服围巾手套靴鞋"
    "透明湿裸全半单双多女性男性少女少年萝莉正太动物猫狗狐"
)
SIMPLIFIED_HINTS = set(
    "\u4e2a\u4e3a\u4e49\u4e4c\u4e50\u4e60\u4e61\u4e66\u4e70\u4e89\u4e8e\u4e9a"
    "\u4ea7\u4eb2\u4ebf\u4ec5\u4ece\u4ed1\u4eec\u4f18\u4f1a\u4f1e\u4f20\u4f24"
    "\u4f53\u4f59\u4f5b\u4f5c\u4f63\u4fa7\u4fa6\u4fe1\u4fe9\u4fe6\u4fee"
    "\u503a\u503e\u507f\u513f\u514b\u5151\u5170\u5173\u5174\u517b\u517d"
    "\u5185\u519b\u519c\u51b2\u51b3\u51b5\u51c0\u51c6\u51e0\u51fb\u5212"
    "\u5218\u5219\u521a\u521b\u5220\u522b\u5236\u5239\u5242\u5251\u5267"
    "\u529d\u529e\u52a1\u52a8\u52b1\u52b2\u52b3\u52bf\u52cb\u5300\u533a"
    "\u534e\u534f\u5355\u5362\u536b\u5385\u5386\u5389\u538b\u53bf\u53c2"
    "\u53cc\u53d1\u53d8\u53e0\u53f6\u53f7\u53f9\u540e\u5411\u5428\u542f"
    "\u5458\u547c\u54cd\u54d1\u54d2\u56a3\u56e2\u56ed\u56f4\u56fd\u56fe"
    "\u5706\u5723\u573a\u5757\u575a\u575b\u575d\u575e\u575f\u57ab\u57ce"
    "\u5904\u5907\u590d\u5934\u5938\u5939\u5956\u5986\u5987\u5988\u5b59"
    "\u5b66\u5b9d\u5b9e\u5ba0\u5ba1\u5baa\u5bab\u5bbd\u5bbe\u5bc6\u5bf9"
    "\u5bfc\u5c06\u5c14\u5c18\u5c1d\u5c27\u5c38\u5c3d\u5c42\u5c49\u5c4a"
    "\u5c5e\u5c61\u5c66\u5c7f\u5c81\u5c82\u5c96\u5c97\u5c9b\u5cad\u5cb3"
    "\u5de9\u5def\u5e01\u5e05\u5e08\u5e10\u5e18\u5e1c\u5e26\u5e2e\u5e73"
    "\u5e7f\u5e84\u5e86\u5e90\u5e93\u5e94\u5e99\u5e9f\u5f00\u5f02\u5f03"
    "\u5f20\u5f25\u5f2f\u5f39\u5f3a\u5f52\u5f53\u5f55\u5f7b\u5f84\u5fc6"
    "\u5fe7\u5ffd\u6001\u6000\u6002\u603b\u604b\u6076\u607c\u60a6\u60ac"
    "\u60ca\u60e8\u60e9\u60eb\u60ed\u60ef\u6124\u613f\u6151\u61d2\u620f"
    "\u6218\u6237\u624e\u6267\u6269\u626b\u626c\u6270\u629a\u629b\u629f"
    "\u62a2\u62a4\u62a5\u62c5\u62df\u62e2\u62e3\u62e5\u62e6\u62e8\u62e9"
    "\u6302\u631a\u631b\u631c\u6325\u6326\u632f\u633a\u633d\u6362\u636e"
    "\u637b\u6380\u6388\u6389\u638c\u6392\u63a0\u63a2\u63a5\u63a7\u63a8"
    "\u63ba\u63fd\u6400\u6401\u6402\u6405\u641c\u6444\u6446\u6447\u6448"
    "\u6478\u64a4\u64ad\u64b5\u64cd\u64de\u64e6\u6536\u6539\u653b\u653e"
    "\u6548\u654c\u6551\u6559\u655b\u6570\u6587\u65ad\u65e0\u65e7\u65f6"
    "\u65f7\u660e\u6613\u661f\u6625\u663e\u6653\u6682\u66a7\u672f\u673a"
    "\u6740\u6742\u6743\u6761\u6765\u6781\u6784\u679e\u67a2\u67aa\u67ab"
    "\u67dc\u6807\u6808\u680b\u680f\u6811\u6837\u6838\u6839\u683c\u6843"
    "\u6863\u6865\u6868\u68c0\u697c\u6982\u69db\u6a2a\u6a31\u6a71\u6b22"
    "\u6b27\u6b7c\u6b8b\u6bb4\u6bc1\u6bd5\u6bd9\u6c14\u6c22\u6c47\u6c49"
    "\u6c64\u6c9f\u6ca1\u6ca3\u6cb3\u6cb9\u6cbb\u6cd5\u6ce8\u6d01\u6d12"
    "\u6d4b\u6d4e\u6d4f\u6d53\u6d77\u6d82\u6d88\u6d89\u6d9b\u6d9d\u6df1"
    "\u6e05\u6e10\u6e14\u6e29\u6e38\u6e7e\u6e7f\u6eda\u6ee1\u6ee5\u6ee8"
    "\u6ee9\u6f47\u6f5c\u6f6e\u706d\u706f\u7070\u7075\u707e\u7089\u70b9"
    "\u70bc\u70ed\u7115\u7231\u7237\u7247\u724c\u7275\u7279\u72b6\u72ec"
    "\u72ed\u72ee\u730e\u732a\u732e\u736d\u739b\u73af\u73b0\u73ba\u7535"
    "\u753b\u7545\u754c\u7559\u7565\u7597\u75af\u75d2\u75db\u767b\u767d"
    "\u767e\u7684\u76d1\u76d6\u76d8\u770b\u771f\u7740\u77eb\u77f6\u77ff"
    "\u7801\u7816\u7834\u7855\u786e\u7891\u78b0\u793c\u793e\u795e\u7968"
    "\u79bb\u79cd\u79f0\u79ef\u7a0b\u7a33\u7a7a\u7a9d\u7a77\u7a83\u7a91"
    "\u7a9c\u7a9d\u7ad6\u7ade\u7b14\u7b49\u7b5b\u7b56\u7b80\u7ba1\u7c7b"
    "\u7c89\u7cbe\u7ea0\u7ea2\u7ea4\u7ea6\u7ea7\u7eaa\u7eb1\u7eb2\u7eb3"
    "\u7eb5\u7eb6\u7eb7\u7eb8\u7eb9\u7eba\u7ebd\u7ebf\u7ec3\u7ec4\u7ec6"
    "\u7ec8\u7ecd\u7ecf\u7ed1\u7ed3\u7ed5\u7ed8\u7ed9\u7edc\u7edd\u7edf"
    "\u7ee2\u7ee3\u7ee7\u7eed\u7ef3\u7ef4\u7ef5\u7eff\u7f16\u7f18\u7f29"
    "\u7f51\u7f57\u7f5a\u7f62\u7f8e\u7f9e\u7fa4\u7fc5\u8005\u804c\u8054"
    "\u806a\u8083\u80a4\u80ae\u80c1\u80c6\u80dc\u80e1\u80f6\u80f8\u80fd"
    "\u8102\u8106\u8138\u8131\u8170\u819c\u820d\u822c\u8230\u8272\u827a"
    "\u8282\u8292\u82cf\u82e5\u82f1\u8303\u8363\u836f\u83b1\u83b7\u83b9"
    "\u8425\u843d\u84dd\u84df\u84ec\u8511\u8537\u8584\u85cf\u864f\u8651"
    "\u865a\u866b\u867d\u867e\u8680\u86ee\u8840\u884c\u8863\u8865\u8868"
    "\u8877\u8884\u88c5\u88e4\u89c1\u89c2\u89c4\u89c6\u89c9\u89c8\u89e3"
    "\u89e6\u8a00\u8ba1\u8ba2\u8ba4\u8ba8\u8ba9\u8bad\u8bae\u8bb0\u8bb2"
    "\u8bb8\u8bba\u8bbe\u8bbf\u8bc1\u8bc4\u8bc6\u8bc9\u8bca\u8bcd\u8bd5"
    "\u8bd7\u8bda\u8bdd\u8be2\u8be5\u8be6\u8bed\u8bef\u8bf1\u8bf4\u8bf7"
    "\u8bf8\u8bfa\u8bfb\u8c03\u8c08\u8c0b\u8c22\u8c61\u8d1d\u8d1f\u8d21"
    "\u8d22\u8d23\u8d24\u8d25\u8d27\u8d28\u8d29\u8d2a\u8d2b\u8d2c\u8d2d"
    "\u8d2e\u8d34\u8d35\u8d39\u8d3a\u8d3c\u8d44\u8d4b\u8d4c\u8d4f\u8d5b"
    "\u8d5e\u8d60\u8d62\u8d76\u8d77\u8d8b\u8db3\u8dc3\u8def\u8df3\u8df5"
    "\u8f66\u8f68\u8f6c\u8f6e\u8f6f\u8f70\u8f74\u8f7b\u8f7d\u8f7f\u8f83"
    "\u8f85\u8f86\u8f89\u8f91\u8f93\u8fbe\u8fc1\u8fc7\u8fd0\u8fd1\u8fd8"
    "\u8fd9\u8fdb\u8fdc\u8fde\u8fdf\u8ff0\u8ff7\u9000\u9001\u9009\u900a"
    "\u9012\u901a\u9020\u903b\u9057\u9065\u9093\u90a3\u90ae\u90bb\u90c1"
    "\u90d1\u90e8\u90fd\u914d\u9152\u9177\u9178\u91cc\u91cd\u91ce\u91cf"
    "\u91d1\u9274\u94a2\u94a5\u94a6\u94a7\u94a9\u94ae\u94b1\u94bb\u94c1"
    "\u94c3\u94dc\u94dd\u94f6\u94fe\u94fa\u9500\u9501\u9505\u9510\u9519"
    "\u952e\u9526\u952e\u953b\u9547\u955c\u957f\u95e8\u95ee\u95f2\u95f4"
    "\u95f7\u95fb\u95fb\u9605\u961f\u9633\u9634\u9635\u9636\u9645\u9646"
    "\u9648\u9650\u9669\u966a\u96be\u96c6\u96e8\u96ea\u96f7\u96fe\u9759"
    "\u9762\u97e9\u97f3\u9875\u9876\u9879\u987a\u987b\u987d\u987e\u9884"
    "\u9886\u9891\u9898\u989c\u989d\u98ce\u98de\u98df\u996d\u996e\u9970"
    "\u9971\u997c\u9986\u9996\u9999\u9a6c\u9a71\u9a8c\u9a91\u9a97\u9a9a"
    "\u9aa8\u9ad8\u9b3c\u9c7c\u9e1f\u9e21\u9e3f\u9e45\u9e7f\u9ea6\u9ebb"
    "\u9ec4\u9ed1\u9f50\u9f7f"
)


def read_csv_dict(path: Path) -> list[dict[str, str]]:
    with path.open("r", encoding="utf-8-sig", newline="") as f:
        return list(csv.DictReader(f))


def write_csv_dict(path: Path, rows: list[dict[str, str]], fields: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8-sig", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def flatten_other_names(value: Any) -> list[str]:
    if value is None:
        return []
    if isinstance(value, str):
        parts = re.split(r"[,;\n\r]+", value)
    elif isinstance(value, list):
        parts = []
        for item in value:
            if isinstance(item, str):
                parts.extend(re.split(r"[,;\n\r]+", item))
    else:
        parts = []
    return [part.strip() for part in parts if part and part.strip()]


def looks_chinese(text: str) -> bool:
    if not HAS_HAN.search(text):
        return False
    if HAS_KANA.search(text) or HAS_HANGUL.search(text):
        return False
    if re.search(r"[ぁ-ゟ゠-ヿ]", text):
        return False
    return True


def chinese_score(text: str) -> tuple[int, int, int]:
    simplified = sum(1 for ch in text if ch in SIMPLIFIED_HINTS)
    zh_hint = sum(1 for ch in text if ch in ZH_HINTS)
    jp_penalty = sum(1 for ch in text if ch in JP_ONLY_HINTS)
    han_count = len(HAS_HAN.findall(text))
    ascii_penalty = len(re.findall(r"[A-Za-z]", text))
    length_penalty = abs(len(text) - 4)
    return (simplified * 3 + zh_hint * 2 + han_count - jp_penalty * 4 - ascii_penalty, -length_penalty, -len(text))


def choose_chinese_name(other_names: Any) -> tuple[str, list[str]]:
    candidates = []
    seen = set()
    for name in flatten_other_names(other_names):
        cleaned = name.strip()
        if cleaned in seen:
            continue
        seen.add(cleaned)
        if looks_chinese(cleaned):
            candidates.append(cleaned)
    if not candidates:
        return "", []
    candidates.sort(key=chinese_score, reverse=True)
    return candidates[0], candidates


def request_json(url: str, login: str, api_key: str, cookie: str, timeout: int) -> tuple[int, Any, str]:
    headers = {
        "Accept": "application/json",
        "User-Agent": "illuTag-translator/0.1 (official Danbooru API lookup)",
    }
    if cookie:
        headers["Cookie"] = cookie
    request = urllib.request.Request(url, headers=headers)
    if login and api_key:
        credentials = f"{login}:{api_key}".encode("utf-8")
        import base64

        request.add_header("Authorization", "Basic " + base64.b64encode(credentials).decode("ascii"))

    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            body = response.read().decode("utf-8")
            return response.status, json.loads(body), ""
    except urllib.error.HTTPError as exc:
        text = exc.read().decode("utf-8", errors="replace")
        return exc.code, None, text[:500]
    except Exception as exc:
        return 0, None, str(exc)


def wiki_urls(base_url: str, tag: str, login: str, api_key: str) -> list[str]:
    base = base_url.rstrip("/")
    quoted = urllib.parse.quote(tag, safe="")
    urls = [f"{base}/wiki_pages/{quoted}.json"]
    query = urllib.parse.urlencode({"search[title]": tag})
    urls.append(f"{base}/wiki_pages.json?{query}")
    return urls


def load_cache(cache_path: Path) -> dict[str, Any]:
    if not cache_path.exists():
        return {}
    with cache_path.open("r", encoding="utf-8") as f:
        return json.load(f)


def save_cache(cache_path: Path, cache: dict[str, Any]) -> None:
    cache_path.parent.mkdir(parents=True, exist_ok=True)
    tmp = cache_path.with_suffix(".tmp")
    with tmp.open("w", encoding="utf-8") as f:
        json.dump(cache, f, ensure_ascii=False, indent=2, sort_keys=True)
    tmp.replace(cache_path)


def fetch_wiki(tag: str, args: argparse.Namespace, cache: dict[str, Any]) -> dict[str, Any]:
    cached = cache.get(tag)
    if cached and not args.refresh:
        return cached

    last_error = ""
    for url in wiki_urls(args.base_url, tag, args.login, args.api_key):
        status, payload, error = request_json(url, args.login, args.api_key, args.cookie, args.timeout)
        if status == 200:
            if isinstance(payload, list):
                payload = payload[0] if payload else None
            record = {
                "status": "ok" if payload else "not_found",
                "http_status": status,
                "url": url,
                "payload": payload,
                "error": "",
            }
            cache[tag] = record
            return record
        if status == 404:
            last_error = "404 not found"
            continue
        if status == 429:
            time.sleep(args.retry_after)
            continue
        last_error = f"HTTP {status}: {error}"
        break

    record = {
        "status": "error" if last_error else "not_found",
        "http_status": 0,
        "url": "",
        "payload": None,
        "error": last_error,
    }
    cache[tag] = record
    return record


def load_fallback(path: Path) -> dict[str, dict[str, str]]:
    rows = read_csv_dict(path)
    return {row["tag"]: row for row in rows}


def build_output_row(input_row: dict[str, str], fallback: dict[str, dict[str, str]], wiki: dict[str, Any]) -> dict[str, str]:
    tag = input_row["tag"]
    fallback_row = fallback.get(tag, {})
    payload = wiki.get("payload") or {}
    zh_name, candidates = choose_chinese_name(payload.get("other_names"))
    donmai_url = f"https://danbooru.donmai.us/wiki_pages/{urllib.parse.quote(tag, safe='')}"

    if zh_name:
        return {
            "tag": tag,
            "count": input_row["count"],
            "zh_name": zh_name,
            "confidence": "high" if len(candidates) == 1 else "medium",
            "source": "danbooru_api_other_names",
            "note": "; ".join(candidates[:8]),
            "donmai_url": donmai_url,
            "api_status": wiki.get("status", ""),
        }

    return {
        "tag": tag,
        "count": input_row["count"],
        "zh_name": fallback_row.get("zh_name", ""),
        "confidence": fallback_row.get("confidence", "low"),
        "source": "local_auto_split",
        "note": fallback_row.get("note", wiki.get("error", "")),
        "donmai_url": donmai_url,
        "api_status": wiki.get("status", ""),
    }


def parse_args() -> argparse.Namespace:
    root = Path(__file__).resolve().parent
    default_input = root / "selected_tags_missing_from_dictionary01.csv"
    default_fallback = root / "\u7b49\u5f85\u6821\u5bf92" / "selected_tags_missing_from_dictionary01_zh_translation_all.csv"
    default_output = root / "\u7b49\u5f85\u6821\u5bf93"
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, default=default_input)
    parser.add_argument("--fallback", type=Path, default=default_fallback)
    parser.add_argument("--output-dir", type=Path, default=default_output)
    parser.add_argument("--base-url", default=os.environ.get("DANBOORU_BASE_URL", "https://danbooru.donmai.us"))
    parser.add_argument("--login", default=os.environ.get("DANBOORU_LOGIN", ""))
    parser.add_argument("--api-key", default=os.environ.get("DANBOORU_API_KEY", ""))
    parser.add_argument("--cookie", default=os.environ.get("DANBOORU_COOKIE", ""))
    parser.add_argument("--batch-size", type=int, default=500)
    parser.add_argument("--delay", type=float, default=0.25)
    parser.add_argument("--timeout", type=int, default=30)
    parser.add_argument("--retry-after", type=float, default=5.0)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--refresh", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    input_rows = read_csv_dict(args.input)
    if args.limit:
        input_rows = input_rows[: args.limit]
    fallback = load_fallback(args.fallback)
    cache_path = args.output_dir / "danbooru_wiki_cache.json"
    cache = load_cache(cache_path)

    output_rows: list[dict[str, str]] = []
    for index, row in enumerate(input_rows, start=1):
        tag = row["tag"]
        wiki = fetch_wiki(tag, args, cache)
        output_rows.append(build_output_row(row, fallback, wiki))
        if index % 25 == 0:
            save_cache(cache_path, cache)
            print(f"processed {index}/{len(input_rows)}")
        time.sleep(args.delay)

    save_cache(cache_path, cache)

    fields = ["tag", "count", "zh_name", "confidence", "source", "note", "donmai_url", "api_status"]
    all_path = args.output_dir / "selected_tags_missing_from_dictionary01_zh_translation_danbooru_all.csv"
    write_csv_dict(all_path, output_rows, fields)

    for start in range(0, len(output_rows), args.batch_size):
        end = min(start + args.batch_size, len(output_rows))
        batch_path = args.output_dir / (
            f"selected_tags_missing_from_dictionary01_zh_translation_danbooru_"
            f"{start + 1:04d}_{end:04d}.csv"
        )
        write_csv_dict(batch_path, output_rows[start:end], fields)

    stats: dict[str, int] = {}
    for row in output_rows:
        stats[row["source"]] = stats.get(row["source"], 0) + 1
    print(json.dumps({"rows": len(output_rows), "sources": stats, "output": str(args.output_dir)}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
