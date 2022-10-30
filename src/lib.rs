#![feature(map_try_insert)]

use std::{collections::HashMap, error::Error};

use css_inline::{CSSInliner, InlineError};
use kuchiki::{traits::TendrilSink, NodeRef};
use pulldown_cmark::html;
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};

pub mod config;
use config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(flatten)]
    fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum ChatlogBlock {
    Timestamp { message: String },
    Post { user: User, message: String },
}

/// Parse a chatlog of messages into a list of [`PostBlock`] objects.
///
/// The `input` is a chatlog of messages, formatted in play-script style.
/// For example:
///
/// ```
/// A: Here's a series of messages as sent by A
/// A: Hello!
/// A: Hi!
/// B: Other people can also speak!
/// A: You can weave messages together seamlessly!
/// @ Today at 4:20 PM
/// B: If timestamps are used in the template, you can set them using an "@" symbol on it's own line
/// @ Tomorrow on Wednesday
/// C: The timestamp is freeform and can be any text.
/// ```
pub fn parse_posts(config: &Config, input: String) -> Vec<ChatlogBlock> {
    let mut posts = vec![];

    let mut prev_post = None;

    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        } else if line.starts_with("@") {
            // If there is a message already being constructed, finish it, then go on with the rest of the timestamp
            if let Some((user, message)) = prev_post {
                posts.push(ChatlogBlock::Post { user, message });
                prev_post = None;
            }

            // Lines starting with @ are timestamp messages
            // These have the format "@ Today at 4:13 PM" and update the timestamp
            // (The timestamp is actually freeform text, allowing for Goofs)
            let message = line[1..].trim().to_string();
            posts.push(ChatlogBlock::Timestamp { message });
        } else {
            match line.split_once(": ") {
                // Check if this is a line that looks like it starts with a name
                // Ex: "AARON: bee removal"
                // if it is, treat it as a new message. Otherwise, treat it
                // as a multiline message.
                // Note that multiline messages have slightly closer spacing
                // compared to lines across different messages
                Some((name, message)) if name.chars().all(|x| x.is_alphanumeric()) => {
                    if let Some((user, message)) = prev_post {
                        posts.push(ChatlogBlock::Post { user, message });
                    }

                    let user = get_user(config, &name);
                    // Need to re-add new line explicitly, since `input.lines()` strips the newline.
                    prev_post = Some((user, format!("{}\n", message)))
                }
                _ => {
                    if let Some((_, message)) = &mut prev_post {
                        message.push_str(line);
                        // Same deal, need to explicitly re-add new line
                        message.push('\n');
                    }
                }
            };
        }
    }

    if let Some((user, message)) = prev_post {
        posts.push(ChatlogBlock::Post { user, message });
    }

    fn get_user(config: &Config, name: &str) -> User {
        config.people.get(name).cloned().unwrap_or({
            User {
                fields: [
                    ("name".to_string(), name.to_string()),
                    ("key".to_string(), name.to_string()),
                ]
                .into_iter()
                .collect(),
            }
        })
    }

    posts
}

/// Render a slice of PostBlocks using the given Tera template.
///
/// `template_name` is cosmetic--this is simply used for error reporting and debugging.
/// `template` should be contain the contents of the Tera template.
/// `posts` is a list of PostBlocks. This list should usually be produced by [`parse_posts`].
pub fn render(
    template_name: &str,
    template: &str,
    posts: &[ChatlogBlock],
    config: &Config,
    additional_variables: impl IntoIterator<Item = (String, serde_json::Value)>,
) -> Result<String, Box<dyn Error>> {
    let mut tera = Tera::default();
    tera.add_raw_template(template_name, template)?;
    tera.register_filter("markdown", markdown_to_html);

    let mut context = Context::new();
    context.insert("posts", &posts);
    context.insert("users", &config.people.values().collect::<Vec<_>>());
    for (name, value) in additional_variables {
        context.insert(name, &value);
    }

    let html = tera.render(template_name, &context)?;

    // Cohost ignores <style> tags in posts. Therefore, we need to make anything in
    // a style tag become an inline `style=""` attribute.
    let html = inline_style_tags(&html)?;

    let mut document = kuchiki::parse_html().one(html);

    // Cohost also ignores `class` and `id` attributes. Hence, we can strip to make the
    // post somewhat smaller.
    remove_class_and_id_attributes(&mut document)
        .map_err(|()| "Couldn't remove class and id attributes!")?;

    // css_inline adds <html>, <head>, and <body> tags to our post, which are all ignored by
    // Cohost and wouldn't even make sense to add. We remove them here.
    let inner = document.select_first("body").unwrap().as_node().children();
    let html = inner
        .map(|node| node.to_string())
        .collect::<Vec<String>>()
        .join("\n");
    Ok(html)
}

