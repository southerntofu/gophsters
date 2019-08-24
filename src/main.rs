use chrono::{DateTime,Utc};
use regex::Regex;
use textwrap::{fill, indent};
use deunicode::deunicode;
use serde::{Serialize,Deserialize};
use structopt::StructOpt;

use std::fs::File;
use std::io::prelude::*;

use reqwest::get;
use url::Url;

// Used for asynchronous iteration over stories
// i.e. parallel blocking network IO via rayon
use rayon::prelude::*;

// For simple automagic error handling
use error_chain::error_chain;

error_chain!{
    foreign_links {
        Http(reqwest::Error);
        Json(serde_json::Error);
        Io(std::io::Error);
        Templating(tera::Error);
    }
}

//use tera::{Tera,compile_templates};
use tera::{Context,Tera};

#[derive(Debug, StructOpt)]
#[structopt(name = "gophsters", about = "Generate a gophermap from lobste.rs recent stories")]
struct Cli {
    /// The host to fetch Lobsters articles from
    #[structopt(short = "h", long = "host", default_value = "lobste.rs")]
    host: String,
}

// TODO:
// - replace \n with \r\n
// - look for other regressions (tests would help)

const GOPHER_MAP: &str = r#"""
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

Last updated {{ now | date(format="%a %b %e %T %Y") }}

{% for story in stories %}
h[{{ story.score }}] - {{ story.title }}{% if story.url %}  URL:{{ story.url }}{% endif %}
Submitted {{ story.date | date(format="%a %b %e %T %Y") }} by {{ story.user.name }} | {{ story.tags | join(sep=", ") }}
0View comments ({{ story.count }}) {{ story.id }}.txt
{% endfor %}
"""#;

const GOPHER_PAGE: &str = r#"""
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


