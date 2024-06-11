use futures::stream::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use rand::seq::SliceRandom;
use reqwest::{Client, Proxy};
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
    let client = Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(10))
        .build()?;

    let mut total_duration = 0.0;
    for _ in 0..10 {
        let start = Instant::now();
        let resp = client.get(&url).send().await?;
        resp.text().await?;
        let duration = start.elapsed().as_millis() as f64;
        total_duration += duration;
        sleep(Duration::from_secs(2)).await; // 间隔测试时间
    }
    let avg_duration = total_duration / 5.0;
    Ok((avg_duration * 100.0).round() / 100.0)
}

async fn test_speed(ip: &str, domain: &str, file: &str) -> Result<f64, reqwest::Error> {
    let latency = measure_latency(domain, ip).await.unwrap_or(0.0);

    if latency > 500.0 || latency == 0.0 {
        return Ok(0.0);
    }
    let proxy = format!("http://{}:80", ip);
    let url = format!("http://{}/{}", domain, file);
    let client = Client::builder()
        .proxy(reqwest::Proxy::http(&proxy)?)
        .timeout(Duration::from_secs(30))
        .build()?;

    let start_time = Instant::now();
    let response = client.get(&url).send().await?;
    let duration = start_time.elapsed();

    let content_length = response.content_length().unwrap_or(0);
    let speed = (content_length as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64();

    Ok(speed)
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
            if speed > 500.00 {
                let mut res = res.lock().unwrap();
                res.push((ip, speed));
            }
        })
    });
    futures::stream::iter(tasks)
        .for_each_concurrent(500, |task| async {
            task.await.unwrap();
        })
        .await;

    let mut res = Arc::try_unwrap(res).unwrap().into_inner().unwrap();
    if res.len() == 0 {
        println!("没有找到速度大于 500MB/s 的 IP");
        return Ok(());
    }
    println!(
        "共找到 {} 个速度大于 500MB/s 的 IP，现在根据速度重新排序...",
        res.len()
    );
    res.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    res.truncate(20);
    println!("最佳IP: {:?} Speed: {:.0}", res[0].0, res[0].1);
    let res: Vec<String> = res
        .iter()
        .map(|(ip, speed)| format!("{}#BCFGL{:.0}", ip, speed))
        .collect();
    let data = res.join("\n");
    std::fs::write("best_ips.txt", data).expect("Unable to write file");
    Ok(())
}
