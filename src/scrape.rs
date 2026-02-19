use html5ever::ParseOpts;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

/// Converts HTML to markdown by walking the DOM and extracting text with basic formatting.
///
/// # Panics
///
/// Panics if the HTML cannot be parsed (e.g. invalid UTF-8 or malformed HTML).
pub fn html_to_markdown(html: &str) -> String {
    let dom = parse_document(RcDom::default(), ParseOpts::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap();

    let mut out = String::with_capacity(html.len());
    if let Some(main) = find_element_recursive(&dom.document, "main") {
        tracing::info!("Found main content");
        walk(&main, &mut out);
    } else if let Some(body) = find_element_recursive(&dom.document, "body") {
        tracing::info!("No main content found, walking body");
        walk(&body, &mut out);
    } else {
        tracing::warn!("No main content or body found, walking document");
        walk(&dom.document, &mut out);
    }
    out
}

fn find_element_recursive(node: &Handle, tag: &str) -> Option<Handle> {
    match &node.data {
        NodeData::Element { name, .. } if name.local.as_ref() == tag => {
            return Some(Handle::clone(node));
        }
        _ => {}
    }
    for child in node.children.borrow().iter() {
        if let Some(found) = find_element_recursive(child, tag) {
            return Some(found);
        }
    }
    None
}

fn walk(node: &Handle, out: &mut String) {
    let tag = match &node.data {
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_bytes();
            let tag = unsafe { std::str::from_utf8_unchecked(tag) };

            if matches!(
                tag,
                "header"
                    | "nav"
                    | "script"
                    | "style"
                    | "aside"
                    | "footer"
                    | "iframe"
                    | "svg"
                    | "noscript"
            ) {
                return;
            }

            // Skip by attributes (role, aria-hidden)
            for attr in attrs.borrow().iter() {
                let attr_name = attr.name.local.as_ref();
                let value = attr.value.to_lowercase();
                if attr_name == "role"
                    && matches!(
                        value.as_str(),
                        "button"
                            | "navigation"
                            | "banner"
                            | "presentation"
                            | "complementary"
                            | "contentinfo"
                            | "menu"
                            | "menubar"
                            | "menuitem"
                    )
                {
                    return;
                }
                if attr_name == "aria-hidden" && (value == "true" || value == "1") {
                    return;
                }
            }
            tag
        }
        NodeData::Text { contents } => {
            out.push_str(contents.borrow().trim());
            return;
        }
        _ => {
            for child in node.children.borrow().iter() {
                walk(child, out);
            }
            return;
        }
    };

    match tag {
        "h1" => out.push_str("# "),
        "h2" => out.push_str("## "),
        "h3" => out.push_str("### "),
        "h4" => out.push_str("#### "),
        "h5" => out.push_str("##### "),
        "h6" => out.push_str("###### "),
        "strong" | "b" => out.push_str("**"),
        "em" | "i" => out.push('*'),
        "code" => out.push('`'),
        "li" => out.push_str("- "),
        "pre" => out.push_str("```\n"),
        "hr" => {
            out.push_str("---\n");
            return;
        }
        "br" => {
            out.push('\n');
            return;
        }
        _ => {}
    }

    for child in node.children.borrow().iter() {
        walk(child, out);
    }

    // Suffix
    match tag {
        "strong" | "b" => out.push_str("**"),
        "em" | "i" => out.push('*'),
        "code" => out.push('`'),
        "pre" => out.push_str("```\n"),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "li" | "blockquote" => out.push_str("\n\n"),
        "div" => out.push('\n'),
        _ => {}
    }
}
