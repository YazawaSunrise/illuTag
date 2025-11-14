import gradio as gr
import os
import threading
import time
import sys
import argparse
import json
import subprocess
from typing import List, Dict, Tuple, Optional, Set
from PIL import Image

# 尝试导入 TensorFlow
try:
    import tensorflow as tf
except ImportError:
    pass

# 导入后端核心模块
# (我们将有条件地导入，以支持 --search-only)
from database_manager import DatabaseManager
from dictionary_manager import DictionaryManager

# --- 全局配置 ---
CONFIG_FILE = "illutag_config.json"
LOADED_CONFIG = {"allowed_paths": []}

# --- 启动模式 ---
# 1. 创建 ArgumentParser
parser = argparse.ArgumentParser(description="illuTag - 图像索引与搜索工具")
parser.add_argument(
    '--search-only',
    action='store_true',
    help='启动为搜索专用模式，不加载 AI 模型'
)
args = parser.parse_args()
SEARCH_ONLY_MODE = args.search_only

# --- (条件) 初始化全局服务实例 ---
DB_MANAGER = DatabaseManager()
DICTIONARY_MANAGER = DictionaryManager()
PROCESSOR = None
SCAN_ENGINE = None

if SEARCH_ONLY_MODE:
    print("--- 启动为 [搜索专用模式] ---")
    print("AI 模型将不会被加载。扫描功能将被禁用。")
else:
    print("--- 启动为 [完整模式] ---")
    try:
        # 仅在完整模式下导入和初始化
        from tag_processor import TagProcessor
        from scanner_engine import ScanEngine
        
        PROCESSOR = TagProcessor()
        SCAN_ENGINE = ScanEngine(PROCESSOR, DB_MANAGER)
        print("AI 模型已加载，扫描功能已启用。")
    except Exception as e:
        print(f"致命错误：在完整模式下加载核心服务失败。{e}")
        print("请检查 TensorFlow/Keras/模型文件是否完好。")
        sys.exit(1)

# --- 辅助函数 ---

def load_config():
    """在启动时加载配置文件"""
    global LOADED_CONFIG
    if os.path.exists(CONFIG_FILE):
        try:
            with open(CONFIG_FILE, 'r', encoding='utf-8') as f:
                config_data = json.load(f)
                # (新) 健壮性检查：确保加载的是字典
                if isinstance(config_data, dict):
                    LOADED_CONFIG = config_data
                else:
                    # 如果格式不是字典（例如只是一个列表），则重置
                    print(f"警告：{CONFIG_FILE} 格式无效 (非字典)，将重置。")
                    LOADED_CONFIG = {'allowed_paths': []}

            # 确保 'allowed_paths' 键存在且是列表
            if 'allowed_paths' not in LOADED_CONFIG or not isinstance(LOADED_CONFIG['allowed_paths'], list):
                print(f"警告：'allowed_paths' 键丢失或格式无效，将重置。")
                LOADED_CONFIG['allowed_paths'] = []
                
        except Exception as e:
            # (新) 修复 "list indices must be integers..." 错误
            # 如果 JSON 解析失败或类型错误，则重置
            print(f"警告：加载 {CONFIG_FILE} 失败: {e}。将重置为默认配置。")
            LOADED_CONFIG = {'allowed_paths': []}
            save_config() # (新) 保存重置后的配置
    else:
        # 如果文件不存在，创建一个空的
        save_config()
    
    # 确保至少包含当前工作目录
    cwd = os.path.normpath(os.getcwd())
    if cwd not in LOADED_CONFIG['allowed_paths']:
        LOADED_CONFIG['allowed_paths'].append(cwd)
    
    print(f"Gradio 已获准访问以下路径: {LOADED_CONFIG['allowed_paths']}")
    return LOADED_CONFIG

def save_config():
    """保存配置到 JSON 文件"""
    try:
        with open(CONFIG_FILE, 'w', encoding='utf-8') as f:
            json.dump(LOADED_CONFIG, f, indent=4, ensure_ascii=False)
    except Exception as e:
        print(f"错误：保存配置 {CONFIG_FILE} 失败: {e}")

