from __future__ import annotations

import argparse
import csv
import re
from pathlib import Path


FULL_TAGS: dict[str, tuple[str, str, str]] = {
    "?": ("？", "high", "符号表情标签"),
    "^^^": ("^^^", "medium", "符号/表情标签"),
    ";(": (";(", "medium", "符号/表情标签"),
    "loli": ("萝莉", "high", ""),
    "playboy_bunny": ("花花公子兔女郎", "high", ""),
    "tanlines": ("晒痕", "high", ""),
    "lab_coat": ("实验服", "high", ""),
    "streetwear": ("街头服饰", "high", ""),
    "rural": ("乡村", "high", ""),
    "baking": ("烘焙", "high", ""),
    "meandros": ("希腊回纹", "medium", "纹样名需校对"),
    "poi": ("poi", "low", "语气/梗标签，需人工确认"),
    "one-hour_drawing_challenge": ("一小时绘画挑战", "high", ""),
    "dakimakura_(medium)": ("抱枕（媒介）", "high", ""),
    "bc_freedom_(emblem)": ("BC自由学园（徽章）", "medium", "作品专属徽章"),
    "rhodes_island_logo_(arknights)": ("罗德岛标志（明日方舟）", "high", ""),
    "sailor_swimsuit_(idolmaster)": ("水手泳装（偶像大师）", "medium", "作品专属服装"),
    "kin-iro_mosaic_high_school_uniform": ("黄金拼图高中校服", "medium", "作品专属制服"),
    "limiter_(tsukumo_sana)": ("限制器（九十九佐命）", "medium", "角色关联物件"),
    "stakes_of_purgatory": ("炼狱七桩", "medium", "作品专属名词"),
    "ai_ai_gasa": ("相合伞", "high", ""),
}


