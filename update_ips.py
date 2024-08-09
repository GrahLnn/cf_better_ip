import requests

# 获取数据
response = requests.get('https://ip.164746.xyz/ipTop.html')
response.raise_for_status()
data = response.text.strip()

# 处理数据
ips = data.split(',')
alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
formatted_ips = [f"{ip}  #{alphabet[i % len(alphabet)]} {".".join(ip.split(".")[-2:])}\n" for i, ip in enumerate(ips)]

# 写入文件
with open('best_ips.txt', 'w') as f:
    f.writelines(formatted_ips)