def add_folder_to_config(folder_path: str):
    """(新) 添加文件夹到配置并保存"""
    global LOADED_CONFIG
    normalized_path = os.path.normpath(folder_path)
    
    if not os.path.isdir(normalized_path):
        # (新) 修复：返回 3 个值以匹配 outputs
        return f"错误: 路径 '{normalized_path}' 无效或不存在。", "\n".join(LOADED_CONFIG['allowed_paths']), gr.Dropdown(choices=LOADED_CONFIG['allowed_paths'])

    if normalized_path not in LOADED_CONFIG['allowed_paths']:
        LOADED_CONFIG['allowed_paths'].append(normalized_path)
        save_config()
        
        folders_list = "\n".join(LOADED_CONFIG['allowed_paths'])
        msg = f"成功添加: {normalized_path}\n\n请注意：您必须重启本应用 (app.py) 才能在图库中查看此新文件夹的图片。"
        # (新) 修复：返回 3 个值
        return msg, folders_list, gr.Dropdown(choices=LOADED_CONFIG['allowed_paths'], value=None)
    else:
        folders_list = "\n".join(LOADED_CONFIG['allowed_paths'])
        msg = f"路径: {normalized_path} 已在列表中。"
        # (新) 修复：返回 3 个值
        return msg, folders_list, gr.Dropdown(choices=LOADED_CONFIG['allowed_paths'])

# (新) 新增函数：用于从配置中移除文件夹
def remove_folder_from_config(folder_to_remove: str):
    """(新) 从配置中移除一个文件夹"""
    global LOADED_CONFIG
    
    if not folder_to_remove:
        return "错误：未选择文件夹。", "\n".join(LOADED_CONFIG['allowed_paths']), gr.Dropdown(choices=LOADED_CONFIG['allowed_paths'], value=None)

    # (新) 安全检查：不允许移除当前工作目录
    cwd = os.path.normpath(os.getcwd())
    if os.path.normpath(folder_to_remove) == cwd:
        msg = f"错误：不能移除当前工作目录 ({cwd})。"
        return msg, "\n".join(LOADED_CONFIG['allowed_paths']), gr.Dropdown(choices=LOADED_CONFIG['allowed_paths'], value=None)

    if folder_to_remove in LOADED_CONFIG['allowed_paths']:
        LOADED_CONFIG['allowed_paths'].remove(folder_to_remove)
        save_config()
        msg = f"成功移除: {folder_to_remove}\n\n请注意：您必须重启本应用 (app.py) 才能使此更改完全生效。"
        # (新) 更新 choices
        new_choices = LOADED_CONFIG['allowed_paths']
        return msg, "\n".join(new_choices), gr.Dropdown(choices=new_choices, value=None)
    else:
        msg = f"错误：未在列表中找到: {folder_to_remove}"
        return msg, "\n".join(LOADED_CONFIG['allowed_paths']), gr.Dropdown(choices=LOADED_CONFIG['allowed_paths'], value=None)


def start_rescan_all_folders_thread():
    """(新) 在新线程中启动对所有管理文件夹的扫描"""
    global LOADED_CONFIG
    if SCAN_ENGINE.get_status().get("is_scanning"):
        return "扫描正在进行中..."

    folders_to_scan = LOADED_CONFIG.get('allowed_paths', [])
    if not folders_to_scan:
        return "错误：没有已管理的文件夹可供扫描。"
        
    def scan_all():
        print(f"开始重新扫描所有 {len(folders_to_scan)} 个文件夹...")
        for folder in folders_to_scan:
            if os.path.isdir(folder):
                print(f"--- 正在扫描: {folder} ---")
                # start_scan 是一个阻塞操作，它会完成一个文件夹再到下一个
                SCAN_ENGINE.start_scan(folder, None, force_rescan=False)
            else:
                print(f"跳过无效路径: {folder}")
        print("所有文件夹扫描完成。")

    threading.Thread(target=scan_all, daemon=True).start()
    return f"开始重新扫描所有 {len(folders_to_scan)} 个已添加的文件夹..."

