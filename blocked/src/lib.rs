#![feature(proc_macro_diagnostic)]

extern crate proc_macro;

use proc_macro::{Diagnostic, Level, TokenStream, Span};
use syn::{parse::Parser, LitStr, punctuated::Punctuated, Token};

use reqwest::header::{self, HeaderMap};
use serde::Deserialize;
use url::Url;
use regex::Regex;
use lazy_static::lazy_static;
use git2::Repository;

lazy_static! {
    static ref ISSUE: Regex = Regex::new(r"#?\d+").unwrap();
    static ref REPOISSUE: Regex = Regex::new(r"[\w-]+[#/]\d+").unwrap();
    static ref OWNERREPOISSUE: Regex = Regex::new(r"([\w-]+)/([\w-]+)[#/](\d+)").unwrap();
    static ref URL: Regex = Regex::new(r"https?://github.com/[\w-]+/issues/[\w-]+[#/]\d+").unwrap();
    static ref BASE: Url = Url::parse("https://api.github.com/repos/").unwrap();
    static ref REMOTE: Regex = Regex::new(r"(?:https://github.com/([\w-]+)/([\w-]+).git)|(?:git@github.com:([\w-]+)/([\w-]+).git)").unwrap();
}

/// Data returned from the Github issue API
///
/// Currently we only care about the state (open/closed)
// TODO: Add the date it was closed here?
#[derive(Deserialize)]
struct GithubIssue {
    state: String
}


#[proc_macro]
pub fn blocked(input: TokenStream) -> TokenStream {
    // Parse Arguments
    let (issue_pattern, reason) = match parse_args(input) {
        Ok(args) => args,
        Err(err) => return TokenStream::from(err.to_compile_error())
    };

    // Check if we have an API key, otherwise exit silently
    let api_key = if let Ok(key) = std::env::var("BLOCKED_GITHUB_API_KEY") {
        key
    } else {
        return TokenStream::new();
    };

    let client = github_client(&api_key);

    // Try to resolve the issue pattern to an issue API URL
    let url = match parse_issue_pattern(&issue_pattern) {
        Ok(url) => url,
        Err(err) => return TokenStream::from(err.to_compile_error())
    };
    eprintln!("{}", url);

    // Get issue status
    let r = client.get(url).send().unwrap();
    let issue = r.json::<GithubIssue>().unwrap();

    // Warn if the issue has been closed
    match issue.state.as_str() {
        "open" => (),
        "closed" => Diagnostic::spanned([Span::call_site()].as_ref(), Level::Note, reason.unwrap_or_else(|| "Issue was closed.".to_string())).emit(),
        _ => panic!("unknown response")
    }

    TokenStream::new()
}

/// Try to parse a reference to an issue (in a few forms) and optionally a 'reason' from the input TokenStream.
fn parse_args(input: TokenStream) -> Result<(String, Option<String>), syn::Error> {
    let parser = Punctuated::<LitStr, Token![,]>::parse_separated_nonempty;
    let args = parser.parse(input.clone())?;
    if args.len() < 1 || args.len() > 2 {
        return Err(error("Expected between 1 and 2 arguments"));
    }
    let mut args_iter = args.iter();
    Ok(
        (
            args_iter.next().ok_or_else(|| error("Expected an issue pattern as a first argument."))?.value(),
            args_iter.next().map(|s| s.value())
        )
    )
}

/// Get a client suitable for interacting with the Github API
fn github_client(api_key: &str) -> reqwest::blocking::Client {
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, header::HeaderValue::from_str(api_key).unwrap());
    headers.insert(header::USER_AGENT, header::HeaderValue::from_static("blocked-rs"));
    reqwest::blocking::Client::builder().default_headers(headers).build().unwrap()
}

/// Parse an issue pattern. Possible forms are documented on the main `blocked!` macro
fn parse_issue_pattern(pattern: &str) -> Result<Url, syn::Error> {
    if URL.is_match(pattern) {
        return Url::parse(pattern).map_err(|_| error("URL matched regex but was not accepted by the URL crate"))
    }
    if let Some(captures) = OWNERREPOISSUE.captures(pattern) {
        return BASE.clone()
            .join(&format!("{}/{}/issues/{}", captures.get(1).unwrap().as_str(), captures.get(2).unwrap().as_str(), captures.get(3).unwrap().as_str())).map_err(|_| error("Could not join URL fragments"))
    }
    if let Some(captures) = REPOISSUE.captures(pattern) {
        let (org, _) = try_get_org_repo()?;
        return BASE.clone()
            .join(&format!("{}/{}/issues/{}", org, captures.get(1).unwrap().as_str(), captures.get(2).unwrap().as_str())).map_err(|_| error("Could not join URL fragments"))
    }
    if let Some(captures) = ISSUE.captures(pattern) {
        let (org, repo) = try_get_org_repo()?;
        return BASE.clone()
            .join(&format!("{}/{}/issues/{}", org, repo, captures.get(2).unwrap().as_str())).map_err(|_| error("Could not join URL fragments"))
    }
    Err(error("Could not parse issue pattern"))
}

/// Try to get the organisation and repository from the current git repo.
///
/// This is used for shorthand issue patterns.
fn try_get_org_repo() -> Result<(String, String), syn::Error> {
    let repo = Repository::open_from_env().map_err(|_| error("Could not find or open a git repository"))?;

    let remote = if let Ok(remote) = repo.find_remote("upstream") {
        Some(remote)
    } else {
        repo.find_remote("origin").ok()
    }.ok_or_else(|| error("Could not find an 'upstream' or 'origin' remote"))?;

    REMOTE.captures(remote.url().ok_or_else(|| error("Remote URL not valid unicode"))?)
        .map(|captures| (captures.get(1).unwrap().as_str().to_owned(), captures.get(2).unwrap().as_str().to_owned()))
        .ok_or_else(|| error("Failed to parse remote URL"))
}

fn error(message: impl AsRef<str>) -> syn::Error {
    syn::Error::new(proc_macro2::Span::call_site(), message.as_ref())
}