TOKEN_ZH: dict[str, str] = {
    # colors
    "black": "黑色",
    "white": "白色",
    "red": "红色",
    "blue": "蓝色",
    "green": "绿色",
    "yellow": "黄色",
    "purple": "紫色",
    "pink": "粉色",
    "orange": "橙色",
    "brown": "棕色",
    "grey": "灰色",
    "gray": "灰色",
    "aqua": "水色",
    "multicolored": "多色",
    "colored": "有色",
    "gradient": "渐变",
    "striped": "条纹",
    "vertical": "竖向",
    "print": "印花",
    "fishnet": "网眼",
    "ribbed": "罗纹",
    "transparent": "透明",
    "see": "透",
    "through": "视",
    "wet": "湿",
    "stained": "染污",
    "torn": "破损",
    "unworn": "未穿戴",
    "detached": "分离式",
    "single": "单个",
    "multiple": "多个",
    "large": "大",
    "small": "小",
    "mini": "迷你",
    "oversized": "过大",
    "tight": "紧身",
    "loose": "宽松",
    "official": "官方",
    "alternate": "替换",
    "ambiguous": "不明",
    "female": "女性",
    "male": "男性",
    "mature": "成熟",
    "aged": "年龄",
    "down": "变小",
    "up": "变大",
    "younger": "年轻",
    "older": "年长",
    # clothing
    "clothes": "服装",
    "clothing": "服装",
    "costume": "服装",
    "hairstyle": "发型",
    "headwear": "头饰",
    "hat": "帽子",
    "cap": "帽子",
    "aviator": "飞行员",
    "shirt": "衬衫",
    "sweater": "毛衣",
    "vest": "背心",
    "cardigan": "开衫",
    "jacket": "夹克",
    "coat": "外套",
    "cloak": "斗篷",
    "cape": "披肩",
    "dress": "连衣裙",
    "skirt": "裙子",
    "shorts": "短裤",
    "pants": "裤子",
    "pantyhose": "连裤袜",
    "thighhighs": "长筒袜",
    "socks": "袜子",
    "shoes": "鞋",
    "gloves": "手套",
    "glove": "手套",
    "fingerless": "露指",
    "bikini": "比基尼",
    "swimsuit": "泳装",
    "top": "上装",
    "bottom": "下装",
    "bandeau": "抹胸",
    "tube": "筒状",
    "babydoll": "娃娃裙",
    "serafuku": "水手服",
    "sailor": "水手",
    "collar": "衣领",
    "necktie": "领带",
    "bowtie": "领结",
    "neckerchief": "领巾",
    "scarf": "围巾",
    "belt": "腰带",
    "garter": "吊袜带",
    "ankle": "脚踝",
    "ankleband": "脚环",
    "bracelets": "手镯",
    "bracelet": "手镯",
    "jewelry": "饰品",
    "ornament": "饰品",
    "off": "露出",
    "shoulder": "肩",
    "sleeves": "袖子",
    "sleeve": "袖子",
    "neckline": "领口",
    "front": "前部",
    "side": "侧边",
    "tie": "系带",
    "tucked": "塞入",
    "around": "围绕",
    # body
    "hair": "头发",
    "skin": "皮肤",
    "sclera": "巩膜",
    "teeth": "牙齿",
    "upper": "上",
    "eyebrows": "眉毛",
    "brow": "眉",
    "face": "脸",
    "head": "头",
    "hand": "手",
    "hands": "双手",
    "finger": "手指",
    "fingers": "手指",
    "arm": "手臂",
    "arms": "手臂",
    "waist": "腰",
    "hips": "臀部",
    "hip": "臀部",
    "legs": "腿",
    "feet": "脚",
    "foot": "脚",
    "knees": "膝盖",
    "breast": "乳房",
    "breasts": "乳房",
    "nipple": "乳头",
    "nipples": "乳头",
    "pubic": "阴部",
    "penis": "阴茎",
    "cum": "精液",
    "masturbation": "自慰",
    "after": "之后",
    "nude": "裸体",
    "naked": "裸体",
    "topless": "上身裸露",
    "privates": "私处",
    "pectoral": "胸肌",
    "pectorals": "胸肌",
    "cleavage": "乳沟",
    "muscular": "肌肉发达",
    "scar": "伤疤",
    "lips": "嘴唇",
    "pupils": "瞳孔",
    "ears": "耳朵",
    "tail": "尾巴",
    "horns": "角",
    "antlers": "鹿角",
    "ahoge": "呆毛",
    # poses/actions
    "holding": "拿着",
    "grabbing": "抓住",
    "covering": "遮住",
    "hugging": "抱着",
    "squeezed": "挤压",
    "together": "在一起",
    "hidden": "隐藏",
    "by": "被",
    "object": "物体",
    "aside": "拨开",
    "only": "仅",
    "out": "露出",
    "in": "在",
    "on": "在",
    "own": "自己的",
    "another": "他人的",
    "turning": "转动",
    "ruffling": "揉乱",
    "focus": "特写",
    "frame": "框",
    "tickling": "挠痒",
    "measuring": "测量",
    "flying": "飞起",
    "behind": "在后方",
    "across": "横过",
    "apart": "分开",
    "closed": "闭合",
    "lift": "掀起",
    "lying": "躺着",
    "sitting": "坐着",
    "standing": "站着",
    # objects
    "bell": "铃铛",
    "fan": "扇子",
    "ring": "泳圈",
    "weapon": "武器",
    "club": "棍棒",
    "staff": "杖",
    "bo": "长棍",
    "pole": "杆",
    "polearm": "长柄武器",
    "sheath": "鞘",
    "knife": "刀",
    "torch": "火把",
    "key": "钥匙",
    "newspaper": "报纸",
    "magnifying": "放大",
    "glass": "镜",
    "plectrum": "拨片",
    "oar": "桨",
    "needle": "针",
    "bottle": "瓶",
    "shampoo": "洗发水",
    "soy": "酱油",
    "sauce": "酱",
    "oven": "烤箱",
    "package": "包裹",
    "stool": "凳子",
    "chair": "椅子",
    "bathtub": "浴缸",
    "sheets": "床单",
    "blanket": "毯子",
    "border": "边框",
    "background": "背景",
    "grid": "网格",
    "logo": "标志",
    "emblem": "徽章",
    "watermark": "水印",
    "sample": "样张",
    "market": "市场",
    "stall": "摊位",
    "hieroglyphics": "象形文字",
    "piano": "钢琴",
    "keys": "琴键",
    "audio": "音频",
    "jack": "插孔",
    "energy": "能量",
    "drink": "饮料",
    "chocolate": "巧克力",
    "heart": "心形",
    "organ": "器官",
    "constellation": "星座",
    "liquid": "液体",
    "red_liquid": "红色液体",
    "chili": "辣椒",
    "pepper": "辣椒",
    "race": "比赛",
    "bib": "号码布",
    "button": "纽扣",
    "rag": "抹布",
    "fusuma": "隔扇",
    # animals / species
    "animal": "动物",
    "rabbit": "兔子",
    "bug": "虫",
    "furry": "兽人",
    "cat": "猫",
    "dog": "狗",
    "monkey": "猴",
    "moth": "蛾",
    "bat": "蝙蝠",
    "deer": "鹿",
    "turtle": "龟",
    "shark": "鲨鱼",
    "rooster": "公鸡",
    "arthropod": "节肢动物",
    "minotaur": "弥诺陶洛斯",
    "robot": "机器人",
    "ghost": "幽灵",
    "witch": "女巫",
    "boy": "男孩",
    "girl": "女孩",
    "person": "人物",
    "ship": "舰船",
    "abyssal": "深海",
    # emotions / misc
    "worried": "担心",
    "disgust": "厌恶",
    "affectionate": "亲昵",
    "ojou": "大小姐",
    "sama": "大人",
    "pose": "姿势",
    "like": "点赞",
    "retweet": "转发",
    "ligne": "线条",
    "claire": "清线派",
    "hard": "硬质",
    "helmet": "头盔",
    "service": "制服",
    "female_service": "女性制服",
    "singlet": "背心",
    "kittysuit": "猫咪紧身衣",
}


