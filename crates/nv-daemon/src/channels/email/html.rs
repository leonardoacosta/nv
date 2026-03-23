/// Convert HTML email body to plain text.
///
/// Strips HTML tags, decodes common HTML entities, and preserves
/// paragraph/line breaks. This is a lightweight approach suitable
/// for typical email HTML without pulling in a full HTML parser.
pub fn html_to_text(html: &str) -> String {
    if html.is_empty() {
        return String::new();
    }

    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag_name = String::new();
    let mut capturing_tag = false;

    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        match ch {
            '<' => {
                in_tag = true;
                capturing_tag = true;
                tag_name.clear();
                i += 1;
            }
            '>' if in_tag => {
                in_tag = false;
                capturing_tag = false;

                // Insert newlines for block-level elements
                let tag_lower = tag_name.to_ascii_lowercase();
                let tag_base = tag_lower
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('/');

                match tag_base {
                    "br" | "br/" => {
                        result.push('\n');
                    }
                    "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                        if !tag_lower.starts_with('/') =>
                    {
                        if !result.is_empty() && !result.ends_with('\n') {
                            result.push('\n');
                        }
                    }
                    "p" | "div" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                        if tag_lower.starts_with('/') =>
                    {
                        result.push('\n');
                    }
                    _ => {}
                }

                i += 1;
            }
            _ if in_tag => {
                if capturing_tag {
                    if ch.is_whitespace() || ch == '/' {
                        if ch == '/' && tag_name.is_empty() {
                            // Closing tag: capture the slash
                            tag_name.push(ch);
                        } else if ch == '/' {
                            // Self-closing tag like <br/>
                            tag_name.push(ch);
                        } else {
                            capturing_tag = false;
                        }
                    } else {
                        tag_name.push(ch);
                    }
                }
                i += 1;
            }
            '&' => {
                // Decode HTML entities
                if let Some(end) = chars[i..].iter().position(|&c| c == ';') {
                    let entity: String = chars[i..i + end + 1].iter().collect();
                    match entity.as_str() {
                        "&amp;" => result.push('&'),
                        "&lt;" => result.push('<'),
                        "&gt;" => result.push('>'),
                        "&nbsp;" => result.push(' '),
                        "&quot;" => result.push('"'),
                        "&apos;" | "&#39;" => result.push('\''),
                        "&#160;" => result.push(' '),
                        _ if entity.starts_with("&#x") => {
                            // Hex numeric entity
                            let hex = &entity[3..entity.len() - 1];
                            if let Ok(code) = u32::from_str_radix(hex, 16) {
                                if let Some(c) = char::from_u32(code) {
                                    result.push(c);
                                }
                            }
                        }
                        _ if entity.starts_with("&#") => {
                            // Decimal numeric entity
                            let num = &entity[2..entity.len() - 1];
                            if let Ok(code) = num.parse::<u32>() {
                                if let Some(c) = char::from_u32(code) {
                                    result.push(c);
                                }
                            }
                        }
                        _ => {
                            // Unknown entity, pass through
                            result.push_str(&entity);
                        }
                    }
                    i += end + 1;
                } else {
                    result.push('&');
                    i += 1;
                }
            }
            _ => {
                result.push(ch);
                i += 1;
            }
        }
    }

    // Collapse multiple consecutive newlines into at most two (paragraph break)
    let mut collapsed = String::with_capacity(result.len());
    let mut newline_count = 0;
    for ch in result.chars() {
        if ch == '\n' {
            newline_count += 1;
            if newline_count <= 2 {
                collapsed.push('\n');
            }
        } else {
            newline_count = 0;
            collapsed.push(ch);
        }
    }

    collapsed.trim().to_string()
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert_eq!(html_to_text(""), "");
    }

    #[test]
    fn plain_text_passthrough() {
        assert_eq!(html_to_text("Hello world"), "Hello world");
    }

    #[test]
    fn basic_html_stripping() {
        assert_eq!(html_to_text("<p>Hello</p>"), "Hello");
    }

    #[test]
    fn nested_tags() {
        assert_eq!(
            html_to_text("<div><span>nested</span></div>"),
            "nested"
        );
    }

    #[test]
    fn bold_and_italic() {
        assert_eq!(
            html_to_text("<b>bold</b> and <i>italic</i>"),
            "bold and italic"
        );
    }

    #[test]
    fn entity_decoding_amp() {
        assert_eq!(html_to_text("foo &amp; bar"), "foo & bar");
    }

    #[test]
    fn entity_decoding_lt_gt() {
        assert_eq!(html_to_text("&lt;tag&gt;"), "<tag>");
    }

    #[test]
    fn entity_decoding_nbsp() {
        assert_eq!(html_to_text("hello&nbsp;world"), "hello world");
    }

    #[test]
    fn entity_decoding_quot() {
        assert_eq!(html_to_text("say &quot;hello&quot;"), "say \"hello\"");
    }

    #[test]
    fn numeric_entity_decimal() {
        assert_eq!(html_to_text("&#65;"), "A");
    }

    #[test]
    fn numeric_entity_hex() {
        assert_eq!(html_to_text("&#x41;"), "A");
    }

    #[test]
    fn br_tags_become_newlines() {
        assert_eq!(html_to_text("line1<br>line2"), "line1\nline2");
        assert_eq!(html_to_text("line1<br/>line2"), "line1\nline2");
        assert_eq!(html_to_text("line1<br />line2"), "line1\nline2");
    }

    #[test]
    fn paragraph_breaks_preserved() {
        let html = "<p>First paragraph</p><p>Second paragraph</p>";
        let text = html_to_text(html);
        assert!(text.contains("First paragraph"));
        assert!(text.contains("Second paragraph"));
        // Should have a newline between paragraphs
        assert!(text.contains('\n'));
    }

    #[test]
    fn div_breaks() {
        let html = "<div>Block one</div><div>Block two</div>";
        let text = html_to_text(html);
        assert!(text.contains("Block one\n"));
        assert!(text.contains("Block two"));
    }

    #[test]
    fn typical_email_html() {
        let html = r#"<html><body><p>Hi John,</p><p>Please review the attached document.</p><p>Thanks,<br>Jane</p></body></html>"#;
        let text = html_to_text(html);
        assert!(text.contains("Hi John,"));
        assert!(text.contains("Please review the attached document."));
        assert!(text.contains("Thanks,\nJane"));
    }

    #[test]
    fn multiple_newlines_collapsed() {
        let html = "<p>A</p>\n\n\n<p>B</p>";
        let text = html_to_text(html);
        // Should not have more than 2 consecutive newlines
        assert!(!text.contains("\n\n\n"));
    }

    #[test]
    fn heading_tags() {
        let html = "<h1>Title</h1><p>Content</p>";
        let text = html_to_text(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Content"));
    }

    #[test]
    fn non_breaking_space_entity() {
        assert_eq!(html_to_text("a&#160;b"), "a b");
    }

    #[test]
    fn apos_entity() {
        assert_eq!(html_to_text("it&apos;s"), "it's");
        assert_eq!(html_to_text("it&#39;s"), "it's");
    }
}
