use async_std::task;
use crossbeam::crossbeam_channel;
use reqwest;
use scraper::{Html, Selector};
use std::fs::{self, File};
use std::io;

async fn parse_pages(u: String, sender: crossbeam_channel::Sender<String>) {
    if let Ok(response) = reqwest::blocking::get(&u) {
        let document = Html::parse_fragment(&response.text().unwrap());
        let ul = Selector::parse("#post-list > ul").unwrap();
        let li = Selector::parse("li").unwrap();
        let a = Selector::parse(".thumb-link").unwrap();

        if let Some(iter) = document.select(&ul).next() {
            for element in iter.select(&li) {
                for h in element.select(&a) {
                    let _ = match h.value().attr("href") {
                        Some(u) => sender.send(u.to_owned()),
                        None => continue,
                    };
                }
            }
        };
    }
}

async fn extract_urls(
    receiver: crossbeam_channel::Receiver<String>,
    sender: crossbeam_channel::Sender<String>,
) {
    loop {
        let u = match receiver.recv() {
            Ok(u) => u,
            Err(crossbeam_channel::RecvError) => break,
        };

        let div = Selector::parse("#primary-home > article > div.entry-content").unwrap();
        let img = Selector::parse("img").unwrap();
        if let Ok(response) = reqwest::blocking::get(&u) {
            let document = Html::parse_fragment(&response.text().unwrap());
            if let Some(iter) = document.select(&div).next() {
                for element in iter.select(&img) {
                    let _ = match element.value().attr("src") {
                        Some(u) => sender.send(u.to_owned()),
                        None => continue,
                    };
                }
            };
        }
    }
}

async fn download_images(receiver: crossbeam_channel::Receiver<String>) {
    loop {
        let u = match receiver.recv() {
            Ok(u) => u,
            Err(crossbeam_channel::RecvError) => break,
        };

        if let Ok(mut response) = reqwest::blocking::get(&u) {
            let filename = format!("{}/{}", FLODER, u.split('/').last().unwrap());
            let mut out = File::create(filename.to_owned()).unwrap();
            io::copy(&mut response, &mut out).unwrap();
            println!("download file => {}", filename)
        }
    }
}

const FLODER: &'static str = "images";
const START_URL: &'static str = "https://www.jdlingyu.com/collection/meizitu/page";
const CONCURRENCY: u32 = 32;
const PAGE_COUNT: u32 = 115;

fn main() {
    println!("bingo => crawler is running......");
    fs::create_dir_all(FLODER).unwrap();

    let (ptx, prx) = crossbeam_channel::bounded::<String>(1024);
    let mut parser = vec![];

    let (etx, erx) = crossbeam_channel::bounded::<String>(1024);
    let mut extracter = vec![];
    let mut downloader = vec![];

    task::block_on(async {
        for i in 1..PAGE_COUNT {
            let u = format!("{}/{}", START_URL, i);
            let sender = ptx.clone();
            parser.push(task::spawn(async move { parse_pages(u, sender).await }));
        }

        for _ in 0..CONCURRENCY {
            let receiver = prx.clone();
            let sender = etx.clone();
            extracter.push(task::spawn(
                async move { extract_urls(receiver, sender).await },
            ));
        }

        for _ in 0..CONCURRENCY {
            let receiver = erx.clone();
            downloader.push(task::spawn(async move { download_images(receiver).await }));
        }

        for t in parser {
            t.await;
        }
        drop(ptx);

        for t in extracter {
            t.await;
        }
        drop(etx);

        for t in downloader {
            t.await;
        }
    })
}