CONNECTORS = {
    "of",
    "the",
    "a",
    "an",
    "and",
    "to",
    "with",
    "from",
    "at",
}

TOKEN_ZH.update(
    {
        "dark": "\u6df1\u8272",
        "skinned": "\u76ae\u80a4",
        "completely": "\u5b8c\u5168",
        "cutout": "\u5f00\u6d1e",
        "bun": "\u53d1\u9afb",
        "another's": "\u4ed6\u4eba\u7684",
        "uniform": "\u5236\u670d",
        "school": "\u5b66\u6821",
        "academy": "\u5b66\u56ed",
        "two": "\u4e24\u4e2a",
        "one": "\u4e00\u4e2a",
        "double": "\u53cc",
        "mouth": "\u5634",
        "eyes": "\u773c\u775b",
        "eye": "\u773c",
        "ear": "\u8033\u6735",
        "eyewear": "\u773c\u955c",
        "bag": "\u5305",
        "bow": "\u8774\u8776\u7ed3",
        "fur": "\u6bdb\u76ae",
        "trimmed": "\u9970\u8fb9",
        "over": "\u8986\u5728",
        "neck": "\u8116\u5b50",
        "legwear": "\u817f\u90e8\u7a7f\u7740",
        "frilled": "\u8377\u53f6\u8fb9",
        "flower": "\u82b1",
        "ribbon": "\u4e1d\u5e26",
        "piece": "\u4ef6",
        "earrings": "\u8033\u73af",
        "leg": "\u817f",
        "leotard": "\u8fde\u4f53\u8863",
        "long": "\u957f",
        "food": "\u98df\u7269",
        "blood": "\u8840",
        "halo": "\u5149\u73af",
        "cross": "\u5341\u5b57",
        "mask": "\u9762\u5177",
        "shaped": "\u5f62",
        "wings": "\u7fc5\u8180",
        "choker": "\u9879\u5708",
        "under": "\u4e0b",
        "star": "\u661f\u661f",
        "footwear": "\u978b\u7c7b",
        "tattoo": "\u7eb9\u8eab",
        "grab": "\u6293\u4f4f",
        "hoodie": "\u8fde\u5e3d\u886b",
        "ascot": "\u9886\u5dfe",
        "bird": "\u9e1f",
        "looking": "\u770b\u7740",
        "nose": "\u9f3b\u5b50",
        "sports": "\u8fd0\u52a8",
        "chest": "\u80f8\u90e8",
        "pull": "\u62c9",
        "suit": "\u5957\u88c5",
        "back": "\u80cc\u540e",
        "sash": "\u8170\u5e26",
        "kimono": "\u548c\u670d",
        "bangs": "\u5218\u6d77",
        "body": "\u8eab\u4f53",
        "stuffed": "\u586b\u5145",
        "open": "\u6253\u5f00",
        "panties": "\u5185\u88e4",
        "username": "\u7528\u6237\u540d",
        "piercing": "\u7a7f\u5b54",
        "hood": "\u515c\u5e3d",
        "sky": "\u5929\u7a7a",
        "gold": "\u91d1\u8272",
        "sword": "\u5251",
        "hairband": "\u53d1\u5e26",
        "mole": "\u75e3",
        "no": "\u65e0",
        "checkered": "\u68cb\u76d8\u683c",
        "pom": "\u7ed2\u7403",
        "strap": "\u5e26\u5b50",
        "fish": "\u9c7c",
        "wall": "\u5899",
        "sex": "\u6027\u4ea4",
        "robe": "\u957f\u888d",
        "cheek": "\u8138\u988a",
        "chain": "\u94fe\u6761",
        "necklace": "\u9879\u94fe",
        "tinted": "\u67d3\u8272",
        "glowing": "\u53d1\u5149",
        "floating": "\u6f02\u6d6e",
        "table": "\u684c\u5b50",
        "between": "\u4e4b\u95f4",
        "butterfly": "\u8774\u8776",
        "thigh": "\u5927\u817f",
        "wrist": "\u624b\u8155",
        "connection": "\u8fde\u63a5",
        "brooch": "\u80f8\u9488",
        "gemstone": "\u5b9d\u77f3",
        "ass": "\u5c41\u80a1",
        "mechanical": "\u673a\u68b0",
        "low": "\u4f4e",
        "hooded": "\u5e26\u515c\u5e3d",
        "capelet": "\u5c0f\u62ab\u80a9",
        "text": "\u6587\u5b57",
        "flag": "\u65d7\u5e1c",
        "cuffs": "\u8896\u53e3",
        "tree": "\u6811",
        "licking": "\u8214",
        "dot": "\u70b9",
        "stubble": "\u80e1\u832c",
        "apron": "\u56f4\u88d9",
        "underwear": "\u5185\u8863",
        "sided": "\u53cc\u9762",
        "plaid": "\u683c\u7eb9",
        "bodysuit": "\u7d27\u8eab\u8863",
        "bear": "\u718a",
        "ball": "\u7403",
        "broken": "\u7834\u635f",
        "implied": "\u6697\u793a",
        "coin": "\u786c\u5e01",
        "tank": "\u5766\u514b",
        "laced": "\u857e\u4e1d\u8fb9",
        "umbrella": "\u4f1e",
        "floor": "\u5730\u677f",
        "light": "\u6d45\u8272",
        "eyeshadow": "\u773c\u5f71",
        "symbol": "\u7b26\u53f7",
        "pillow": "\u6795\u5934",
        "kissing": "\u4eb2\u543b",
        "feathers": "\u7fbd\u6bdb",
        "bandaid": "\u521b\u53ef\u8d34",
        "diamond": "\u83f1\u5f62",
        "forehead": "\u989d\u5934",
        "year": "\u5e74",
        "thick": "\u539a",
        "game": "\u6e38\u620f",
        "skull": "\u9ab7\u9ac5",
        "leaf": "\u53f6\u5b50",
        "gun": "\u67aa",
        "armor": "\u76d4\u7532",
        "huge": "\u5de8\u5927",
        "riding": "\u9a91\u4e58",
        "camisole": "\u540a\u5e26\u80cc\u5fc3",
        "box": "\u76d2\u5b50",
        "self": "\u81ea\u5df1",
        "day": "\u65e5",
        "bunny": "\u5154\u5973\u90ce",
        "challenge": "\u6311\u6218",
        "partially": "\u90e8\u5206",
        "feather": "\u7fbd\u6bdb",
        "uneven": "\u4e0d\u5747\u5300",
        "crescent": "\u65b0\u6708",
        "goat": "\u5c71\u7f8a",
        "tassel": "\u6d41\u82cf",
        "mismatched": "\u4e0d\u5339\u914d",
        "bandaged": "\u7ef7\u5e26\u5305\u624e",
        "maid": "\u5973\u4ec6",
        "bolt": "\u87ba\u6813",
        "tiger": "\u8001\u864e",
        "gag": "\u53e3\u7403",
        "wing": "\u7fc5\u8180",
        "diagonal": "\u5bf9\u89d2",
        "tongue": "\u820c\u5934",
        "covered": "\u8986\u76d6",
        "beard": "\u80e1\u5b50",
        "shell": "\u58f3",
        "shared": "\u5171\u4eab",
        "leash": "\u7275\u7ef3",
        "fellatio": "\u53e3\u4ea4",
        "armband": "\u81c2\u7ae0",
        "cover": "\u8986\u76d6",
        "very": "\u975e\u5e38",
        "turtleneck": "\u9ad8\u9886",
        "cut": "\u5207\u5f00",
        "water": "\u6c34",
        "fake": "\u5047",
        "cropped": "\u77ed\u6b3e",
        "facial": "\u9762\u90e8",
        "mark": "\u6807\u8bb0",
        "above": "\u4e0a\u65b9",
        "lace": "\u857e\u4e1d",
        "peek": "\u9732\u51fa",
        "clock": "\u65f6\u949f",
        "cup": "\u676f\u5b50",
        "shota": "\u6b63\u592a",
        "themed": "\u4e3b\u9898",
        "bridal": "\u65b0\u5a18",
        "paper": "\u7eb8",
        "hakama": "\u88b4",
        "short": "\u77ed",
        "reference": "\u5f15\u7528",
        "imminent": "\u5373\u5c06",
        "knee": "\u819d",
        "split": "\u5206\u5f00",
        "toy": "\u73a9\u5177",
        "pipe": "\u7ba1",
        "argyle": "\u83f1\u683c",
        "can": "\u7f50",
        "traditional": "\u4f20\u7edf",
        "medium": "\u5a92\u4ecb",
    }
)


