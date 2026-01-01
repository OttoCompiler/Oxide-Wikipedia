use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::SystemTime;


#[derive(Clone, Debug)]
struct Article {
    title: String,
    content: String,
    timestamp: u64,
}


#[derive(Clone, Debug)]
struct ArticleHistory {
    versions: Vec<Article>,
}


type WikiData = Arc<Mutex<HashMap<String, ArticleHistory>>>;


fn main() {
    let wiki_data: WikiData = Arc::new(Mutex::new(HashMap::new()));

    {
        let mut data = wiki_data.lock().unwrap();
        data.insert("main".to_string(), ArticleHistory {
            versions: vec![Article {
                title: "Main Page".to_string(),
                content: "Welcome to BauhausWiki\n\nA minimalist encyclopedia inspired by Bauhaus design principles.\n\nFeatured Articles:\n- [[Bauhaus]]\n- [[Design]]\n- [[Architecture]]\n\nStart exploring or create a new article.".to_string(),
                timestamp: timestamp(),
            }],
        });

        data.insert("bauhaus".to_string(), ArticleHistory {
            versions: vec![Article {
                title: "Bauhaus".to_string(),
                content: "The Bauhaus\n\nThe Bauhaus was a German art school operational from 1919 to 1933 that combined crafts and the fine arts.\n\nKey Principles:\n- Form follows function\n- Unity of art and technology\n- Geometric abstraction\n- Primary colors and shapes\n\nThe Bauhaus style is characterized by geometric forms, clean lines, and a focus on functionality. It influenced [[Architecture]] and [[Design]] worldwide.".to_string(),
                timestamp: timestamp(),
            }],
        });
    }

    let listener = TcpListener::bind("0.0.0.0:24439").unwrap();
    println!("BauhausWiki running at http://0.0.0.0:24439");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let wiki_clone = Arc::clone(&wiki_data);
                std::thread::spawn(move || handle_client(stream, wiki_clone));
            }
            Err(e) => eprintln!("Connection error: {}", e),
        }
    }
}


fn handle_client(mut stream: TcpStream, wiki_data: WikiData) {
    let mut buffer = [0; 4096];
    match stream.read(&mut buffer) {
        Ok(size) => {
            let request = String::from_utf8_lossy(&buffer[..size]);
            let response = process_request(&request, wiki_data);
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
        Err(e) => eprintln!("Read error: {}", e),
    }
}


fn process_request(request: &str, wiki_data: WikiData) -> String {
    let lines: Vec<&str> = request.lines().collect();
    if lines.is_empty() {
        return http_response(400, "Bad Request");
    }

    let parts: Vec<&str> = lines[0].split_whitespace().collect();
    if parts.len() < 2 {
        return http_response(400, "Bad Request");
    }

    let method = parts[0];
    let path = parts[1];

    match method {
        "GET" => handle_get(path, wiki_data),
        "POST" => handle_post(path, request, wiki_data),
        _ => http_response(405, "Method Not Allowed"),
    }
}


fn handle_get(path: &str, wiki_data: WikiData) -> String {
    if path == "/" {
        return handle_get("/wiki/main", wiki_data);
    }

    if path.starts_with("/wiki/") {
        let article_name = &path[6..];
        return view_article(article_name, wiki_data);
    }

    if path.starts_with("/edit/") {
        let article_name = &path[6..];
        return edit_page(article_name, wiki_data);
    }

    if path.starts_with("/history/") {
        let article_name = &path[9..];
        return history_page(article_name, wiki_data);
    }

    if path.starts_with("/search") {
        let query = extract_query_param(path, "q");
        return search_page(&query, wiki_data);
    }

    if path == "/styles.css" {
        return css_response();
    }

    http_response(404, "Not Found")
}


fn handle_post(path: &str, request: &str, wiki_data: WikiData) -> String {
    if path.starts_with("/save/") {
        let article_name = &path[6..];
        let body = extract_body(request);
        let content = extract_form_param(&body, "content");
        save_article(article_name, &content, wiki_data);
        return redirect_response(&format!("/wiki/{}", article_name));
    }

    http_response(404, "Not Found")
}


fn view_article(name: &str, wiki_data: WikiData) -> String {
    let data = wiki_data.lock().unwrap();

    match data.get(name) {
        Some(history) => {
            let article = &history.versions.last().unwrap();
            let html_content = markdown_to_html(&article.content);
            html_response(&format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>{} - BauhausWiki</title>
    <link rel="stylesheet" href="/styles.css">
</head>
<body>
    <header>
        <h1><a href="/">BauhausWiki</a></h1>
        <nav>
            <form action="/search" method="get" class="search-form">
                <input type="text" name="q" placeholder="Search..." class="search-input">
                <button type="submit" class="primary-btn">Search</button>
            </form>
        </nav>
    </header>
    <main>
        <article>
            <div class="article-header">
                <h2>{}</h2>
                <div class="article-actions">
                    <a href="/edit/{}" class="btn">Edit</a>
                    <a href="/history/{}" class="btn">History</a>
                </div>
            </div>
            <div class="article-content">
                {}
            </div>
        </article>
    </main>
    <footer>
        <p>2025 OttoCompiler</p>
    </footer>
</body>
</html>"#,
                article.title, article.title, name, name, html_content
            ))
        }
        None => {
            html_response(&format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Article Not Found - BauhausWiki</title>
    <link rel="stylesheet" href="/styles.css">
</head>
<body>
    <header>
        <h1><a href="/">BauhausWiki</a></h1>
    </header>
    <main>
        <div class="not-found">
            <h2>Article Not Found: {}</h2>
            <p>This article does not exist yet.</p>
            <a href="/edit/{}" class="primary-btn">Create Article</a>
            <a href="/" class="btn">Back to Main Page</a>
        </div>
    </main>
</body>
</html>"#,
                name, name
            ))
        }
    }
}


