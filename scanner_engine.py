import os
import threading
from typing import List, Dict, Optional
import time

# 导入依赖模块
from tag_processor import TagProcessor
from database_manager import DatabaseManager # 需要 DatabaseManager 实例

class ScanEngine:
    """
    负责管理文件系统扫描、调用标签处理和数据库存储的引擎。
    使用线程锁确保多线程访问状态时的安全。
    """
    
    # 支持的图片文件扩展名
    SUPPORTED_EXTENSIONS = ('.jpg', '.jpeg', '.png', '.gif')

    def __init__(self, processor: TagProcessor, db_manager: DatabaseManager):
        self.processor = processor
        self.db_manager = db_manager
        self.lock = threading.Lock()
        self.status = {
            "is_scanning": False,
            "total_files": 0,
            "files_processed": 0,
            "progress_percent": 0,
            "folder": ""
        }

    def get_status(self) -> Dict:
        """返回当前的扫描状态。"""
        with self.lock:
            return self.status.copy()
            
    def start_scan(self, folder_path: str, progress_callback=None, force_rescan: bool = False):
        """
        启动扫描过程。在一个单独的线程中运行。
        现在会根据 force_rescan 参数来决定是否跳过数据库中已存在的索引文件。
        
        Args:
            folder_path: 要扫描的文件夹路径。
            progress_callback: 进度更新回调函数（在此应用中未使用）。
            force_rescan: 如果为 True，则不跳过任何文件，强制重新打标和存储。
        """
        with self.lock:
            if self.status["is_scanning"]:
                print("扫描正在进行中，跳过新的启动请求。")
                return

            self.status["is_scanning"] = True
            self.status["total_files"] = 0
            self.status["files_processed"] = 0
            self.status["progress_percent"] = 0
            self.status["folder"] = folder_path

        print(f"扫描引擎启动: {folder_path} (强制重新扫描: {force_rescan})")

        if not os.path.isdir(folder_path):
            with self.lock:
                self.status["is_scanning"] = False
            print(f"错误: 路径 '{folder_path}' 无效或不存在。")
            return

        try:
            # 1. 获取已扫描文件列表 (或为空集)
            if force_rescan:
                already_indexed_paths = set()
                print("执行强制重新扫描，将不会跳过任何现有文件。")
            else:
                print("正在检查数据库中已存在的索引文件...")
                already_indexed_paths = self.db_manager.get_all_indexed_file_paths()
                print(f"找到 {len(already_indexed_paths)} 个已索引文件。")
            
            files_to_scan: List[str] = []
            all_files_in_folder: List[str] = [] # 用于计算总数
            
            # 2. 第一次遍历：筛选出需要扫描的新文件
            for root, _, files in os.walk(folder_path):
                for file in files:
                    if file.lower().endswith(self.SUPPORTED_EXTENSIONS):
                        full_path = os.path.join(root, file)
                        normalized_path = os.path.normpath(full_path) 
                        all_files_in_folder.append(normalized_path)
                        
                        # 只有在非强制重扫模式下才跳过
                        if not force_rescan and normalized_path in already_indexed_paths:
                            continue # 跳过已索引文件
                        
                        files_to_scan.append(normalized_path)
            
            # 3. 设置初始状态和总文件数
            with self.lock:
                # 总文件数 = 文件夹中所有文件
                self.status["total_files"] = len(all_files_in_folder)
                
                if force_rescan:
                    # 强制重扫模式下，从 0 开始处理
                    self.status["files_processed"] = 0
                    files_to_process = len(all_files_in_folder)
                    print(f"强制重新扫描：总共 {files_to_process} 个文件需要处理。")
                else:
                    # 增量扫描模式下，从已索引的文件数开始
                    self.status["files_processed"] = len(already_indexed_paths)
                    files_to_process = len(files_to_scan) # 仅遍历这些文件
                    print(f"增量扫描：已索引 {len(already_indexed_paths)} 个文件，新增 {files_to_process} 个文件需要处理。")
                
                # 更新进度百分比
                if self.status["total_files"] > 0:
                     self.status["progress_percent"] = int((self.status["files_processed"] / self.status["total_files"]) * 100)
                else:
                     self.status["progress_percent"] = 0
                     
            total_files = self.status["total_files"] # 获取最新的总数
            
            # 4. 实际扫描未索引的新文件 (或所有文件，如果是强制重扫)
            for i, file_path in enumerate(files_to_scan):
                
                # a. 调用标签处理
                print(f"[{i+1}/{files_to_process}] 正在打标: {os.path.basename(file_path)}")
                
                # 尝试打标，如果失败则跳过
                try:
                    # --- 修复开始 ---
                    # 错误: process_image 返回 (image_path, tags_list) 元组
                    # tags = self.processor.process_image(file_path)
                    
                    # 修正: 正确解包元组，我们只关心标签列表
                    _ , tags_list = self.processor.process_image(file_path)
                    
                    # b. 存储到数据库 (DBManager 负责处理冲突和更新)
                    # 修正: 使用 tags_list 变量
                    if tags_list and self.db_manager.save_tags_to_db(file_path, tags_list):
                        print(f"  -> 成功保存 {len(tags_list)} 个标签。")
                    else:
                        print(f"  -> 数据库保存失败或未打到标签，跳过文件。")
                    # --- 修复结束 ---
                        
                except Exception as e:
                    print(f"打标或保存文件 {os.path.basename(file_path)} 失败: {e}")
                    
                
                # c. 更新进度
                with self.lock:
                    self.status["files_processed"] += 1
                    processed = self.status["files_processed"]
                    
                    if total_files > 0:
                        self.status["progress_percent"] = int((processed / total_files) * 100)
                    else:
                        self.status["progress_percent"] = 100 
                        
                # 如果提供了回调函数，则调用它
                if progress_callback:
                    progress_callback(self.get_status())
                
                # 模拟 IO 延时，避免 CPU 占用过高
                # time.sleep(0.01) 

        except Exception as e:
            print(f"扫描引擎运行时发生未知错误: {e}")
        finally:
            # 5. 扫描结束，更新最终状态
            with self.lock:
                self.status["is_scanning"] = False
                
                # 确保在扫描结束后，如果 total > 0，进度达到 100%
                if self.status["total_files"] > 0:
                    self.status["files_processed"] = self.status["total_files"]
                    self.status["progress_percent"] = 100
                elif self.status["total_files"] == 0:
                    self.status["files_processed"] = 0
                    self.status["progress_percent"] = 0

            print(f"扫描完成，总共处理了 {self.status['files_processed']} 个文件。")