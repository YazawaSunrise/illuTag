import os
import numpy as np
from typing import List, Dict, Tuple
from PIL import Image

# 尝试导入 TensorFlow/Keras
try:
    import tensorflow as tf
    from tensorflow.keras.models import load_model
except ImportError:
    # 如果 TensorFlow 不可用，则抛出错误，因为用户已声明安装了依赖且不需要模拟
    raise ImportError("错误: TensorFlow 未安装。请运行 'pip install tensorflow' 以继续。")

# --- 配置常量 ---
MODEL_PATH = "model-resnet_custom_v3.h5"
TAGS_FILE = "tags.txt"
SCORE_THRESHOLD = 0.5
# 修正模型输入尺寸以匹配错误信息中的要求 (512x512)
IMAGE_SIZE = (512, 512)


class TagProcessor:
    """
    illuTag 项目的标签处理核心。
    负责加载 DeepDanbooru 模型，对图片进行打标，并按分数阈值筛选结果。
    """
    def __init__(self, threshold: float = SCORE_THRESHOLD):
        """
        初始化标签处理器，加载模型和标签列表。
        """
        self.threshold = threshold
        self.tags = self._load_tags()
        self.model = self._load_model()
        
        # 检查关键组件是否加载成功
        if not self.model or not self.tags:
             raise RuntimeError("TagProcessor 初始化失败：模型或标签列表加载失败。")

        # 最终验证模型和标签数量是否匹配
        if self.model.output_shape[1] != len(self.tags):
            print(f"警告: 模型输出维度 ({self.model.output_shape[1]}) 与标签数量 ({len(self.tags)}) 不匹配！请检查 {TAGS_FILE}。")
            
        print(f"标签处理器初始化完成，模型已加载，分数筛选阈值: {self.threshold}")

    def _load_model(self):
        """加载 Keras 模型。如果失败，则返回 None 并打印错误。"""
        try:
            # 禁止 Keras 在加载时打印进度条
            model = load_model(MODEL_PATH, compile=False) 
            print(f"成功加载模型: {MODEL_PATH}")
            return model
        except Exception as e:
            print(f"致命错误: 无法加载模型 {MODEL_PATH}。请检查文件路径和格式。错误详情: {e}")
            return None

    def _load_tags(self) -> List[str]:
        """加载标签列表。如果失败，则返回空列表并打印错误。"""
        if os.path.exists(TAGS_FILE):
            try:
                with open(TAGS_FILE, 'r', encoding='utf-8') as f:
                    return [line.strip() for line in f if line.strip()]
            except Exception as e:
                print(f"致命错误: 无法加载标签文件 {TAGS_FILE}。请检查文件内容和编码。错误详情: {e}")
                return []
        else:
            print(f"致命错误: 找不到标签文件 {TAGS_FILE}。请提供与模型对应的真实标签列表。")
            return []

    def _preprocess_image(self, image_path: str) -> np.ndarray | None:
        """
        加载、预处理图片以符合模型输入要求 (512x512, 归一化)。
        """
        try:
            img = Image.open(image_path).convert('RGB')
            # 修正尺寸为 512x512
            img = img.resize(IMAGE_SIZE) 
            img_array = np.asarray(img, dtype=np.float32)
            
            # 标准化 (0-1 归一化)
            img_array /= 255.0 
            
            # 添加 Batch 维度 (1, H, W, C)
            return np.expand_dims(img_array, axis=0) 
        except Exception as e:
            print(f"图片预处理失败: {image_path}。错误: {e}")
            return None

    def _perform_danbooru_prediction(self, image_path: str) -> List[Tuple[str, float]]:
        """
        执行 DeepDanbooru 模型的预测。
        """
        if not self.model: # 再次检查模型是否加载
            return []
            
        processed_image = self._preprocess_image(image_path)
        
        if processed_image is None:
            return []

        # 执行预测 (verbose=0 避免打印进度条)
        try:
            predictions = self.model.predict(processed_image, verbose=0)[0]
        except Exception as e:
            print(f"模型预测失败: {image_path}。错误: {e}")
            return []
        
        # 验证模型输出维度
        if len(predictions) != len(self.tags):
            # 这个检查在 __init__ 中已经有了警告，这里仅作保护性检查
            return []
            
        results = []
        for tag_name, score in zip(self.tags, predictions):
            results.append((tag_name, float(score)))

        return results

    def process_image(self, image_path: str) -> Tuple[str, List[Dict]]:
        """
        处理单张图片，进行打标和分数筛选。
        """
        if not os.path.exists(image_path):
            print(f"错误: 找不到文件 {image_path}")
            return image_path, []

        # 1. 执行标签预测
        raw_predictions = self._perform_danbooru_prediction(image_path)

        # 2. 应用分数筛选逻辑
        filtered_tags: List[Dict] = []
        for tag_name, score in raw_predictions:
            if score >= self.threshold:
                # 只保留分数高于阈值的标签
                filtered_tags.append({
                    "tag_name": tag_name,
                    "score": round(score, 4)
                })

        print(f"--- 处理图片: {os.path.basename(image_path)} ---")
        print(f"筛选后标签数: {len(filtered_tags)}")

        # 3. 返回结构化数据 (文件路径, 标签列表)
        return image_path, filtered_tags