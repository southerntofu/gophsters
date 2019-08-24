use structopt::StructOpt;
use std::fs::File;
use std::io::prelude::*;

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

// For templating
use tera::{Context,Tera};

mod templates;
mod fetch;
mod data;
use data::Story;

#[derive(Debug, StructOpt)]
#[structopt(name = "gophsters", about = "Generate a gophermap from lobste.rs recent stories")]
struct Cli {
    /// The host to fetch Lobsters articles from
    #[structopt(short = "h", long = "host", default_value = "lobste.rs")]
    host: String,
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

fn main() -> Result<()> {
    // TODO:
    // - replace \n with \r\n
    // - fix \t output
    // - look for other regressions (tests would help)
    
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
    tera.add_raw_template("gopher/section", templates::GOPHER_MAP)?;
    tera.add_raw_template("gopher/article", templates::GOPHER_PAGE)?;

    // Configure rayon to use maximum 4 threads (so we don't get blocked by the lobsters API)
    rayon::ThreadPoolBuilder::new().num_threads(4).build_global().unwrap();

    let mut stories = fetch::stories(&url)?;
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