Viewing comments for "{{ story.title }}"
---
{% for comment in story.comments %}
{{ comment.user.name }} commented [{{ comment.score }}]
{{ comment.text }}{#| cleanup(comment.indentation) }}TODO#}
{% endfor %}
"""#;

fn download(url: &str) -> Result<String> {
    Ok(get(url)?.text()?)
}

fn fetch_stories(url: &str) -> Result<Vec<Story>> {
    let body = download(&url)?;
    let stories: Vec<Story> = serde_json::from_str(&body)?;
    Ok(stories)
}

fn fetch_comments(permalink: &str) -> Result<Vec<Comment>> {
    let url = format!("{}.json", permalink);
    let body = download(&url)?;
    let comment_root: CommentRoot = serde_json::from_str(&body)?;
    Ok(comment_root.comments)
}

fn main() -> Result<()> {
    let cli = Cli::from_args();

    let host = match cli.host.starts_with("http") {
        true => cli.host,
        false => format!("https://{}", cli.host)
    };

    let base_url = Url::parse(&host).expect("Could not parse hostname");
    // join() doesn't care about a trailing slash passed as host
    let url: String = base_url.join("hottest.json").unwrap().as_str().parse().unwrap();

    // Initialize the templates
    let mut tera = Tera::default();
    tera.add_raw_template("gopher/section", GOPHER_MAP)?;
    tera.add_raw_template("gopher/article", GOPHER_PAGE)?;

    // Configure rayon to use maximum 4 threads (so we don't get blocked by the lobsters API)
    rayon::ThreadPoolBuilder::new().num_threads(4).build_global().unwrap();

    let mut stories = fetch_stories(&url)?;
    build_gopher_section(&stories, &tera)?;

    // Sweet, sweet rayon for parellel processing
    stories.par_iter_mut()
        .for_each(|story| {
            match story.fetch_comments() {
                Ok(_) => {
                    println!("Story {} has {} comments.", story.title, story.comments.len());
                    match build_gopher_article(&story, &tera) {
                        Ok(_) => {
                            // The comments page was built successfully
                        },
                        Err(_) => {
                            eprintln!("Failed to build comments for page {}", &story.title);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Failed to fetch comments for page {} because of error\n{}", &story.title, e);
                }
            }
        });                 

    println!("Done.");
    Ok(())
}

fn build_gopher_section(stories: &Vec<Story>, tera: &Tera) -> Result<()> {
    let mut f = File::create("gophermap")?;
    //let gophermap = stories_to_gophermap(stories);
    let mut context = Context::new();
    context.insert("stories", stories);
    let contents = match tera.render("gopher/section", &context) {
        Ok(s) => s,
        Err(e) => {
            println!("Building the template failed because of error\n{:#?}", e);
            // Silently discard the error
            return Ok(());
        }
    };

    f.write_all(&contents.as_bytes())?;
    Ok(())
}

fn termination_line() -> String {
    "\r\n.".to_owned()
}


fn build_gopher_article(story: &Story, tera: &Tera) -> Result<()> {
    let mut f = File::create(format!("{}.txt", story.id))?;
    //let coms = build_comments_page(story);
    let mut context = Context::new();
    context.insert("story", story);
    let contents = match tera.render("gopher/article", &context) {
        Ok(s) => s,
        Err(e) => { println!("Tera failed because of error\n{:?}", e); return Ok(()); }
    };
    f.write_all(&contents.as_bytes())?;
    Ok(())
}

fn indent_comment(string: String, level: u8) -> String {
    match level {
        1 => indent(&fill(&string, 60), ""),
        2 => indent(&fill(&string, 60), "\t"),
        _ => indent(&fill(&string, 60), "\t\t"),
    }
}

fn cleanup(comment: &str) -> String {
    let re = Regex::new(r"<.*?>").unwrap();
    let cleaned: String = deunicode(&comment);
    let result = re.replace_all(&cleaned, "");
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

fn comment_title(title: &str) -> String {
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


Viewing comments for \"{}\"
---

", deunicode(title))
}

fn pretty_date(date: &Date) -> String {
    //let parsed_date = date_string.parse::<DateTime<Utc>>();
    date.format("%a %b %e %T %Y").to_string()
}

#[derive(Serialize, Deserialize, Debug)]
struct Story {
    title: String,
    #[serde(rename(deserialize = "created_at"))]
    date: DateTime<Utc>,
    score: u8,
    #[serde(rename(deserialize = "comment_count"))]
    count: u8,
    #[serde(rename(deserialize = "short_id"))]
    id: String,
    #[serde(rename(deserialize = "short_id_url"))]
    permalink: String,
    url: Option<String>,
    tags: Vec<String>,
    #[serde(rename(deserialize = "submitter_user"))]
    user: User,
    #[serde(default = "Vec::new")]
    //#[serde(skip)]
    comments: Vec<Comment>,
    #[serde(rename(deserialize = "description"))]
    text: String
}

impl Story {
    fn fetch_comments(&mut self) -> Result<()> {
        //println!("im within the loop!");
        self.comments = fetch_comments(&self.permalink)?;
        Ok(())
    }
}

// Flatten and renaming don't work well together so we need a dumb struct
#[derive(Serialize, Deserialize, Debug)]
struct User {
    #[serde(rename(deserialize = "username"))]
    name: String
}

type Date = DateTime<Utc>;

/// CommentRoot is meta-structure linked to a page that holds its related comments
/// We omit some fields from the API because we flatten what's relevant directly into a Story
#[derive(Deserialize, Debug)]
struct CommentRoot {
    comments: Vec<Comment>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Comment {
    #[serde(rename(deserialize = "comment"))]
    text: String,
    #[serde(rename(deserialize = "created_at"))]
    date: Date,
    score: u8,
    #[serde(rename(deserialize = "indent_level"))]
    indentation: u8,
    #[serde(rename(deserialize = "commenting_user"))]
    user: User,
}

/* For further reference, the original "templates"

fn stories_to_gophermap(stories: &Vec<Story>) -> String {
    let mut gophermap = String::new();
    gophermap.push_str(&main_title());
    for story in stories {
        let story_line = match &story.url {
            Some(url) => {
                format!("h[{}] - {}\tURL:{}\n", story.score, deunicode(&story.title), url)
            },
            None => {
                format!("h[{}] - {}\n", story.score, deunicode(&story.title))
            }
        };

        let meta_line = format!("Submitted {} by {} | {}\n", pretty_date(&story.date), story.user.name, story.tags.join(", "));
        let comment_line = format!("0View comments ({})\t{}\n\n", &story.count, format!("{}.txt", &story.id));

        gophermap.push_str(&story_line);
        gophermap.push_str(&meta_line);
        gophermap.push_str(&comment_line);
    }
    gophermap.push_str(&termination_line());
    gophermap
}


fn build_comments_page(story: &Story) -> String {
    let mut c = String::new();
    c.push_str(&comment_title(&story.title));
    for comment in &story.comments {
        let meta_line = indent_comment(format!("> {} commented [{}]:\n", comment.user.name, comment.score), comment.indentation);
        let comment_line = format!("{}\n", indent_comment(cleanup(&comment.text), comment.indentation));
        c.push_str(&meta_line);
        c.push_str(&comment_line);
    }
    c.push_str(&termination_line());
    c
}
*/
