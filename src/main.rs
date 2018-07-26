// #![deny(warnings)]
extern crate hyper;
extern crate hyper_tls;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

extern crate chrono;
use chrono::prelude::*;

extern crate regex;
use regex::Regex;

extern crate textwrap;
use textwrap::{fill, indent};

use std::fs::File;
use std::io::prelude::*;

use hyper::Client;
use hyper::rt::{self, Future, Stream};
use hyper_tls::HttpsConnector;

const API_URL: &'static str = "https://lobste.rs/hottest.json";

fn main() {
    let url = API_URL.parse().unwrap();

    let fut = fetch_stories(url)
        .map(|stories| {
            create_gophermap(stories).unwrap();
        })
        .map_err(|e| {
            match e {
                FetchError::Http(e) => eprintln!("http error: {}", e),
                FetchError::Json(e) => eprintln!("json parsing error: {}", e),
            }
        });

    rt::run(fut);

    println!("Done.")
}

fn create_gophermap(stories: Vec<Story>) -> std::io::Result<()> {
    let mut f = File::create("gophermap")?;
    let gophermap = stories_to_gophermap(stories);
    f.write_all(&gophermap.as_bytes())?;
    Ok(())
}

fn stories_to_gophermap(stories: Vec<Story>) -> String {
    let mut gophermap = String::new();
    gophermap.push_str(&main_title());
    for story in stories {
        println!("Building story: {}", story.title);

        let story_line = format!("h[{}] - {}\tURL:{}\n", story.upvotes, story.title, story.short_id_url);
        let meta_line = format!("Submitted {} by {} | {}\n", pretty_date(&story.created_at), story.submitter_user.username, story.tags.join(", "));
        let comment_line = format!("0View comments ({})\t{}\n\n", &story.comment_count, format!("{}.txt", &story.short_id));
        build_comments_for(story);

        gophermap.push_str(&story_line);
        gophermap.push_str(&meta_line);
        gophermap.push_str(&comment_line);
    }
    gophermap
}

fn build_comments_for(story: Story) {
    let url = format!("{}.json", &story.short_id_url).parse().unwrap();
    let fut = fetch_comments(url)
        .map(|(comments, short_id)| {
            let mut f = File::create(format!("{}.txt", short_id)).unwrap();
            let coms = build_comments_page(comments);
            f.write_all(&coms.as_bytes()).expect("could not write file");
        })
        .map_err(|e| {
            match e {
                FetchError::Http(e) => eprintln!("http error: {}", e),
                FetchError::Json(e) => eprintln!("json parsing error: {}", e),
            }
        });

    rt::run(fut);
}

fn build_comments_page(comments: Vec<Comment>) -> String {
    let mut c = String::new();
    c.push_str(&comment_title());
    for comment in comments {
        let meta_line = indent_comment(format!("{} commented:\n", comment.commenting_user.username), comment.indent_level);
        let comment_line = format!("{}\n", indent_comment(cleanup(comment.comment), comment.indent_level));
        c.push_str(&meta_line);
        c.push_str(&comment_line);
    }
    c
}

fn indent_comment(string: String, level: u8) -> String {
    match level {
        1 => indent(&fill(&string, 60), ""),
        2 => indent(&fill(&string, 60), "\t"),
        _ => indent(&fill(&string, 60), "\t\t"),
    }
}

fn cleanup(comment: String) -> String {
    let re = Regex::new(r"<.*?>").unwrap();
    let result = re.replace_all(&comment, "");
    result.to_string()
}

fn main_title() -> String {
    let utc = Utc::now().format("%a %b %e %T %Y").to_string();
    format!("
 .----------------.
| .--------------. |
| |   _____      | |
| |  |_   _|     | |
| |    | |       | |
| |    | |   _   | |
| |   _| |__/ |  | |
| |  |________|  | |
| |              | |
| '--------------' |
 '----------------'

This is an unofficial Lobste.rs mirror on gopher.
You can find the 25 hottest stories and their comments.
Sync happens every 10 minutes or so.

Last updated {}

", utc)
}

fn comment_title() -> String {
    "
 .----------------.
| .--------------. |
| |   _____      | |
| |  |_   _|     | |
| |    | |       | |
| |    | |   _   | |
| |   _| |__/ |  | |
| |  |________|  | |
| |              | |
| '--------------' |
 '----------------'


".to_owned()
}

fn pretty_date(date_string: &String) -> String {
    let parsed_date = date_string.parse::<DateTime<Utc>>();
    let date = match parsed_date {
        Ok(date) => date,
        Err(_e)  => Utc::now(),
    };
    date.format("%a %b %e %T %Y").to_string()
}

fn fetch_stories(url: hyper::Uri) -> impl Future<Item=Vec<Story>, Error=FetchError> {
    let https = HttpsConnector::new(4).expect("TLS initialization failed");
    let client = Client::builder()
        .build::<_, hyper::Body>(https);

    client
        .get(url)
        .and_then(|res| {
            res.into_body().concat2()
        })
        .from_err::<FetchError>()
        .and_then(|body| {
            let stories = serde_json::from_slice(&body)?;

            Ok(stories)
        })
        .from_err()
}

fn fetch_comments(url: hyper::Uri) -> impl Future<Item=(Vec<Comment>, String), Error=FetchError> {
    let https = HttpsConnector::new(4).expect("TLS initialization failed");
    let client = Client::builder()
        .build::<_, hyper::Body>(https);

    client
        .get(url)
        .and_then(|res| {
            res.into_body().concat2()
        })
        .from_err::<FetchError>()
        .and_then(|body| {
            let body_string = std::str::from_utf8(&body).unwrap();
            let json_body: CommentRoot = serde_json::from_str(&body_string)?;
            let comments = json_body.comments;
            Ok((comments, json_body.short_id))
        })
        .from_err()
}

#[derive(Deserialize, Debug)]
struct Story {
    title: String,
    created_at: String,
    upvotes: u8,
    score: i8,
    comment_count: u8,
    short_id: String,
    short_id_url: String,
    tags: Vec<String>,
    submitter_user: User,
}

#[derive(Deserialize, Debug)]
struct User {
    username: String,
}

#[derive(Deserialize, Debug)]
struct CommentRoot {
    short_id: String,
    comments: Vec<Comment>,
}

#[derive(Deserialize, Debug)]
struct Comment {
    comment: String,
    created_at: String,
    upvotes: u8,
    score: i8,
    indent_level: u8,
    commenting_user: User,
}

enum FetchError {
    Http(hyper::Error),
    Json(serde_json::Error),
}

impl From<hyper::Error> for FetchError {
    fn from(err: hyper::Error) -> FetchError {
        FetchError::Http(err)
    }
}

impl From<serde_json::Error> for FetchError {
    fn from(err: serde_json::Error) -> FetchError {
        FetchError::Json(err)
    }
}