def check_scan_status():
    """定期检查扫描状态，并更新进度条。"""
    # 如果在搜索模式，SCAN_ENGINE 为 None
    if SCAN_ENGINE is None:
        return 0.0, "扫描功能已禁用 (搜索专用模式)"
        
    status = SCAN_ENGINE.get_status()
    
    is_scanning = status['is_scanning']
    total = status['total_files']
    processed = status['files_processed'] 
    percent = status['progress_percent']
    folder = status['folder']
    
    # 计算进度条值 (0.0 到 1.0)
    if total == 0:
        progress = 0.0
    else:
        progress = processed / total
            
    if is_scanning:
        status_text = f"正在扫描: {os.path.basename(folder)} | 进度: {processed}/{total} ({percent}%)"
        return progress, status_text
    else:
        # 初始状态或空任务完成
        initial_processed = DB_MANAGER.get_all_indexed_file_paths()
        initial_count = len(initial_processed)
        if initial_count > 0:
             status_text = f"等待启动扫描... (数据库中已索引 {initial_count} 个文件)"
        else:
             status_text = "等待启动扫描..."
        return 0.0, status_text

def get_cn_suggestions(cn_partial_input: str) -> Tuple[gr.Dropdown, str]:
    """
    根据中文模糊输入，获取联想到的中文标签列表，并更新下拉框。
    (只显示数据库中已有的英文标签对应的中文翻译)
    """
    cn_partial_input = cn_partial_input.strip()
    if not cn_partial_input:
        return gr.Dropdown(choices=[], value=None, visible=False), ""

    # 1. 获取数据库中已存在的所有英文标签 
    allowed_en_tags = DB_MANAGER.get_all_indexed_tags()
    
    if not allowed_en_tags:
        msg = "数据库中没有索引标签。请先进行扫描。"
        return gr.Dropdown(choices=[], value=None, visible=False), msg

    # 2. 模糊查找所有包含该词的精确中文标签，并根据数据库标签集进行过滤
    suggestions = DICTIONARY_MANAGER.fuzzy_lookup_suggestions(
        cn_partial_input, 
        allowed_en_tags=allowed_en_tags
    )

    if suggestions:
        options = suggestions
        msg = f"已找到 {len(suggestions)} 个包含 '{cn_partial_input}' 的联想词 (已过滤)。"
        return gr.Dropdown(choices=options, value=None, visible=True), msg
    else:
        msg = f"未找到任何包含 '{cn_partial_input}' 的中文标签，或它们未被用于任何图片。"
        return gr.Dropdown(choices=[], value=None, visible=False), msg