fn edit_page(name: &str, wiki_data: WikiData) -> String {
    let data = wiki_data.lock().unwrap();
    let content = match data.get(name) {
        Some(history) => history.versions.last().unwrap().content.clone(),
        None => String::new(),
    };

    html_response(&format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Edit {} - BauhausWiki</title>
    <link rel="stylesheet" href="/styles.css">
</head>
<body>
    <header>
        <h1><a href="/">BauhausWiki</a></h1>
    </header>
    <main>
        <article>
            <h2>Editing: {}</h2>
            <form action="/save/{}" method="post" class="edit-form">
                <textarea name="content" rows="20" class="edit-textarea">{}</textarea>
                <div class="form-actions">
                    <button type="submit" class="primary-btn">Save Article</button>
                    <a href="/wiki/{}" class="btn">Cancel</a>
                </div>
            </form>
            <div class="help-box">
                <h3>Formatting Help</h3>
                <ul>
                    <li>[[Link]] - Create internal links</li>
                    <li>Paragraphs separated by blank lines</li>
                    <li>Simple markdown supported</li>
                </ul>
            </div>
        </article>
    </main>
</body>
</html>"#,
        name, name, name, escape_html(&content), name
    ))
}


fn history_page(name: &str, wiki_data: WikiData) -> String {
    let data = wiki_data.lock().unwrap();

    match data.get(name) {
        Some(history) => {
            let mut versions_html = String::new();
            for (i, version) in history.versions.iter().enumerate().rev() {
                let date = format_timestamp(version.timestamp);
                versions_html.push_str(&format!(
                    r#"<div class="history-item">
                        <div class="history-number">Version {}</div>
                        <div class="history-date">{}</div>
                        <div class="history-preview">{}</div>
                    </div>"#,
                    i + 1,
                    date,
                    escape_html(&version.content.chars().take(100).collect::<String>())
                ));
            }

            html_response(&format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>History: {} - BauhausWiki</title>
    <link rel="stylesheet" href="/styles.css">
</head>
<body>
    <header>
        <h1><a href="/">BauhausWiki</a></h1>
    </header>
    <main>
        <article>
            <h2>Revision History: {}</h2>
            <div class="history-list">
                {}
            </div>
            <a href="/wiki/{}" class="btn">Back to Article</a>
        </article>
    </main>
</body>
</html>"#,
                name, name, versions_html, name
            ))
        }
        None => redirect_response(&format!("/wiki/{}", name)),
    }
}


