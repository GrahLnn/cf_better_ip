import subprocess
import os
from datetime import datetime
import time

def run_ta_script():
    # 运行 ta.py
    subprocess.run(["python", "ta.py"], check=True)

def git_commit_and_push():
    # 获取当前时间作为提交消息
    current_time = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    commit_message = f"Automated commit at {current_time}"

    # Git 命令来添加更改并提交
    subprocess.run(["git", "add", "."], check=True)
    subprocess.run(["git", "commit", "-m", commit_message], check=True)
    subprocess.run(["git", "push", "origin", "main"], check=True)

if __name__ == "__main__":
    while True:
        run_ta_script()
        git_commit_and_push()
        
        # 等待24小时（86400秒）
        time.sleep(3600)