def search_images_wrapper(
    cn_partial_input: str, 
    cn_selected_tag: Optional[str], 
    english_input: str, 
    file_name_input: str,
    min_score: float,
    max_score: float,
    show_favorites: bool
) -> Tuple[List[Tuple], str, gr.Dropdown, List[Dict], Dict, None]: # (新) 添加 None 用于清空 selected_item
    """
    (新) 搜索逻辑。
    返回: (图库数据, 状态消息, 重置的下拉框, 完整的原始结果集, 图库路径映射, (新)清空选中项)
    """
    
    # 1. ----- 确定搜索意图 -----
    
    # 规范化输入
    cn_partial_input = cn_partial_input.strip()
    english_input = english_input.strip().lower()
    file_name_input = file_name_input.strip().lower()
    
    # cn_terms_to_search: 最终用于精确匹配的 "中文标签" 列表
    cn_terms_to_search: List[str] = []
    
    # user_intended_search: 用户是否在任何一个框中输入了内容？
    user_intended_search = bool(cn_partial_input or english_input or cn_selected_tag or file_name_input or show_favorites)
    
    search_msg_parts = []

    # 2. ----- 确定中文搜索标签 (精确匹配) -----
    if cn_selected_tag:
        # 模式 A: 用户选择了特定的精确中文标签 (优先级最高)
        cn_terms_to_search = [cn_selected_tag]
        search_msg_parts.append(f"中文精确: '{cn_selected_tag}'")
    elif cn_partial_input:
        # 模式 B: 用户提供了模糊输入，但未选择 -> 搜索所有模糊匹配
        allowed_en_tags = DB_MANAGER.get_all_indexed_tags()
        fuzzy_matches = DICTIONARY_MANAGER.fuzzy_lookup_suggestions(cn_partial_input, allowed_en_tags=allowed_en_tags)
        cn_terms_to_search = fuzzy_matches
        search_msg_parts.append(f"中文模糊: '{cn_partial_input}' (匹配 {len(fuzzy_matches)} 个)")
    
    # cn_search_tags: 从中文精确匹配转换来的 "英文标签" 集合
    cn_search_tags = set(DICTIONARY_MANAGER.get_search_tags_from_cn_list(cn_terms_to_search))

    # 3. ----- 确定英文搜索标签 (模糊匹配) -----
    # en_fuzzy_terms: 从英文输入框解析出的 "英文模糊词" 列表
    en_fuzzy_terms: List[str] = []
    if english_input:
        en_fuzzy_terms = [t.strip() for t in english_input.replace(",", " ").split() if t.strip()]
        search_msg_parts.append(f"英文模糊: {en_fuzzy_terms}")

    # 4. ----- 检查是否为失败的搜索 -----
    if user_intended_search and not cn_search_tags and not en_fuzzy_terms and not file_name_input and not show_favorites:
        # 用户输入了内容 (例如 'wa')，但中文联想失败 (0个匹配)，且英文框为空
        final_message = f"未找到 '{cn_partial_input}' 对应的任何标签。显示 0 个结果。"
        return [], final_message, gr.Dropdown(choices=[], value=None), [], {}, None

    # 5. ----- 获取基础数据 -----
    all_images_data = DB_MANAGER.get_all_indexed_images()
    
    output_data = []
    filtered_raw_results = [] # 存储过滤后的完整数据
    
    if not all_images_data:
        return [], "数据库为空。请先扫描图片。", gr.Dropdown(choices=[], value=None), [], {}, None

    # 6. ----- 在 Python 中执行过滤循环 -----
    for item in all_images_data:
        
        # 过滤器 1: 收藏夹
        if show_favorites and not item['is_favorite']:
            continue # 如果要求收藏，但这张不是，则跳过

        # 过滤器 2: 文件名
        if file_name_input and file_name_input not in item['file_path'].lower():
            continue # 如果提供了文件名，但不匹配，则跳过

        # 过滤器 3: 标签和分数
        
        if not cn_search_tags and not en_fuzzy_terms:
            # 这种情况 = (仅文件名搜索) 或 (仅收藏搜索) 或 (显示全部)
            
            if not user_intended_search:
                # 显示所有图片 (需要应用分数范围)
                tags_in_range = [
                    f"{t['tag_name']} ({t['score']:.2f})" 
                    for t in item['tags'] 
                    if min_score <= t['score'] <= max_score
                ]
                
                if not tags_in_range:
                    continue 
                
                title = f"{os.path.basename(item['file_path'])}\n\n高分标签:\n" + "\n".join(tags_in_range[:5]) + "..."
            
            else:
                # 这种情况 = 仅文件名/收藏夹搜索 (显示所有标签)
                all_tags = [f"{t['tag_name']} ({t['score']:.2f})" for t in item['tags']]
                title = f"{os.path.basename(item['file_path'])}\n\n所有标签:\n" + "\n".join(all_tags[:5]) + "..."
            
            output_data.append((item['file_path'], title))
            filtered_raw_results.append(item)
            continue

        # --- 如果执行到这里，说明用户 *确实* 输入了标签 (cn or en) ---

        matched_tags = [] # 存储这张图片上匹配的标签

        for tag_info in item['tags']:
            tag_name = tag_info['tag_name']
            score = tag_info['score']
            
            # 检查分数范围
            if not (min_score <= score <= max_score):
                continue 

            is_match = False
            
            # 检查是否匹配中文精确搜索
            if tag_name in cn_search_tags:
                matched_tags.append(f"{tag_name} [中] ({score:.2f})")
                is_match = True

            # 检查是否匹配英文模糊搜索
            # (如果已匹配中文，则不再检查英文，避免重复)
            if not is_match:
                for term in en_fuzzy_terms:
                    if term in tag_name:
                        matched_tags.append(f"{tag_name} [英] ({score:.2f})")
                        break # 只要一个模糊词匹配就行

        # --- 循环结束 ---
        
        if matched_tags:
            title = f"{os.path.basename(item['file_path'])}\n\n匹配的标签:\n" + "\n".join(matched_tags)
            output_data.append((item['file_path'], title))
            filtered_raw_results.append(item)

    # 7. ----- 返回结果 -----
    
    # (新) 构建图库路径映射 (index -> file_path)
    # 这对于 'open_image_file' 和 'on_gallery_select' 至关重要
    gallery_state = {i: item['file_path'] for i, item in enumerate(filtered_raw_results)}
    
    if file_name_input:
        search_msg_parts.append(f"文件名: '{file_name_input}'")
    if show_favorites:
        search_msg_parts.append("仅显示收藏")

    if not search_msg_parts:
        final_message = f"显示所有 {len(output_data)} 张图片 (分数范围 {min_score:.1f} ~ {max_score:.1f})。"
    else:
        final_message = f"搜索条件: " + " & ".join(search_msg_parts) + f" (分数 {min_score:.1f} ~ {max_score:.1f})。\n找到 {len(output_data)} 张匹配图片。"
    
    # (新) 返回 None 清空 selected_item
    return output_data, final_message, gr.Dropdown(choices=[], value=None), filtered_raw_results, gallery_state, None