fn search_page(query: &str, wiki_data: WikiData) -> String {
    let data = wiki_data.lock().unwrap();
    let mut results = Vec::new();

    let query_lower = query.to_lowercase();
    for (name, history) in data.iter() {
        let article = history.versions.last().unwrap();
        if article.title.to_lowercase().contains(&query_lower)
            || article.content.to_lowercase().contains(&query_lower) {
            results.push((name.clone(), article.title.clone()));
        }
    }

    let results_html = if results.is_empty() {
        "<p>No articles found.</p>".to_string()
    } else {
        results.iter()
            .map(|(name, title)| format!(r#"<div class="search-result"><a href="/wiki/{}">{}</a></div>"#, name, title))
            .collect::<Vec<_>>()
            .join("\n")
    };

    html_response(&format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Search: {} - BauhausWiki</title>
    <link rel="stylesheet" href="/styles.css">
</head>
<body>
    <header>
        <h1><a href="/">BauhausWiki</a></h1>
    </header>
    <main>
        <article>
            <h2>Search Results for: {}</h2>
            <div class="search-results">
                {}
            </div>
            <a href="/" class="btn">Back to Main Page</a>
        </article>
    </main>
</body>
</html>"#,
        query, escape_html(query), results_html
    ))
}


fn save_article(name: &str, content: &str, wiki_data: WikiData) {
    let mut data = wiki_data.lock().unwrap();
    let title = name.replace("_", " ").split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let article = Article {
        title,
        content: content.to_string(),
        timestamp: timestamp(),
    };

    data.entry(name.to_string())
        .or_insert_with(|| ArticleHistory { versions: Vec::new() })
        .versions
        .push(article);
}


fn markdown_to_html(content: &str) -> String {
    let mut html = String::new();
    let mut in_paragraph = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if in_paragraph {
                html.push_str("</p>\n");
                in_paragraph = false;
            }
            continue;
        }

        if trimmed.starts_with("# ") {
            if in_paragraph { html.push_str("</p>\n"); in_paragraph = false; }
            html.push_str(&format!("<h2>{}</h2>\n", escape_html(&trimmed[2..])));
        } else if trimmed.starts_with("- ") {
            if in_paragraph { html.push_str("</p>\n"); in_paragraph = false; }
            html.push_str(&format!("<ul><li>{}</li></ul>\n", process_links(&trimmed[2..])));
        } else {
            if !in_paragraph {
                html.push_str("<p>");
                in_paragraph = true;
            } else {
                html.push(' ');
            }
            html.push_str(&process_links(trimmed));
        }
    }

    if in_paragraph {
        html.push_str("</p>\n");
    }

    html
}


fn process_links(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '[' && chars.peek() == Some(&'[') {
            chars.next();
            let mut link = String::new();
            while let Some(c) = chars.next() {
                if c == ']' && chars.peek() == Some(&']') {
                    chars.next();
                    let link_url = link.to_lowercase().replace(" ", "_");
                    result.push_str(&format!(r#"<a href="/wiki/{}" class="wiki-link">{}</a>"#, link_url, escape_html(&link)));
                    break;
                }
                link.push(c);
            }
        } else {
            result.push(c);
        }
    }

    result
}


fn css_response() -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/css\r\n\r\n{}",
        r#"* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: 'Helvetica Neue', Arial, sans-serif;
    background: #ffffff;
    color: #000000;
    line-height: 1.6;
}

header {
    background: #000000;
    color: #ffffff;
    padding: 1.5rem 2rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 4px solid #ff0000;
}

header h1 {
    font-size: 1.8rem;
    font-weight: 700;
    letter-spacing: -1px;
}

header h1 a {
    color: #ffffff;
    text-decoration: none;
}

nav {
    display: flex;
    gap: 1rem;
    align-items: center;
}

.search-form {
    display: flex;
    gap: 0.5rem;
}

.search-input {
    padding: 0.5rem 1rem;
    border: 2px solid #ffffff;
    background: #000000;
    color: #ffffff;
    font-size: 1rem;
}

main {
    max-width: 900px;
    margin: 2rem auto;
    padding: 0 2rem;
}

article {
    background: #ffffff;
    border: 2px solid #000000;
    padding: 2rem;
}

.article-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 2rem;
    padding-bottom: 1rem;
    border-bottom: 2px solid #000000;
}

.article-header h2 {
    font-size: 2rem;
    font-weight: 700;
}

.article-actions {
    display: flex;
    gap: 0.5rem;
}

.article-content {
    font-size: 1.1rem;
}

.article-content h2 {
    font-size: 1.5rem;
    margin: 1.5rem 0 1rem 0;
    font-weight: 700;
}

.article-content p {
    margin-bottom: 1rem;
}

.article-content ul {
    margin-left: 2rem;
    margin-bottom: 1rem;
}

