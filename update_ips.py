import requests

# 获取第一个数据源
response = requests.get('https://ip.164746.xyz/ipTop.html')
response.raise_for_status()
data = response.text.strip()

# 处理第一个数据源
ips = data.split(',')
alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
formatted_ips = [f"{ip}  #{alphabet[i % len(alphabet)]} {'.'.join(ip.split('.')[-2:])}\n" for i, ip in enumerate(ips)]

# 获取GitHub上的CloudFlare优质IP
github_response = requests.get('https://raw.githubusercontent.com/Alvin9999/new-pac/master/CloudFlare%E4%BC%98%E8%B4%A8IP')
github_response.raise_for_status()
github_data = github_response.text

# 处理GitHub数据
github_ips = [line.strip() for line in github_data.split('\n') if line.strip()]

# 写入文件
with open('best_ips.txt', 'w', encoding='utf-8') as f:
    f.writelines(formatted_ips)
    f.writelines(f"{ip}\n" for ip in github_ips)