def split_tag(tag: str) -> tuple[list[str], str | None]:
    match = re.match(r"^(?P<body>.*?)(?:_?\((?P<paren>[^()]*)\))?$", tag)
    if not match:
        return re.split(r"[_-]+", tag), None
    body = match.group("body")
    paren = match.group("paren")
    tokens = [part for part in re.split(r"[_-]+", body) if part]
    return tokens, paren


def compose_translation(tag: str) -> tuple[str, str, str]:
    if tag in FULL_TAGS:
        return FULL_TAGS[tag]

    tokens, paren = split_tag(tag)
    translated: list[str] = []
    unknown: list[str] = []

    i = 0
    while i < len(tokens):
        if i + 1 < len(tokens):
            two = f"{tokens[i]}_{tokens[i + 1]}"
            if two in TOKEN_ZH:
                translated.append(TOKEN_ZH[two])
                i += 2
                continue
        token = tokens[i]
        if token in CONNECTORS:
            i += 1
            continue
        if token in TOKEN_ZH:
            translated.append(TOKEN_ZH[token])
        elif token.isdigit():
            translated.append(token)
        else:
            translated.append(token)
            unknown.append(token)
        i += 1

    zh = "".join(translated) if translated else tag
    if paren:
        paren_zh = TOKEN_ZH.get(paren, paren)
        zh = f"{zh}（{paren_zh}）"

    if unknown:
        confidence = "low" if len(unknown) >= 2 or len(unknown) == len(tokens) else "medium"
        note = "自动拆词；需校对未知片段: " + ", ".join(unknown[:5])
    else:
        confidence = "medium"
        note = "自动拆词组合，建议校对语序"

    return zh, confidence, note


