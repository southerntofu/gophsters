use chrono::{DateTime,Utc};
use serde::{Serialize,Deserialize};

use crate::Result;
use crate::fetch;

#[derive(Serialize, Deserialize, Debug)]
pub struct Story {
    pub title: String,
    #[serde(rename(deserialize = "created_at"))]
    pub date: DateTime<Utc>,
    pub score: i8,
    #[serde(rename(deserialize = "comment_count"))]
    pub count: u8,
    #[serde(rename(deserialize = "short_id"))]
    pub id: String,
    #[serde(rename(deserialize = "short_id_url"))]
    pub permalink: String,
    pub url: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename(deserialize = "submitter_user"))]
    pub user: User,
    #[serde(default = "Vec::new")]
    pub comments: Vec<Comment>,
    #[serde(rename(deserialize = "description"))]
    pub text: String
}

impl Story {
    pub fn fetch_comments(&mut self) -> Result<()> {
        //println!("im within the loop!");
        self.comments = fetch::comments(&self.permalink)?;
        Ok(())
    }
}

// Flatten and renaming don't work well together so we need a dumb struct
#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    #[serde(rename(deserialize = "username"))]
    pub name: String
}

pub type Date = DateTime<Utc>;

/// CommentRoot is meta-structure linked to a page that holds its related comments
/// We omit some fields from the API because we flatten what's relevant directly into a Story
#[derive(Deserialize, Debug)]
pub struct CommentRoot {
    pub comments: Vec<Comment>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Comment {
    #[serde(rename(deserialize = "comment"))]
    pub text: String,
    #[serde(rename(deserialize = "created_at"))]
    pub date: Date,
    pub score: i8,
    #[serde(rename(deserialize = "indent_level"))]
    pub indentation: u8,
    #[serde(rename(deserialize = "commenting_user"))]
    pub user: User,
}
