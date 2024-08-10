import subprocess
import os

def run_cloudflarest():
    try:
        # 切换工作目录到 ./cst 文件夹
        os.chdir("./cst")
        
        # 使用 subprocess 运行 ./CloudflareST
        result = subprocess.run(["./CloudflareST"], check=True, capture_output=True, text=True)
        print("CloudflareST 输出:")
        print(result.stdout)
        
        # 读取 result.csv 文件
        if os.path.exists("result.csv"):
            with open("result.csv", "r") as file:
                lines = file.readlines()

            # 去掉第一行标题，处理剩余行
            ips = []
            for index, line in enumerate(lines[1:], start=1):  # 从第二行开始，index从1开始
                ip = line.split(",")[0]
                ips.append(f"{ip} #{index}")

            # 将结果写入到父文件夹的 best_ips.txt
            with open("../best_ips.txt", "w") as output_file:
                for ip in ips:
                    output_file.write(ip + "\n")

            print("best_ips.txt 已生成并保存到父文件夹。")
        else:
            print("result.csv 文件不存在，无法处理。")
    except subprocess.CalledProcessError as e:
        print("运行 CloudflareST 时出错:")
        print(e.stderr)
    except FileNotFoundError:
        print("找不到 ./CloudflareST 文件，请确认它存在并可执行。")
    except Exception as e:
        print(f"发生未知错误: {e}")

if __name__ == "__main__":
    run_cloudflarest()