.wiki-link {
    color: #0000ff;
    text-decoration: none;
    font-weight: 500;
    border-bottom: 1px solid #0000ff;
}

.wiki-link:hover {
    background: #ffff00;
    border-bottom: 2px solid #0000ff;
}

.btn, .primary-btn {
    padding: 0.5rem 1.5rem;
    text-decoration: none;
    font-weight: 600;
    display: inline-block;
    cursor: pointer;
    border: 2px solid #000000;
    background: #ffffff;
    color: #000000;
    font-size: 1rem;
}

.btn:hover {
    background: #ffff00;
}

.primary-btn {
    background: #ff0000;
    color: #ffffff;
    border-color: #ff0000;
}

.primary-btn:hover {
    background: #cc0000;
}

.edit-form {
    margin-bottom: 2rem;
}

.edit-textarea {
    width: 100%;
    padding: 1rem;
    font-family: 'Courier New', monospace;
    font-size: 1rem;
    border: 2px solid #000000;
    resize: vertical;
}

.form-actions {
    margin-top: 1rem;
    display: flex;
    gap: 0.5rem;
}

.help-box {
    background: #f5f5f5;
    border: 2px solid #000000;
    padding: 1rem;
    margin-top: 2rem;
}

.help-box h3 {
    margin-bottom: 0.5rem;
    font-size: 1.2rem;
}

.help-box ul {
    list-style-position: inside;
}

.not-found {
    text-align: center;
    padding: 3rem 0;
}

.not-found h2 {
    font-size: 2rem;
    margin-bottom: 1rem;
}

.not-found p {
    margin-bottom: 2rem;
    font-size: 1.1rem;
}

.history-list {
    margin: 2rem 0;
}

.history-item {
    padding: 1rem;
    border: 2px solid #000000;
    margin-bottom: 1rem;
    background: #f5f5f5;
}

.history-number {
    font-weight: 700;
    font-size: 1.2rem;
    margin-bottom: 0.5rem;
}

.history-date {
    color: #666666;
    margin-bottom: 0.5rem;
}

.history-preview {
    font-family: 'Courier New', monospace;
    font-size: 0.9rem;
}

.search-results {
    margin: 2rem 0;
}

.search-result {
    padding: 1rem;
    border: 2px solid #000000;
    margin-bottom: 1rem;
    background: #ffffff;
}

.search-result a {
    font-size: 1.2rem;
    color: #0000ff;
    text-decoration: none;
    font-weight: 600;
}

.search-result a:hover {
    text-decoration: underline;
}

footer {
    text-align: center;
    padding: 2rem;
    background: #000000;
    color: #ffffff;
    margin-top: 4rem;
    border-top: 4px solid #ff0000;
}
"#
    )
}


fn http_response(code: u16, message: &str) -> String {
    format!("HTTP/1.1 {} {}\r\n\r\n{}", code, message, message)
}


fn html_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n{}",
        body
    )
}


fn redirect_response(location: &str) -> String {
    format!(
        "HTTP/1.1 303 See Other\r\nLocation: {}\r\n\r\n",
        location
    )
}


fn extract_query_param(path: &str, param: &str) -> String {
    if let Some(query_start) = path.find('?') {
        let query = &path[query_start + 1..];
        for pair in query.split('&') {
            let parts: Vec<&str> = pair.split('=').collect();
            if parts.len() == 2 && parts[0] == param {
                return urldecode(parts[1]);
            }
        }
    }
    String::new()
}


fn extract_body(request: &str) -> String {
    if let Some(pos) = request.find("\r\n\r\n") {
        return request[pos + 4..].to_string();
    }
    String::new()
}


fn extract_form_param(body: &str, param: &str) -> String {
    for pair in body.split('&') {
        let parts: Vec<&str> = pair.split('=').collect();
        if parts.len() == 2 && parts[0] == param {
            return urldecode(parts[1]);
        }
    }
    String::new()
}


fn urldecode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '+' => result.push(' '),
            '%' => {
                let mut hex = String::new();
                hex.push(chars.next().unwrap_or('0'));
                hex.push(chars.next().unwrap_or('0'));
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                }
            }
            _ => result.push(c),
        }
    }
    result
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn format_timestamp(ts: u64) -> String {
    let days = ts / 86400;
    let years = days / 365 + 1970;
    let remaining_days = days % 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    format!("{:04}-{:02}-{:02}", years, month, day)
}
