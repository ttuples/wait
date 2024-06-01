use std::str::Chars;
use std::iter::Peekable;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone)]
pub struct ManifestParseError;

impl std::fmt::Display for ManifestParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Failed to parse manifest")
    }
}

impl std::error::Error for ManifestParseError {}

fn skip_whitespace(input: &mut Peekable<Chars>) {
    while let Some(&ch) = input.peek() {
        if ch.is_whitespace() {
            input.next();
        } else {
            break;
        }
    }
}

fn parse_string(input: &mut Peekable<Chars>) -> String {
    let mut value = String::new();
    input.next(); // Consume the opening double quote

    while let Some(ch) = input.next() {
        if ch == '"' {
            break;
        }
        value.push(ch);
    }

    value
}

fn parse_object(input: &mut Peekable<Chars>) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    let mut key;

    while let Some(&ch) = input.peek() {
        if ch == '}' {
            break;
        }
        skip_whitespace(input);
        key = parse_string(input); // Extract the key value
        skip_whitespace(input);

        if let Some(&ch) = input.peek() {
            if ch == '{' {
                input.next(); // Consume the opening curly brace
                obj.insert(key, parse_object(input));
                skip_whitespace(input);
            } else if ch == '"' {
                obj.insert(key, serde_json::Value::String(parse_string(input)));
                skip_whitespace(input);
            }

            if let Some(&ch) = input.peek() {
                if ch == '}' {
                    input.next(); // Consume the closing curly brace
                    break;
                }
            }
        }
    }

    serde_json::Value::Object(obj)
}

pub fn parse_manifest(path: std::path::PathBuf) -> Result<serde_json::Value> {
    let input = std::fs::read_to_string(path)?;
    let mut input = input.chars().peekable();

    // Skip first line
    while let Some(ch) = input.next() {
        if ch == '\n' {
            break;
        }
    }

    // Consume the opening curly brace and check for empty object
    while let Some(&ch) = input.peek() {
        if ch == '{' {
            break;
        }
        input.next();
    }
    input.next();

    Ok(parse_object(&mut input))
}