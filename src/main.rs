use std::{io::Read, path::PathBuf};

use clap::Parser;
use serde::Serialize;
use tera::{Context, Tera};

#[derive(Debug, Clone, Serialize)]
struct User {
    name: String,
    color: String,
    avatar: String,
}

impl User {
    fn aaron() -> User {
        User {
            name: "Aaron".into(),
            color: "#FF8200".into(),
            avatar: "ralsei_cropped.png".into(),
        }
    }

    fn cassie() -> User {
        User {
            name: "Cassie".into(),
            color: "#69C97A".into(),
            avatar: "cassie.webp".into(),
        }
    }
}

// Represents a block of posts. Each PostBlock renders the user headline plus
// some number of messages in the post body. A new PostBlock is usually issued
// whenever the following happen:
// - The time stamp would change
// - A different person posted something
#[derive(Debug, Clone, Serialize)]
struct PostBlock {
    user: User,
    timestamp: String,
    messages: Vec<String>,
}

impl PostBlock {
    fn new(user: User, timestamp: &str, messages: &[String]) -> PostBlock {
        PostBlock {
            user,
            timestamp: timestamp.into(),
            messages: messages.into(),
        }
    }
}

fn parse_posts(input: String) -> Vec<PostBlock> {
    let mut posts = vec![];

    let mut timestamp = String::from("Today");

    let mut name = String::new();
    let mut messages = vec![];

    fn post(name: &str, timestamp: &str, messages: &mut Vec<String>) -> PostBlock {
        let user = match name {
            "AARON" => User::aaron(),
            "CASSIE" => User::cassie(),
            _ => User {
                name: name.into(),
                color: "#FFFFFF".into(),
                avatar: "nothing.png".into(),
            },
        };

        let post = PostBlock::new(user, &timestamp, &messages);
        messages.clear();
        post
    }

    for line in input.lines() {
        // Lines starting with @ are timestamp messages
        // These have the format "@ Today at 4:13 PM" and update the timestamp
        // (The timestamp is actually freeform text, allowing for Goofs)
        if line.starts_with("@") {
            if !messages.is_empty() {
                posts.push(post(&name, &timestamp, &mut messages));
            }

            let new_timestamp = line[1..].trim();
            if !new_timestamp.is_empty() {
                timestamp = new_timestamp.to_string();
            }
        } else {
            match line.split_once(": ") {
                Some((maybe_next_name, maybe_message)) => {
                    // Check if this is a line that looks like it starts with a name
                    // Ex: "AARON: bee removal"
                    // if it is, treat it as a new message. Otherwise, treat it
                    // as a multiline message.
                    // Note that multiline messages have slightly closer spacing
                    // compared to lines across different messages
                    if maybe_next_name
                        .chars()
                        .all(|x| x.is_alphabetic() && x.is_uppercase())
                    {
                        if maybe_next_name != name && !name.is_empty() {
                            posts.push(post(&name, &timestamp, &mut messages));
                        }
                        name = maybe_next_name.into();
                        messages.push(maybe_message.into());
                    } else {
                        messages.push(line.into());
                    }
                }
                None => {
                    if let Some(last_msg) = messages.last_mut() {
                        *last_msg += "<br>\n";
                        *last_msg += line;
                    } else {
                        messages.push(line.into())
                    }
                }
            };
        }
    }

    if !messages.is_empty() {
        posts.push(post(&name, &timestamp, &mut messages));
    }
    posts
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about = "a chatlog formatter for cohost", long_about = None)]
struct Args {
    /// The file containing the chatlog.
    in_file: Option<PathBuf>,
    /// The configuration file to use.
    #[clap(long, short, default_value = "config.yaml")]
    config: PathBuf,
    /// The file to write the HTML file to, if provided. Otherwise, prints to standard out.
    #[clap(long = "out", short)]
    out_file: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    let input = if let Some(path) = args.in_file {
        std::fs::read_to_string(path).unwrap()
    } else {
        let mut string = String::new();
        std::io::stdin().read_to_string(&mut string).unwrap();
        string
    };

    let posts = parse_posts(input);

    let tera = Tera::new("templates/**/*.html").unwrap();

    let mut context = Context::new();
    context.insert("posts", &posts);

    let html = tera.render("discord.html", &context).unwrap();

    if let Some(out_path) = args.out_file {
        std::fs::write(out_path, html).unwrap();
    } else {
        println!("{}", html);
    }
}