def load_initial_gallery(min_score, max_score):
    """(新) 在 Gradio 加载时调用，以显示所有图片"""
    # 默认不显示收藏，不过滤标签
    return search_images_wrapper(
        cn_partial_input="",
        cn_selected_tag=None,
        english_input="",
        file_name_input="",
        min_score=min_score,
        max_score=max_score,
        show_favorites=False
    )

def open_image_file(
    # (新) 修复：输入改为 'current_selected_item_state'
    current_selected_item: Optional[Dict]
):
    """(新) 点击“打开原文件”按钮时触发"""
    
    # (新) 修复：使用 'current_selected_item'
    if current_selected_item is None:
        print("打开文件失败：未选中图片。")
        return
        
    try:
        selected_item_path = current_selected_item.get('file_path')
        
        if selected_item_path:
            path = os.path.normpath(selected_item_path)
            
            if not os.path.exists(path):
                print(f"错误：路径不存在: {path}")
                return

            print(f"正在尝试打开 (系统默认): {path}")
            
            if sys.platform == "win32":
                os.startfile(path)
            elif sys.platform == "darwin": # macOS
                subprocess.call(["open", path])
            else: # Linux
                subprocess.call(["xdg-open", path])
        else:
            print(f"错误：选中的项目没有 'file_path'")
            
    except Exception as e:
        print(f"打开文件失败: {e}")


def on_gallery_select(
    evt: gr.SelectData, 
    current_results_state: List[Dict],
    current_gallery_state: Dict
):
    """
    (新) 当用户在图库中选择一张图片时触发。
    更新“收藏”按钮、“标签”区域和 (新) 'current_selected_item_state'。
    """
    if evt is None:
        return gr.Button("收藏 (未选择)", variant="secondary"), gr.Radio(choices=[], value=None, visible=False), None
        
    selected_index = evt.index
    selected_item = None
    
    selected_path = current_gallery_state.get(selected_index)
    
    if not selected_path:
        return gr.Button("收藏 (错误)", variant="secondary"), gr.Radio(choices=[], value=None, visible=False), None

    for item in current_results_state:
        if item['file_path'] == selected_path:
            selected_item = item
            break
            
    if selected_item is None:
        return gr.Button("收藏 (错误)", variant="secondary"), gr.Radio(choices=[], value=None, visible=False), None

    # --- 1. 更新收藏按钮 ---
    is_fav = selected_item['is_favorite']
    fav_btn_text = "❤️ 已收藏" if is_fav else "♡ 收藏"
    fav_btn_variant = "primary" if is_fav else "secondary"
    
    # --- 2. 更新标签 Radio ---
    sorted_tags = sorted(selected_item['tags'], key=lambda x: x['score'], reverse=True)
    
    tag_choices = []
    for tag_info in sorted_tags:
        en_tag = tag_info['tag_name']
        score = tag_info['score']
        
        cn_tag = DICTIONARY_MANAGER.lookup_en_to_cn(en_tag)
        
        if cn_tag:
            display_text = f"{cn_tag} ({en_tag}) [{score:.2f}]"
        else:
            display_text = f"{en_tag} [{score:.2f}]"
            
        tag_choices.append(display_text)

    # (新) 返回 selected_item 以更新状态
    return gr.Button(fav_btn_text, variant=fav_btn_variant), gr.Radio(choices=tag_choices, value=None, visible=True), selected_item


