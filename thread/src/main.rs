use crossbeam::crossbeam_channel;
use reqwest;
use scraper::{Html, Selector};
use std::fs::{self, File};
use std::io;
use std::thread;

const FLODER: &'static str = "images";
const START_URL: &'static str = "https://www.jdlingyu.com/collection/meizitu/page";
const CONCURRENCY: u32 = 16;
const PAGE_COUNT: u32 = 115;

fn main() {
    println!("bingo => crawler is running......");
    fs::create_dir_all(FLODER).unwrap();

    let (ctx, crx) = crossbeam_channel::bounded::<String>(1024);
    let mut consumers = vec![];
    for _ in 0..CONCURRENCY {
        let crx = crx.clone();
        consumers.push(thread::spawn(move || loop {
            let u = match crx.recv() {
                Ok(u) => u,
                Err(crossbeam_channel::RecvError) => break,
            };

            let mut response = reqwest::blocking::get(&u).unwrap();
            let filename = format!("{}/{}", FLODER, u.split('/').last().unwrap());

            let mut out = File::create(filename.to_owned()).unwrap();
            io::copy(&mut response, &mut out).unwrap();
            println!("download file => {}", filename)
        }));
    }

    let (ptx, prx) = crossbeam_channel::bounded::<String>(1024);
    let mut producers = vec![];
    for _ in 0..CONCURRENCY / 4 {
        let prx = prx.clone();
        let ctx = ctx.clone();
        producers.push(thread::spawn(move || loop {
            let u = match prx.recv() {
                Ok(u) => u,
                Err(crossbeam_channel::RecvError) => break,
            };

            for u in get_image_urls(&u).iter() {
                ctx.send(u.to_owned()).unwrap_or_default();
            }
        }));
    }

    for i in 1..PAGE_COUNT {
        let u = format!("{}/{}", START_URL, i);
        ptx.send(u).unwrap_or_default();
    }
    drop(ptx);

    // block until all urls have been sent.
    for p in producers {
        p.join().unwrap_or_default();
    }
    drop(ctx);

    // block until that all files downloaded.
    for c in consumers {
        c.join().unwrap_or_default();
    }
}

fn get_image_urls(u: &str) -> Vec<String> {
    let mut urls = vec![];

    if let Ok(response) = reqwest::blocking::get(u) {
        let document = Html::parse_fragment(&response.text().unwrap());
        let ul = Selector::parse("#post-list > ul").unwrap();
        let li = Selector::parse("li").unwrap();
        let a = Selector::parse(".thumb-link").unwrap();

        if let Some(iter) = document.select(&ul).next() {
            for element in iter.select(&li) {
                for h in element.select(&a) {
                    urls.push(match h.value().attr("href") {
                        Some(u) => u.to_owned(),
                        None => continue,
                    });
                }
            }
        };
    };

    let mut imgs = vec![];
    let div = Selector::parse("#primary-home > article > div.entry-content").unwrap();
    let img = Selector::parse("img").unwrap();

    for url in urls.iter() {
        let response = match reqwest::blocking::get(url.as_str()).ok() {
            Some(r) => r,
            None => continue,
        };
        let document = Html::parse_fragment(&response.text().ok().unwrap());

        if let Some(iter) = document.select(&div).next() {
            for element in iter.select(&img) {
                imgs.push(match element.value().attr("src") {
                    Some(u) => u.to_owned(),
                    None => continue,
                });
            }
        };
    }

    imgs
}
