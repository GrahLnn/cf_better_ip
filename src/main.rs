use futures::stream::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use rand::seq::SliceRandom;
use reqwest::{Client, Error, Proxy};
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::time::{sleep, Duration};

async fn download_file(url: &str, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !Path::new(filename).exists() {
        println!("从服务器下载 {}", filename);

        // 创建所有父文件夹
        if let Some(parent) = Path::new(filename).parent() {
            std::fs::create_dir_all(&parent)?;
        }

        let client = Client::new();
        let response = client.get(url).send().await?;
        let bytes = response.bytes().await?;
        let mut file = File::create(filename)?;
        file.write_all(&bytes)?;
    }
    Ok(())
}

async fn check_and_download_files() -> Result<(), Box<dyn std::error::Error>> {
    let urls = [
        (
            "https://www.baipiao.eu.org/cloudflare/colo",
            "asset/colo.txt",
        ),
        ("https://www.baipiao.eu.org/cloudflare/url", "asset/url.txt"),
        (
            "https://www.baipiao.eu.org/cloudflare/ips-v4",
            "asset/ips-v4.txt",
        ),
        (
            "https://www.baipiao.eu.org/cloudflare/ips-v6",
            "asset/ips-v6.txt",
        ),
    ];

    for (url, filename) in &urls {
        let mut attempts = 0;
        while attempts < 3 {
            match download_file(url, filename).await {
                Ok(_) => break,
                Err(e) => {
                    attempts += 1;
                    println!("下载 {} 失败: {}，重试({}/3)", filename, e, attempts);
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
    Ok(())
}

fn process_url_file() -> io::Result<(String, String)> {
    let path = "asset/url.txt";
    if let Ok(lines) = read_lines(path) {
        if let Some(Ok(line)) = lines.into_iter().next() {
            let parts: Vec<&str> = line.split('/').collect();
            let domain = parts[0].to_string();
            let file = parts[1..].join("/");
            return Ok((domain, file));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "文件为空或无法读取",
    ))
}

fn process_ips_file() -> io::Result<Vec<String>> {
    let path = "asset/ips-v4.txt";
    let mut ips = Vec::new();
    if let Ok(lines) = read_lines(path) {
        for line in lines {
            if let Ok(ip) = line {
                let parts: Vec<&str> = ip.split('/').collect();
                ips.push(parts[0].to_string());
            }
        }
    }
    Ok(ips)
}

// Helper function to read lines from a file
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

async fn measure_latency(domain: &str, ip: &str) -> Result<f64, Box<dyn std::error::Error>> {
    let url = format!("http://{}/cdn-cgi/trace", domain);
    let proxy_url = format!("http://{}:80", ip);
    let proxy = Proxy::http(&proxy_url)?;
    let client = Client::builder().proxy(proxy).build()?;

    let mut total_duration = 0.0;
    for _ in 0..3 {
        let start = Instant::now();
        let resp = client.get(&url).send().await?;
        resp.text().await?;
        let duration = start.elapsed().as_millis() as f64; // 转换为毫秒
        total_duration += duration;
    }
    let avg_duration = total_duration / 3.0;
    Ok((avg_duration * 100.0).round() / 100.0) // 保留两位小数
}

async fn test_speed(ip: &str, domain: &str, file: &str) -> Result<f64, reqwest::Error> {
    let latency = measure_latency(domain, ip).await.unwrap_or(0.0);

    if latency > 1000.0 || latency == 0.0 {
        return Ok(0.0);
    }
    let proxy = format!("http://{}:80", ip);
    let url = format!("http://{}/{}", domain, file);
    let client = Client::builder()
        .proxy(reqwest::Proxy::http(&proxy)?)
        .build()?;

    let start_time = Instant::now();
    let response = client.get(&url).send().await?;
    let duration = start_time.elapsed();

    let content_length = response.content_length().unwrap_or(0);
    let speed = (content_length as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64();

    Ok(speed)
}

async fn get_ip_location(ip: &str) -> Result<String, Error> {
    let request_url = format!("http://ip-api.com/json/{}", ip);
    let response = reqwest::get(&request_url).await;

    match response {
        Ok(resp) => {
            let location: serde_json::Value = resp.json().await?;
            Ok(location["countryCode"].to_string())
        }
        Err(_) => {
            // 使用第一个备用服务
            let backup_request_url = format!("http://ipinfo.io/{}/json", ip);
            let backup_response = reqwest::get(&backup_request_url).await;

            match backup_response {
                Ok(resp) => {
                    let location: serde_json::Value = resp.json().await?;
                    Ok(location["country"].to_string())
                }
                Err(_) => {
                    // 使用第二个备用服务
                    let second_backup_request_url = format!("https://freegeoip.app/json/{}", ip);
                    let second_backup_response = reqwest::get(&second_backup_request_url).await?;
                    let location: serde_json::Value = second_backup_response.json().await?;
                    Ok(location["country_code"].to_string())
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    check_and_download_files().await?;

    let (domain, file) = process_url_file()?;

    let mut ips = process_ips_file()?;
    ips.shuffle(&mut rand::thread_rng());

    let pb = ProgressBar::new(ips.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )?
            .progress_chars("#>-"),
    );

    let file = Arc::new(file);
    let res = Arc::new(Mutex::new(Vec::new()));
    let tasks = ips.into_iter().map(|ip| {
        let domain = domain.clone();
        let pb = pb.clone();
        let file = Arc::clone(&file);
        let res = Arc::clone(&res);
        tokio::spawn(async move {
            let speed = test_speed(&ip, &domain, &*file).await.unwrap_or(0.0);
            pb.inc(1);
            if speed > 30.00 {
                match get_ip_location(&ip).await {
                    Ok(location) => {
                        // println!(
                        //     "IP: {}, 速度: {:.2} MB/s, Location: {}",
                        //     ip, speed, location
                        // );
                        let mut res = res.lock().unwrap();
                        // res.push(format!("{}:80#{}{:.0}", ip, location, speed));
                        res.push((ip, location, speed));
                    }
                    Err(e) => println!("Failed to get location: {}", e),
                }
            }
        })
    });
    futures::stream::iter(tasks)
        .for_each_concurrent(200, |task| async {
            task.await.unwrap();
        })
        .await;

    let mut res = Arc::try_unwrap(res).unwrap().into_inner().unwrap();
    res.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    let res: Vec<String> = res
        .iter()
        .map(|(ip, location, speed)| format!("{}:80#{}{:.0}", ip, location, speed))
        .collect();
    let data = res.join("\n");
    std::fs::write("best_ips.txt", data).expect("Unable to write file");
    Ok(())
}
