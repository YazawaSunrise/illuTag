import pandas as pd
import os
from typing import List, Dict, Set, Optional

# --- 配置常量 ---
DICTIONARY_FILE = "dictionary01.xlsx"

class DictionaryManager:
    """
    负责加载中文标签词典（dictionary01.xlsx），并提供中文到英文标签的模糊联想和映射查询。
    
    仅使用 Excel 中的 'tag' (C列, 索引2) 和 'right_tag_cn' (D列, 索引3)。
    
    映射结构：
    - 精确中文标签 (D列) -> 对应的单个英文标签 (C列)
    - 所有精确中文标签列表 (用于模糊搜索)
    - (新) 英文标签 (C列) -> 对应的单个中文标签 (D列)
    """
    def __init__(self, dict_path: str = DICTIONARY_FILE):
        self.dict_path = dict_path
        self._df: Optional[pd.DataFrame] = None
        
        # CN Tag (exact) -> Corresponding EN Tag (single)
        self._cn_to_en_tag: Dict[str, str] = {} 
        
        # (新) EN Tag -> Corresponding CN Tag (single)
        self._en_to_cn_tag: Dict[str, str] = {}
        
        # 所有在词典中出现的、用于模糊搜索的精确中文标签
        self._all_cn_tags: List[str] = []
        
        self._load_dictionary()

    def _load_dictionary(self):
        """
        加载 Excel 词典文件，并构建映射结构。
        """
        if not os.path.exists(self.dict_path):
            print(f"警告: 找不到词典文件 {self.dict_path}。中文搜索功能将不可用。")
            return
            
        try:
            # 只读取 C (索引 2) 和 D (索引 3) 列
            # 注意: header=None 从第 0 行开始读取
            self._df = pd.read_excel(self.dict_path, header=None, sheet_name=0, usecols=[2, 3])
            
            # 重命名列
            self._df.rename(columns={2: 'tag', 3: 'right_tag_cn'}, inplace=True)
            
            # 清理数据
            self._df.dropna(subset=['tag', 'right_tag_cn'], inplace=True)
            self._df['tag'] = self._df['tag'].astype(str).str.strip().str.lower()
            self._df['right_tag_cn'] = self._df['right_tag_cn'].astype(str).str.strip()
            
            # 构建 CN -> EN 和 EN -> CN 映射
            for cn_tag, en_tag in zip(self._df['right_tag_cn'], self._df['tag']):
                # 如果有重复，后面的会覆盖前面的
                self._cn_to_en_tag[cn_tag] = en_tag
                
                # (新) 构建反向映射
                # 注意：如果一个 EN 标签对应多个 CN 翻译，这里只会保留最后一个
                self._en_to_cn_tag[en_tag] = cn_tag

            # 构建所有精确中文标签列表 (用于后续的模糊搜索)
            self._all_cn_tags = list(self._cn_to_en_tag.keys())


            print(f"成功加载词典 {self.dict_path}，包含 {len(self._all_cn_tags)} 个精确中文标签。")
            
        except ImportError:
            print("致命错误: 无法导入 'pandas' 或 'openpyxl' 库。请安装：'pip install pandas openpyxl'")
        except Exception as e:
            print(f"加载词典文件失败: {e}")
            
    def fuzzy_lookup_suggestions(self, partial_cn_term: str, allowed_en_tags: Optional[Set[str]] = None) -> List[str]:
        """
        根据部分中文输入，模糊搜索所有包含该词的完整中文标签 (联想词)。
        
        Args:
            partial_cn_term: 用户输入的中文子串。
            allowed_en_tags: 数据库中已有的英文标签集合。如果提供，则只返回映射到
                               这些标签的中文词（实现联想词过滤）。
        
        返回的列表是精确的中文标签。
        """
        partial_cn_term = partial_cn_term.strip()
        if not partial_cn_term:
            return []
            
        # 1. 执行原始模糊匹配
        raw_suggestions = []
        for cn_tag in self._all_cn_tags:
            if partial_cn_term in cn_tag:
                raw_suggestions.append(cn_tag)
                
        # 2. 如果提供了 allowed_en_tags，则进行过滤 (Feature 1)
        if allowed_en_tags is None:
            # 未提供过滤集，返回原始结果 (限制数量)
            return raw_suggestions[:200]
        
        filtered_suggestions = []
        for cn_tag in raw_suggestions:
            en_tag = self._cn_to_en_tag.get(cn_tag)
            # 检查这个中文标签对应的英文标签是否在允许的集合中
            if en_tag and en_tag in allowed_en_tags:
                filtered_suggestions.append(cn_tag)
                
        # 限制返回的数量以防止列表过长
        return filtered_suggestions[:200]

    def get_search_tags_from_cn_list(self, cn_terms: List[str]) -> List[str]:
        """
        给定一个精确中文标签列表，返回所有关联的英文标签的集合（去重）。
        """
        final_en_tags: Set[str] = set()
        for cn_term in cn_terms:
            # 使用精确查找，并将单个英文标签添加到集合中
            en_tag = self._cn_to_en_tag.get(cn_term)
            if en_tag:
                final_en_tags.add(en_tag)
            
        return list(final_en_tags)

    # --- (新) 新增辅助函数 ---

    def lookup_en_to_cn(self, en_tag: str) -> Optional[str]:
        """
        (新) 尝试将英文标签翻译为中文。
        """
        return self._en_to_cn_tag.get(en_tag)

    def is_cn_tag(self, cn_tag: str) -> bool:
        """
        (新) 检查一个标签是否是词典中已知的中文标签。
        """
        return cn_tag in self._cn_to_en_tag