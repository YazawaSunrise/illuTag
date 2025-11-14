import sqlite3
import os
from typing import List, Dict, Tuple, Optional, Set 
from datetime import datetime

# --- 配置常量 ---
DATABASE_FILE = "illutag_data.db"
IMAGE_TABLE = "images"
TAGS_TABLE = "tags"

class DatabaseManager:
    """
    illuTag 项目的数据库管理核心。
    负责连接 SQLite 数据库、初始化表结构、以及处理数据的存储和检索。
    """
    def __init__(self, db_path: str = DATABASE_FILE):
        """
        初始化数据库管理器，并连接到指定的 SQLite 文件。
        """
        self.db_path = db_path
        self._initialize_db_structure() # 确保在应用启动时创建表结构

    def _get_connection(self) -> sqlite3.Connection:
        """
        [线程安全] 创建一个新的数据库连接。
        """
        conn = sqlite3.connect(self.db_path)
        conn.row_factory = sqlite3.Row
        return conn

    def _initialize_db_structure(self):
        """
        连接数据库并创建必要的表结构。
        此方法在应用启动时运行，确保表结构存在。
        """
        conn = None
        try:
            conn = self._get_connection()
            cursor = conn.cursor()
            
            # 1. 创建 Images 表
            cursor.execute(f"""
                CREATE TABLE IF NOT EXISTS {IMAGE_TABLE} (
                    image_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_path TEXT UNIQUE NOT NULL,
                    date_scanned TEXT,
                    is_favorite INTEGER DEFAULT 0
                );
            """)

            # --- (新) 安全地添加新列 (用于现有用户) ---
            try:
                cursor.execute(f"ALTER TABLE {IMAGE_TABLE} ADD COLUMN is_favorite INTEGER DEFAULT 0")
                print("数据库更新：已成功添加 'is_favorite' 列。")
            except sqlite3.OperationalError:
                # 列已存在，忽略错误
                pass
            # --- 更新结束 ---
            
            # 2. 创建 Tags 表 (包含对 Images 表的外键引用)
            cursor.execute(f"""
                CREATE TABLE IF NOT EXISTS {TAGS_TABLE} (
                    tag_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    image_id INTEGER NOT NULL,
                    tag_name TEXT NOT NULL,
                    score REAL NOT NULL,
                    FOREIGN KEY (image_id) REFERENCES {IMAGE_TABLE}(image_id)
                );
            """)
            
            # 创建索引，用于加速标签搜索
            cursor.execute(f"CREATE INDEX IF NOT EXISTS idx_tag_name ON {TAGS_TABLE} (tag_name);")
            cursor.execute(f"CREATE INDEX IF NOT EXISTS idx_score ON {TAGS_TABLE} (score);")
            
            conn.commit()
            print(f"数据库结构初始化成功，文件: {self.db_path}")

        except sqlite3.Error as e:
            print(f"数据库结构初始化失败: {e}")
        finally:
            if conn:
                conn.close()

    def close(self):
        """
        移除 close 方法中关闭连接的逻辑，因为连接现在是按需创建和关闭的。
        """
        print("DatabaseManager 关闭完成 (连接已按需管理)。")

    def get_all_indexed_file_paths(self) -> Set[str]:
        """
        [线程安全] 获取数据库中所有已索引图片的完整文件路径集合。
        用于在扫描时判断文件是否需要跳过。
        """
        conn = None
        try:
            conn = self._get_connection()
            cursor = conn.cursor()

            sql_query = f"""
                SELECT file_path FROM {IMAGE_TABLE};
            """

            cursor.execute(sql_query)
            results = cursor.fetchall()

            # 规范化路径，确保与文件系统读取的路径格式一致，便于 Set 查找
            return {os.path.normpath(row['file_path']) for row in results}

        except sqlite3.Error as e:
            print(f"获取所有索引文件路径失败: {e}")
            return set()
        finally:
            if conn: conn.close()
            
    def save_tags_to_db(self, file_path: str, tags: List[Dict]) -> bool:
        """
        [线程安全] 将单张图片的标签数据保存到数据库。
        """
        conn = None
        try:
            conn = self._get_connection()
            cursor = conn.cursor()
            
            # 开启事务
            conn.execute("BEGIN TRANSACTION;")

            # 1. 插入或更新 Images 表 (获取 image_id)
            now = datetime.now().isoformat()
            
            # 使用 os.path.normpath 确保数据库中存储的路径格式一致
            normalized_path = os.path.normpath(file_path)
            
            cursor.execute(f"""
                INSERT INTO {IMAGE_TABLE} (file_path, date_scanned)
                VALUES (?, ?)
                ON CONFLICT(file_path) DO UPDATE SET date_scanned=excluded.date_scanned
            """, (normalized_path, now))

            # 获取插入或更新后的 image_id
            cursor.execute(f"SELECT image_id FROM {IMAGE_TABLE} WHERE file_path = ?", (normalized_path,))
            image_id = cursor.fetchone()[0]

            # 2. 删除该图片所有旧的标签记录 (处理重新扫描)
            cursor.execute(f"DELETE FROM {TAGS_TABLE} WHERE image_id = ?", (image_id,))
            
            # 3. 插入新的标签记录
            tag_data = [(image_id, tag['tag_name'], tag['score']) for tag in tags]
            if tag_data:
                cursor.executemany(f"""
                    INSERT INTO {TAGS_TABLE} (image_id, tag_name, score)
                    VALUES (?, ?, ?)
                """, tag_data)

            conn.commit()
            return True
            
        except sqlite3.Error as e:
            if conn: conn.rollback()
            print(f"保存数据失败 (文件: {file_path}): {e}")
            return False
        finally:
            if conn: conn.close()
            
    def get_all_indexed_tags(self) -> Set[str]:
        """
        [线程安全] 获取数据库中所有图片使用的唯一英文标签集合。
        """
        conn = None
        try:
            conn = self._get_connection()
            cursor = conn.cursor()
            
            sql_query = f"""
                SELECT DISTINCT tag_name FROM {TAGS_TABLE};
            """
            
            cursor.execute(sql_query)
            results = cursor.fetchall()
            
            return {row['tag_name'] for row in results}

        except sqlite3.Error as e:
            print(f"获取所有索引标签失败: {e}")
            return set()
        finally:
            if conn: conn.close()

    def get_all_indexed_images(self) -> List[Dict]:
        """
        [线程安全] 获取数据库中所有图片及其所有标签信息。
        (新) 现在也获取 'is_favorite' 状态。
        """
        conn = None
        try:
            conn = self._get_connection()
            cursor = conn.cursor()
            
            # 核心 SQL 逻辑：查询所有图片及其标签
            sql_query = f"""
                SELECT 
                    T1.image_id, 
                    T2.file_path,
                    T2.is_favorite,
                    T1.tag_name,
                    T1.score
                FROM {TAGS_TABLE} AS T1
                INNER JOIN {IMAGE_TABLE} AS T2 ON T1.image_id = T2.image_id
                ORDER BY T2.image_id, T1.score DESC
            """
            
            cursor.execute(sql_query)
            results = cursor.fetchall()
            
            # 将扁平化的结果重新整理为图片-标签结构 (与 search_images 相同的输出格式)
            image_results: Dict[int, Dict] = {}
            for row in results:
                image_id = row['image_id']
                if image_id not in image_results:
                    image_results[image_id] = {
                        "image_id": image_id,
                        # 注意：此处从数据库取出的是规范化路径
                        "file_path": row['file_path'], 
                        "is_favorite": bool(row['is_favorite']), # (新) 添加收藏状态
                        "tags": []
                    }
                image_results[image_id]['tags'].append({
                    "tag_name": row['tag_name'],
                    "score": row['score']
                })
            
            # 返回一个列表
            return list(image_results.values())

        except sqlite3.Error as e:
            print(f"获取所有索引图片失败: {e}")
            return []
        finally:
            if conn: conn.close()


    def search_images(self, search_tags: List[str], min_score: float) -> List[Dict]:
        """
        [线程安全] 根据多个标签和最低分数阈值搜索图片。
        """
        if not search_tags:
            # 如果没有搜索标签，返回空列表，将逻辑交给 app.py 的 wrapper
            return []
            
        conn = None
        try:
            conn = self._get_connection()
            cursor = conn.cursor()
            
            # SQL 查询参数
            tag_placeholders = ','.join(['?'] * len(search_tags))
            
            # 核心 SQL 逻辑：查询包含任一标签且分数满足条件的图片
            sql_query = f"""
                SELECT 
                    T1.image_id, 
                    T2.file_path,
                    T2.is_favorite,
                    T1.tag_name,
                    T1.score
                FROM {TAGS_TABLE} AS T1
                INNER JOIN {IMAGE_TABLE} AS T2 ON T1.image_id = T2.image_id
                WHERE 
                    T1.tag_name IN ({tag_placeholders}) -- OR 逻辑：匹配任一标签
                    AND T1.score >= ?
                ORDER BY T1.image_id, T1.tag_name
            """
            
            # 参数列表：[搜索标签] + [最低分]
            params = search_tags + [min_score]

            # 执行查询
            cursor.execute(sql_query, params)
            results = cursor.fetchall()
            
            # 将扁平化的结果重新整理为图片-标签结构
            image_results: Dict[int, Dict] = {}
            for row in results:
                image_id = row['image_id']
                if image_id not in image_results:
                    image_results[image_id] = {
                        "image_id": image_id,
                        "file_path": row['file_path'],
                        "is_favorite": bool(row['is_favorite']), # (新) 添加收藏状态
                        "tags": []
                    }
                image_results[image_id]['tags'].append({
                    "tag_name": row['tag_name'],
                    "score": row['score']
                })
            
            # 返回一个列表
            return list(image_results.values())

        except sqlite3.Error as e:
            print(f"数据库搜索失败: {e}")
            return []
        finally:
            if conn: conn.close()

    # --- (新) 收藏功能 ---
    def toggle_favorite_status(self, image_id: int) -> bool:
        """
        [线程安全] 切换一张图片的收藏状态 (0 -> 1 或 1 -> 0)。
        返回切换后的新状态 (True 表示已收藏)。
        """
        conn = None
        try:
            conn = self._get_connection()
            cursor = conn.cursor()
            
            # 1. 切换状态 (使用 1 - is_favorite)
            cursor.execute(f"""
                UPDATE {IMAGE_TABLE}
                SET is_favorite = 1 - is_favorite
                WHERE image_id = ?
            """, (image_id,))
            
            # 2. 获取新状态
            cursor.execute(f"SELECT is_favorite FROM {IMAGE_TABLE} WHERE image_id = ?", (image_id,))
            new_status = cursor.fetchone()
            
            conn.commit()
            
            if new_status:
                return bool(new_status['is_favorite'])
            return False

        except sqlite3.Error as e:
            if conn: conn.rollback()
            print(f"切换收藏状态失败 (Image ID: {image_id}): {e}")
            return False
        finally:
            if conn: conn.close()