def translate_rows(input_csv: Path) -> list[dict[str, str]]:
    with input_csv.open("r", encoding="utf-8-sig", newline="") as file:
        rows = list(csv.DictReader(file))

    translated_rows: list[dict[str, str]] = []
    for row in rows:
        tag = row["tag"].strip()
        zh_name, confidence, note = compose_translation(tag)
        translated_rows.append(
            {
                "tag": tag,
                "count": row["count"],
                "zh_name": zh_name,
                "confidence": confidence,
                "note": note,
            }
        )
    return translated_rows


def write_csv(path: Path, rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8-sig", newline="") as file:
        writer = csv.DictWriter(file, fieldnames=["tag", "count", "zh_name", "confidence", "note"])
        writer.writeheader()
        writer.writerows(rows)


def main() -> None:
    parser = argparse.ArgumentParser(description="Translate missing general tags into Chinese review drafts.")
    parser.add_argument("--input", type=Path, default=Path("selected_tags_missing_from_dictionary01.csv"))
    parser.add_argument("--output-dir", type=Path, default=Path("等待校对2"))
    parser.add_argument("--batch-size", type=int, default=500)
    parser.add_argument("--merged-name", default="selected_tags_missing_from_dictionary01_zh_translation_all.csv")
    args = parser.parse_args()

    rows = translate_rows(args.input)
    args.output_dir.mkdir(parents=True, exist_ok=True)

    for start in range(0, len(rows), args.batch_size):
        end = min(start + args.batch_size, len(rows))
        batch_name = f"selected_tags_missing_from_dictionary01_zh_translation_{start + 1:04d}_{end:04d}.csv"
        write_csv(args.output_dir / batch_name, rows[start:end])

    write_csv(args.output_dir / args.merged_name, rows)

    counts: dict[str, int] = {}
    for row in rows:
        counts[row["confidence"]] = counts.get(row["confidence"], 0) + 1
    print(f"translated rows: {len(rows)}")
    print("confidence:", counts)
    print(f"output dir: {args.output_dir}")
    print(f"merged: {args.output_dir / args.merged_name}")


if __name__ == "__main__":
    main()