def on_favorite_button_click(
    # (新) 修复：输入改为 'current_selected_item_state'
    current_selected_item: Optional[Dict],
    current_results_state: List[Dict]
):
    """
    (新) 当点击“收藏”按钮时触发。
    """
    # (新) 修复：使用 'current_selected_item'
    if current_selected_item is None:
        return gr.Button("收藏 (未选择)", variant="secondary"), current_results_state, None

    selected_item = current_selected_item
            
    # 切换数据库中的状态
    image_id = selected_item['image_id']
    try:
        new_status = DB_MANAGER.toggle_favorite_status(image_id)
        
        # 更新内存中的状态 (gr.State 和 完整列表)
        selected_item['is_favorite'] = new_status
        
        # (新) 在 'current_results_state' 中找到并更新
        for item in current_results_state:
            if item['image_id'] == image_id:
                item['is_favorite'] = new_status
                break
        
        # 更新按钮
        fav_btn_text = "❤️ 已收藏" if new_status else "♡ 收藏"
        fav_btn_variant = "primary" if new_status else "secondary"
        
        # (新) 返回更新后的 selected_item 和 results_state
        return gr.Button(fav_btn_text, variant=fav_btn_variant), current_results_state, selected_item

    except Exception as e:
        print(f"收藏切换失败: {e}")
        return gr.Button("收藏 (错误)", variant="secondary"), current_results_state, current_selected_item


def on_tag_select_and_search(
    selected_tag_display: str,
    file_name_input: str,
    show_favorites: bool
):
    """
    (新) 当用户点击了图片下方的某个标签时触发。
    """
    
    if not selected_tag_display:
        return (
            gr.Textbox(), 
            gr.Dropdown(), 
            gr.Textbox(), 
            gr.Textbox(value=file_name_input), 
            gr.Checkbox(value=show_favorites), 
            gr.Gallery(),
            gr.Textbox(),
            gr.State(),
            gr.State(), 
            gr.State() # (新) 对应 selected_item_state
        )

    # 解析标签
    cn_search = ""
    en_search = ""
    
    if '(' in selected_tag_display and ')' in selected_tag_display:
        try:
            cn_search = selected_tag_display.split('(')[0].strip()
            en_search = selected_tag_display.split('(')[1].split(')')[0].strip()
        except Exception:
            en_search = selected_tag_display.split('[')[0].strip()
    else:
        en_search = selected_tag_display.split('[')[0].strip()

    # (新) Bug 修复: 当设置 Dropdown 的 value 时，必须同时提供 choices
    # 否则在下一次搜索时会引发 'Value not in choices' 错误
    
    if cn_search and DICTIONARY_MANAGER.is_cn_tag(cn_search):
        # 使用中文进行精确搜索
        gallery, msg, dd_reset, raw_results, gallery_state, sel_item_reset = search_images_wrapper(
            cn_partial_input="", 
            cn_selected_tag=cn_search,
            english_input="",
            file_name_input=file_name_input,
            show_favorites=show_favorites,
            min_score=0.0,
            max_score=1.0
        )
        
        return (
            gr.Textbox(value=""), 
            # (新) 修复：同时设置 choices 和 value
            gr.Dropdown(choices=[cn_search], value=cn_search), 
            gr.Textbox(value=""), 
            gr.Textbox(value=file_name_input), 
            gr.Checkbox(value=show_favorites), 
            gallery,
            msg,
            raw_results,
            gallery_state,
            sel_item_reset # (新) 清空 selected_item
        )
    else:
        # 使用英文进行模糊搜索
        gallery, msg, dd_reset, raw_results, gallery_state, sel_item_reset = search_images_wrapper(
            cn_partial_input="", 
            cn_selected_tag=None, 
            english_input=en_search,
            file_name_input=file_name_input,
            show_favorites=show_favorites,
            min_score=0.0,
            max_score=1.0
        )

        return (
            gr.Textbox(value=""), 
            gr.Dropdown(choices=[], value=None), # 英文搜索不设置下拉框
            gr.Textbox(value=en_search), 
            gr.Textbox(value=file_name_input), 
            gr.Checkbox(value=show_favorites), 
            gallery,
            msg,
            raw_results,
            gallery_state,
            sel_item_reset # (新) 清空 selected_item
        )


# --- 启动时加载配置 ---
load_config()


# --- Gradio 界面定义 ---

custom_css = """
#fixed_gallery .grid-container {
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)) !important;
}
"""