fn remove_class_and_id_attributes(document: &mut NodeRef) -> Result<(), ()> {
    for node in document.select("*")? {
        node.attributes.borrow_mut().remove("class");
        node.attributes.borrow_mut().remove("id");
    }
    Ok(())
}

// Inline the `<style>` tags of a string containing HTML. This will also remove the `<style>` tags
// from the string.
fn inline_style_tags(html: &str) -> Result<String, InlineError> {
    let inliner = CSSInliner::options()
        .inline_style_tags(true)
        .remove_style_tags(true)
        .extra_css(None)
        .build();
    inliner.inline(html)
}

// Convert a string-like tera::Value containing Markdown syntax into
// HTML containing tags that correctly render the syntax.
fn markdown_to_html(
    value: &tera::Value,
    _: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let value = value.as_str().ok_or(tera::Error::msg(
        "non-string value passed to markdown filter",
    ))?;
    let options = pulldown_cmark::Options::ENABLE_STRIKETHROUGH;
    let parser = pulldown_cmark::Parser::new_ext(value, options);

    let mut html = String::new();
    html::push_html(&mut html, parser);
    // pulldown-cmark adds <p> tags to the HTMLified text, which is undesirable since
    // this also adds a large margin to all the text contained within it. We can fix this
    // by simply removing all the <p> tags. This is safe and won't cause problems with
    // literal textual tags in the message, since those will already be escaped.
    // (and if they want an actual <p> tag, they should probably edit the HTML output of Cohoard,
    // as most of the time, a raw <p> will look ugly).
    let html = html.replace("<p>", "").replace("</p>", "");

    // TODO: pulldown_cmark only understands __underscore__ as **bold**, so underline can only
    // be done currently by using <u> tags.
    // TODO: This find-and-replace nonsense should really be replaced by an actual HTML parser that
    // can do actual replaces.

    // Cohost doesn't accept <u>, so we need to replace it with a span that can be manually styled.
    let html = html
        .replace("<u>", r#"<span class="cohoard-underline">"#)
        .replace("</u>", "</span>")
        // pulldown_cmark encodes code blocks like this:
        //  ```rust
        //  my code block
        //  ```
        // as
        // `<pre><code class=language-rust>my code block</code></pre>
        // We will transform this into `<div class="cohoard-codeblock language-rust>my code block</div>`
        // Note that if the language specifier is left off (ex: `rust` is left off in the example above)
        // then pulldown_cmark does not add the `class=language-rust` attribute.
        .replace(r#"<pre><code>"#, r#"<div class="cohoard-codeblock">"#)
        // Note that the missing angle bracket in the transform below is a bit of a hack to allow us
        // to not use a regex.
        .replace(
            r#"<pre><code class="language-"#,
            r#"<div class="cohoard-codeblock language-"#,
        )
        .replace("</code></pre>", "</div>")
        // Cohost attaches pseudo-elements to <code>, which can't be overriden with inline styling
        // to get around this, we replace it with a span that we can manually style in a template.
        .replace("<code>", r#"<span class="cohoard-code">"#)
        .replace("</code>", "</span>");

    Ok(tera::Value::String(html))
}