with gr.Blocks(title="illuTag - 图像索引与搜索工具", css=custom_css) as demo:
    
    # 核心状态
    current_results_state = gr.State([])
    current_gallery_state = gr.State({}) # (新) index -> file_path
    current_selected_item_state = gr.State(None) # (新) 存储选中的 {item dict}

    # --- 搜索选项卡 (默认) ---
    with gr.Tab("🔎 标签搜索"):
        with gr.Row():
            
            # --- 左侧搜索栏 ---
            with gr.Column(scale=1):
                gr.Markdown("## 搜索选项")
                
                with gr.Row():
                    show_favorites_checkbox = gr.Checkbox(
                        label="★ 仅显示收藏",
                        value=False
                    )
                
                with gr.Column(variant="panel"):
                    gr.Markdown("### 1. 中文模糊搜索 (联想)")
                    cn_partial_input = gr.Textbox(
                        label="输入子串 (如 '眼睛')", 
                        placeholder="例如: 眼睛",
                        scale=1
                    )
                    cn_suggestion_dropdown = gr.Dropdown(
                        label="2. 联想到的精确中文标签 (可选)",
                        choices=[],
                        value=None,
                        interactive=True,
                        allow_custom_value=False,
                        scale=1
                    )
                    cn_suggestion_msg = gr.Textbox(label="联想状态", interactive=False)
                
                with gr.Column(variant="panel"):
                    gr.Markdown("### 2. 英文标签 (模糊, 空格分隔)")
                    english_tag_input = gr.Textbox(
                        label="例如: long_hair, outdoors", 
                        placeholder="long_hair outdoors"
                    )
                
                with gr.Column(variant="panel"):
                    gr.Markdown("### 3. 文件名 (模糊)")
                    file_name_input = gr.Textbox(
                        label="例如: 12345_p0.jpg", 
                        placeholder="12345"
                    )

                gr.Markdown("### 4. 标签分数范围")
                min_score_slider = gr.Slider(
                    minimum=0.0, maximum=1.0, step=0.01, 
                    value=0.5, label="最低分数"
                )
                max_score_slider = gr.Slider(
                    minimum=0.0, maximum=1.0, step=0.01, 
                    value=1.0, label="最高分数"
                )
                
                search_btn = gr.Button("🔍 搜索图片", variant="primary")
                
                gr.Markdown("### 5. 图片交互")
                open_file_btn = gr.Button("📂 打开原文件")
                
                favorite_btn = gr.Button("♡ 收藏 (未选择)", variant="secondary")


            # --- 右侧图库 ---
            with gr.Column(scale=3):
                search_msg_output = gr.Textbox(label="搜索结果摘要", interactive=False, lines=2)
                
                image_gallery = gr.Gallery(
                    label="搜索结果",
                    height="auto",
                    columns=4,
                    rows=2,
                    preview=False, 
                    object_fit="contain",
                    elem_id="fixed_gallery"
                )

                tag_display_area = gr.Radio(
                    label="选中图片的标签 (点击可搜索)",
                    choices=[],
                    value=None,
                    visible=False,
                    interactive=True
                )

    # --- 扫描选项卡 ---
    with gr.Tab("📁 扫描与打标", visible=(not SEARCH_ONLY_MODE)) as scan_tab:
        
        with gr.Row():
            with gr.Column(scale=2):
                # (新) 重新组织 UI
                gr.Markdown("#### 1. 添加新文件夹")
                scan_folder_input = gr.Textbox(
                    label="要添加的图片文件夹路径", 
                    placeholder="例如: D:/MyImages/AnimeArt"
                )
                add_folder_btn = gr.Button("添加到管理列表", variant="secondary")
                
                gr.Markdown("#### 2. 移除文件夹") # (新)
                folder_to_remove_dd = gr.Dropdown( # (新)
                    label="选择要移除的文件夹",
                    choices=LOADED_CONFIG['allowed_paths'],
                    value=None,
                    interactive=True
                )
                remove_folder_btn = gr.Button("移除选中的文件夹", variant="stop") # (新)

                gr.Markdown("#### 3. 扫描") # (新)
                rescan_all_btn = gr.Button("🚀 重新扫描所有已添加的文件夹", variant="primary")

            with gr.Column(scale=1):
                gr.Markdown("#### 扫描状态") # (新)
                scan_progress_bar = gr.Slider(
                    minimum=0.0, maximum=1.0, step=0.01, value=0.0,
                    interactive=False, label="扫描进度" 
                )
                scan_progress_text = gr.Textbox(
                    label="当前任务状态", 
                    value="等待启动扫描...", 
                    interactive=False
                )
        
        gr.Markdown("---") # (新) 分隔符
        folder_msg_output = gr.Textbox(label="状态信息", interactive=False, lines=2) # (新) 移到下面
        managed_folders_display = gr.Textbox( # (新) 移到下面
            label="当前已管理的文件夹列表 (重启应用后生效)",
            value="\n".join(LOADED_CONFIG['allowed_paths']),
            lines=5,
            interactive=False
        )

    # --- 绑定 Gradio 事件 ---
    
    # --- 搜索页事件 ---
    
    search_btn.click(
        fn=search_images_wrapper,
        inputs=[
            cn_partial_input, cn_suggestion_dropdown, english_tag_input, 
            file_name_input, min_score_slider, max_score_slider,
            show_favorites_checkbox
        ],
        outputs=[
            image_gallery, search_msg_output, cn_suggestion_dropdown,
            current_results_state,
            current_gallery_state,
            current_selected_item_state # (新) 清空选中项
        ]
    )

    cn_partial_input.change(
        fn=get_cn_suggestions,
        inputs=[cn_partial_input],
        outputs=[cn_suggestion_dropdown, cn_suggestion_msg],
        queue=False 
    )
    
    cn_suggestion_dropdown.focus(
        fn=get_cn_suggestions,
        inputs=[cn_partial_input],
        outputs=[cn_suggestion_dropdown, cn_suggestion_msg],
        queue=False
    )
    
    image_gallery.select(
        fn=on_gallery_select,
        inputs=[current_results_state, current_gallery_state],
        outputs=[favorite_btn, tag_display_area, current_selected_item_state] # (新) 更新选中项
    )
    
    open_file_btn.click(
        fn=open_image_file,
        inputs=[current_selected_item_state], # (新) 更改输入
        outputs=None
    )
    
    favorite_btn.click(
        fn=on_favorite_button_click,
        inputs=[current_selected_item_state, current_results_state], # (新) 更改输入
        outputs=[favorite_btn, current_results_state, current_selected_item_state] # (新) 更新状态
    )

    tag_display_area.select(
        fn=on_tag_select_and_search,
        inputs=[
            tag_display_area,
            file_name_input,
            show_favorites_checkbox
        ],
        outputs=[
            cn_partial_input,
            cn_suggestion_dropdown,
            english_tag_input,
            file_name_input, 
            show_favorites_checkbox, 
            image_gallery,
            search_msg_output,
            current_results_state,
            current_gallery_state,
            current_selected_item_state # (新) 清空选中项
        ]
    )

    # --- 扫描页事件 ---
    if not SEARCH_ONLY_MODE:
        add_folder_btn.click(
            fn=add_folder_to_config,
            inputs=[scan_folder_input],
            outputs=[folder_msg_output, managed_folders_display, folder_to_remove_dd] # (新) 更新 output
        )
        
        # (新) 绑定移除按钮
        remove_folder_btn.click(
            fn=remove_folder_from_config,
            inputs=[folder_to_remove_dd],
            outputs=[folder_msg_output, managed_folders_display, folder_to_remove_dd]
        )
        
        rescan_all_btn.click(
            fn=start_rescan_all_folders_thread,
            inputs=None,
            outputs=[folder_msg_output]
        )
        
        # (新) 修复 Gradio 4.x 的 TypeError
        scan_timer = gr.Timer(1)
        scan_timer.tick(
            fn=check_scan_status, 
            inputs=None, 
            outputs=[scan_progress_bar, scan_progress_text]
        )

    # --- 页面加载事件 ---
    def on_demo_load(min_score, max_score):
        gallery, msg, dd, raw_results, gallery_state, sel_item = load_initial_gallery(min_score, max_score)
        return gallery, msg, dd, raw_results, gallery_state, sel_item

    demo.load(
        fn=on_demo_load,
        inputs=[min_score_slider, max_score_slider],
        outputs=[
            image_gallery, 
            search_msg_output, 
            cn_suggestion_dropdown,
            current_results_state,
            current_gallery_state,
            current_selected_item_state # (新) 清空选中项
        ]
    )

# --- 启动 Gradio 应用 ---
if __name__ == "__main__":
    demo.launch(allowed_paths=LOADED_CONFIG['allowed_paths'])