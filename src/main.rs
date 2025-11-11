use clap::{Parser, ValueEnum};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
    Terminal,
};
use ratatui_image::{Image, Resize, picker::Picker};
use std::{fs, io, path::PathBuf};

/// CLI arguments for the slides application
#[derive(Parser, Debug)]
#[command(name = "slides")]
#[command(about = "A terminal-based presentation tool", long_about = None)]
struct Args {
    /// Markdown file to present
    #[arg(value_name = "FILE")]
    markdown_file: PathBuf,

    /// Heading style to use
    #[arg(short, long, value_enum, default_value_t = HeadingStyle::Classic)]
    style: HeadingStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
enum HeadingStyle {
    /// Classic style with arrows and underlines
    Classic,
    /// Bar style with colored backgrounds
    Bar,
    /// Block style with colored block characters
    Block,
}

#[derive(Clone, Debug)]
enum ImageSize {
    Auto,
    WidthOnly(u16),
    HeightOnly(u16),
    Both(u16, u16),
}

#[derive(Clone)]
enum ContentItem {
    Static(Line<'static>),
    Reveal(Line<'static>),
    Image(PathBuf, ImageSize),
    CenteredHeading(Line<'static>),
}

struct Slide {
    content: Vec<ContentItem>,
    total_reveals: usize,
    urls: Vec<String>,
}

struct Presentation {
    slides: Vec<Slide>,
    current_slide: usize,
    current_reveal: usize,
    #[allow(dead_code)]
    markdown_dir: PathBuf,
    image: Option<image::DynamicImage>,
    image_size: ImageSize,
    picker: Picker,
    heading_style: HeadingStyle,
}

impl Presentation {
    fn from_markdown(markdown: &str, markdown_path: &str, heading_style: HeadingStyle) -> Self {
        let mut slides = Vec::new();
        let mut current_items: Vec<ContentItem> = Vec::new();
        let mut current_urls: Vec<String> = Vec::new();
        let mut url_counter = 1;
        let markdown_dir = PathBuf::from(markdown_path)
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();

        // Pre-process to handle + reveals
        let lines: Vec<&str> = markdown.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            // Check for slide separator
            if line.trim() == "---" {
                if !current_items.is_empty() {
                    let total_reveals = current_items.iter()
                        .filter(|item| matches!(item, ContentItem::Reveal(_)))
                        .count();
                    slides.push(Slide {
                        content: current_items.clone(),
                        total_reveals,
                        urls: current_urls.clone(),
                    });
                    current_items.clear();
                    current_urls.clear();
                    url_counter = 1; // Reset counter for new slide
                }
                i += 1;
                continue;
            }

            // Skip empty lines at the beginning of a slide
            if current_items.is_empty() && line.trim().is_empty() {
                i += 1;
                continue;
            }

            // Check for image syntax: ![alt](path) or ![80x40](path)
            if let Some((img_path, size)) = parse_image_syntax(line) {
                let full_path = markdown_dir.join(img_path);
                current_items.push(ContentItem::Image(full_path, size));
            }
            // Check if line starts with +
            else if line.trim_start().starts_with('+') {
                let content = line.trim_start().strip_prefix('+').unwrap().trim_start();
                // Format as a bullet point
                let (formatted_line, urls, _is_heading) = if content.starts_with('-') || content.starts_with('*') {
                    // Already has a list marker
                    parse_line(content, &mut url_counter, heading_style)
                } else {
                    // Add bullet formatting
                    (Line::from(format!("  • {}", content)), Vec::new(), false)
                };
                current_urls.extend(urls);
                current_items.push(ContentItem::Reveal(formatted_line));
            } else {
                // Check for unsupported inline images
                if let Some((_pos, img_path)) = contains_inline_image(line) {
                    eprintln!("Error: Inline images are not supported");
                    eprintln!("  at line {}: {}", i + 1, line);
                    eprintln!("  Image path: {}", img_path);
                    eprintln!();
                    eprintln!("Images must be on their own line, not inline with text.");
                    eprintln!("Please move the image to a separate line:");
                    eprintln!("  Instead of: 'Text ![size](image.png) more text'");
                    eprintln!("  Use:        'Text'");
                    eprintln!("              '![size](image.png)'");
                    eprintln!("              'more text'");
                    std::process::exit(1);
                }

                let (line, urls, is_heading) = parse_line(line, &mut url_counter, heading_style);
                current_urls.extend(urls);
                if is_heading {
                    // Bar style needs CenteredHeading for alignment-based centering
                    // Classic/Block styles have padding already, use Static
                    if heading_style == HeadingStyle::Bar {
                        current_items.push(ContentItem::CenteredHeading(line));
                    } else {
                        current_items.push(ContentItem::Static(line));
                    }
                    // Add an empty line after headings for spacing
                    current_items.push(ContentItem::Static(Line::from("")));
                } else {
                    current_items.push(ContentItem::Static(line));
                }
            }

            i += 1;
        }

        // Add last slide
        if !current_items.is_empty() {
            let total_reveals = current_items.iter()
                .filter(|item| matches!(item, ContentItem::Reveal(_)))
                .count();
            slides.push(Slide {
                content: current_items,
                total_reveals,
                urls: current_urls,
            });
        }

        if slides.is_empty() {
            slides.push(Slide {
                content: vec![ContentItem::Static(Line::from("Empty presentation"))],
                total_reveals: 0,
                urls: Vec::new(),
            });
        }

        let picker = Picker::from_query_stdio().expect("Failed to create image picker");

        Presentation {
            slides,
            current_slide: 0,
            current_reveal: 0,
            markdown_dir,
            image: None,
            image_size: ImageSize::Auto,
            picker,
            heading_style,
        }
    }

    fn next(&mut self) -> bool {
        let current_slide = &self.slides[self.current_slide];

        // If there are more reveals to show on current slide
        if self.current_reveal < current_slide.total_reveals {
            self.current_reveal += 1;
            true
        } else if self.current_slide < self.slides.len() - 1 {
            // Move to next slide
            self.current_slide += 1;
            self.current_reveal = 0;
            self.load_current_image();
            true
        } else {
            false
        }
    }

    fn previous(&mut self) -> bool {
        // If we're in the middle of reveals, go back one reveal
        if self.current_reveal > 0 {
            self.current_reveal -= 1;
            true
        } else if self.current_slide > 0 {
            // Move to previous slide and show all its reveals
            self.current_slide -= 1;
            self.current_reveal = self.slides[self.current_slide].total_reveals;
            self.load_current_image();
            true
        } else {
            false
        }
    }

    fn load_current_image(&mut self) {
        self.image = None;
        self.image_size = ImageSize::Auto;
        let slide = &self.slides[self.current_slide];

        for item in &slide.content {
            if let ContentItem::Image(path, size) = item {
                if let Ok(img) = image::open(path) {
                    self.image = Some(img);
                    self.image_size = size.clone();
                }
                break;
            }
        }
    }

    fn has_images(&self) -> bool {
        let slide = &self.slides[self.current_slide];
        slide.content.iter().any(|item| matches!(item, ContentItem::Image(_, _)))
    }
}

fn parse_line(text: &str, url_counter: &mut usize, heading_style: HeadingStyle) -> (Line<'static>, Vec<String>, bool) {
    let trimmed = text.trim();

    // Calculate indentation level (count leading whitespace, treating tabs as 4 spaces)
    let leading_whitespace = &text[..text.len() - text.trim_start().len()];
    let indent_level: usize = leading_whitespace.chars().map(|c| {
        if c == '\t' { 4 } else { 1 }
    }).sum();

    // Heading detection
    if trimmed.starts_with("# ") {
        let content = trimmed.strip_prefix("# ").unwrap();

        match heading_style {
            HeadingStyle::Classic => {
                let content_upper = content.to_uppercase();
                let content_len = content_upper.len();
                let terminal_width = 80;
                let padding = if content_len < terminal_width {
                    " ".repeat((terminal_width - content_len) / 2)
                } else {
                    String::new()
                };
                (Line::from(vec![
                    Span::raw(padding),
                    Span::styled(
                        content_upper,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                            .add_modifier(Modifier::UNDERLINED)
                    )
                ]), Vec::new(), true)
            }
            HeadingStyle::Bar => {
                // Bar style with colored background - just the text, no centering padding
                let content_with_padding = format!("  {}  ", content);
                (Line::from(vec![
                    Span::styled(
                        content_with_padding,
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    )
                ]), Vec::new(), true)
            }
            HeadingStyle::Block => {
                // Block style with colored block characters (1 block for #)
                let full_content = format!("   ▒ {}", content.to_uppercase());
                (Line::from(vec![
                    Span::styled(
                        full_content,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    )
                ]), Vec::new(), true)
            }
        }
    } else if trimmed.starts_with("## ") {
        let content = trimmed.strip_prefix("## ").unwrap();

        match heading_style {
            HeadingStyle::Classic => {
                let full_content = format!("▸ {}", content);
                let content_len = full_content.len();
                let terminal_width = 80;
                let padding = if content_len < terminal_width {
                    " ".repeat((terminal_width - content_len) / 2)
                } else {
                    String::new()
                };
                (Line::from(vec![
                    Span::raw(padding),
                    Span::styled("▸ ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        content.to_string(),
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::BOLD)
                    )
                ]), Vec::new(), true)
            }
            HeadingStyle::Bar => {
                // Bar style with colored background - just the text, no centering padding
                let content_with_padding = format!("  {}  ", content);
                (Line::from(vec![
                    Span::styled(
                        content_with_padding,
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Blue)
                            .add_modifier(Modifier::BOLD)
                    )
                ]), Vec::new(), true)
            }
            HeadingStyle::Block => {
                // Block style with colored block characters (2 blocks for ##)
                let full_content = format!("   ▒▒ {}", content);
                (Line::from(vec![
                    Span::styled(
                        full_content,
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::BOLD)
                    )
                ]), Vec::new(), true)
            }
        }
    } else if trimmed.starts_with("### ") {
        let content = trimmed.strip_prefix("### ").unwrap();

        match heading_style {
            HeadingStyle::Classic => {
                let full_content = format!("  › {}", content);
                let content_len = full_content.len();
                let terminal_width = 80;
                let padding = if content_len < terminal_width {
                    " ".repeat((terminal_width - content_len) / 2)
                } else {
                    String::new()
                };
                (Line::from(vec![
                    Span::raw(padding),
                    Span::styled("  › ", Style::default().fg(Color::Magenta)),
                    Span::styled(
                        content.to_string(),
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::ITALIC)
                    )
                ]), Vec::new(), true)
            }
            HeadingStyle::Bar => {
                // Bar style with colored background - just the text, no centering padding
                let content_with_padding = format!("  {}  ", content);
                (Line::from(vec![
                    Span::styled(
                        content_with_padding,
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Magenta)
                            .add_modifier(Modifier::BOLD)
                    )
                ]), Vec::new(), true)
            }
            HeadingStyle::Block => {
                // Block style with colored block characters (3 blocks for ###)
                let full_content = format!("   ▒▒▒ {}", content);
                (Line::from(vec![
                    Span::styled(
                        full_content,
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD)
                    )
                ]), Vec::new(), true)
            }
        }
    } else if trimmed.starts_with("```") {
        (Line::from(trimmed.to_string())
            .style(Style::default().fg(Color::Green)), Vec::new(), false)
    } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        let content = if trimmed.starts_with("- ") {
            trimmed.strip_prefix("- ").unwrap()
        } else {
            trimmed.strip_prefix("* ").unwrap()
        };

        // Determine nesting level and bullet style
        // Assuming tab = 4 spaces, group into levels
        let level = indent_level / 4;
        let (bullet, base_indent) = match level {
            0 => ("• ", "  "),           // 2 spaces before bullet
            1 => ("◦ ", "      "),       // 6 spaces before bullet (2 + 4)
            _ => ("▪ ", "          "),   // 10 spaces before bullet (2 + 4 + 4)
        };

        // Parse inline formatting in list items
        if content.contains('`') || content.contains('[') {
            let (line, urls) = parse_inline_formatting(content, url_counter);
            // Prepend indentation and bullet point
            let mut spans = vec![Span::raw(format!("{}{}", base_indent, bullet))];
            spans.extend(line.spans);
            (Line::from(spans), urls, false)
        } else {
            (Line::from(format!("{}{}{}", base_indent, bullet, content)), Vec::new(), false)
        }
    } else if trimmed.starts_with('`') && trimmed.ends_with('`') && trimmed.len() > 1 {
        let content = trimmed.trim_matches('`');
        (Line::from(vec![
            Span::styled(content.to_string(), Style::default().fg(Color::Yellow))
        ]), Vec::new(), false)
    } else {
        // Check for inline code and links
        if trimmed.contains('`') || trimmed.contains('[') {
            let (line, urls) = parse_inline_formatting(trimmed, url_counter);
            (line, urls, false)
        } else {
            (Line::from(trimmed.to_string()), Vec::new(), false)
        }
    }
}

fn parse_inline_formatting(text: &str, url_counter: &mut usize) -> (Line<'static>, Vec<String>) {
    let mut spans = Vec::new();
    let mut urls = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    let mut in_code = false;

    while let Some(ch) = chars.next() {
        if ch == '`' {
            if in_code {
                // End code span
                spans.push(Span::styled(
                    current.clone(),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC)
                ));
                current.clear();
                in_code = false;
            } else {
                // Start code span
                if !current.is_empty() {
                    spans.push(Span::raw(current.clone()));
                    current.clear();
                }
                in_code = true;
            }
        } else if ch == '[' && !in_code {
            // Try to parse markdown link [text](url)
            let mut link_text = String::new();
            let mut found_close_bracket = false;

            // Collect link text
            for ch in chars.by_ref() {
                if ch == ']' {
                    found_close_bracket = true;
                    break;
                }
                link_text.push(ch);
            }

            if found_close_bracket && chars.peek() == Some(&'(') {
                chars.next(); // consume '('
                let mut url = String::new();
                let mut found_close_paren = false;

                // Collect URL
                for ch in chars.by_ref() {
                    if ch == ')' {
                        found_close_paren = true;
                        break;
                    }
                    url.push(ch);
                }

                if found_close_paren {
                    // Valid markdown link found
                    if !current.is_empty() {
                        spans.push(Span::raw(current.clone()));
                        current.clear();
                    }

                    // Add styled link text with underline and color
                    spans.push(Span::styled(
                        link_text,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::UNDERLINED)
                    ));

                    // Add reference number
                    spans.push(Span::styled(
                        format!("[{}]", *url_counter),
                        Style::default().fg(Color::DarkGray)
                    ));

                    // Store URL for later display
                    urls.push(url);
                    *url_counter += 1;
                } else {
                    // Not a valid link, add back what we collected
                    current.push('[');
                    current.push_str(&link_text);
                    if found_close_bracket {
                        current.push(']');
                        current.push('(');
                        current.push_str(&url);
                    }
                }
            } else {
                // Not a valid link, add back what we collected
                current.push('[');
                current.push_str(&link_text);
                if found_close_bracket {
                    current.push(']');
                }
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        if in_code {
            spans.push(Span::styled(
                current,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC)
            ));
        } else {
            spans.push(Span::raw(current));
        }
    }

    let line = if spans.is_empty() {
        Line::from(text.to_string())
    } else {
        Line::from(spans)
    };

    (line, urls)
}

fn parse_image_syntax(line: &str) -> Option<(&str, ImageSize)> {
    let trimmed = line.trim();
    if trimmed.starts_with("![") {
        if let Some(alt_end) = trimmed.find("](") {
            if let Some(path_end) = trimmed.find(')') {
                if path_end > alt_end + 2 {
                    let alt_text = &trimmed[2..alt_end];
                    let path = &trimmed[alt_end + 2..path_end];

                    // Parse size from alt text: ![80x40], ![80x], ![x40], ![width:height], etc.
                    let size = parse_image_size(alt_text);

                    return Some((path, size));
                }
            }
        }
    }
    None
}

fn contains_inline_image(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim();

    // Find image syntax pattern
    if let Some(start) = trimmed.find("![") {
        if let Some(alt_end) = trimmed[start..].find("](") {
            if let Some(path_end) = trimmed[start + alt_end..].find(')') {
                let full_end = start + alt_end + path_end + 1;

                // Check if there's text before or after the image
                let before = trimmed[..start].trim();
                let after = trimmed[full_end..].trim();

                if !before.is_empty() || !after.is_empty() {
                    // Extract the image path for error reporting
                    let img_start = start + alt_end + 2;
                    let img_end = start + alt_end + path_end;
                    let path = &trimmed[img_start..img_end];
                    return Some((start, path));
                }
            }
        }
    }
    None
}

fn parse_image_size(alt_text: &str) -> ImageSize {
    let trimmed = alt_text.trim();

    if trimmed.is_empty() {
        return ImageSize::Auto;
    }

    // Try to parse "80x40", "80x", or "x40" format
    if let Some(x_pos) = trimmed.find('x') {
        let width_str = trimmed[..x_pos].trim();
        let height_str = trimmed[x_pos + 1..].trim();

        return match (width_str.parse::<u16>(), height_str.parse::<u16>()) {
            (Ok(width), Ok(height)) => ImageSize::Both(width, height),
            (Ok(width), Err(_)) if height_str.is_empty() => ImageSize::WidthOnly(width),
            (Err(_), Ok(height)) if width_str.is_empty() => ImageSize::HeightOnly(height),
            _ => ImageSize::Auto,
        };
    }

    // Try to parse "width:height", "width:", or ":height" format
    if let Some(colon_pos) = trimmed.find(':') {
        let width_str = trimmed[..colon_pos].trim();
        let height_str = trimmed[colon_pos + 1..].trim();

        return match (width_str.parse::<u16>(), height_str.parse::<u16>()) {
            (Ok(width), Ok(height)) => ImageSize::Both(width, height),
            (Ok(width), Err(_)) if height_str.is_empty() => ImageSize::WidthOnly(width),
            (Err(_), Ok(height)) if width_str.is_empty() => ImageSize::HeightOnly(height),
            _ => ImageSize::Auto,
        };
    }

    // Try to parse single number as width
    if let Ok(width) = trimmed.parse::<u16>() {
        return ImageSize::WidthOnly(width);
    }

    ImageSize::Auto
}


fn render_slide(
    presentation: &mut Presentation,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> io::Result<()> {
    terminal.draw(|f| {
        let current_slide = &presentation.slides[presentation.current_slide];
        let has_images = current_slide.content.iter().any(|item| matches!(item, ContentItem::Image(_, _)));

        // Calculate space needed for URL references
        let url_lines = if current_slide.urls.is_empty() {
            0
        } else {
            current_slide.urls.len() as u16 + 1 // +1 for separator line
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),           // Main content
                Constraint::Length(url_lines), // URL references
                Constraint::Length(3)          // Footer
            ])
            .split(f.area());

        // Separate content into before-image and after-image sections
        let mut before_image = Vec::new();
        let mut after_image = Vec::new();
        let mut found_image = false;
        let mut reveals_shown = 0;

        for item in &current_slide.content {
            match item {
                ContentItem::Static(line) => {
                    if found_image {
                        after_image.push(line.clone());
                    } else {
                        before_image.push(line.clone());
                    }
                }
                ContentItem::CenteredHeading(line) => {
                    // Bar style: use alignment-based centering
                    // Classic/Block styles: use padding-based centering (already in the line)
                    let output_line = if presentation.heading_style == HeadingStyle::Bar {
                        line.clone().alignment(Alignment::Center)
                    } else {
                        line.clone()
                    };
                    if found_image {
                        after_image.push(output_line);
                    } else {
                        before_image.push(output_line);
                    }
                }
                ContentItem::Reveal(line) => {
                    if reveals_shown < presentation.current_reveal {
                        // Show the revealed content
                        if found_image {
                            after_image.push(line.clone());
                        } else {
                            before_image.push(line.clone());
                        }
                    } else {
                        // Reserve space for unrevealed items with empty lines
                        if found_image {
                            after_image.push(Line::from(""));
                        } else {
                            before_image.push(Line::from(""));
                        }
                    }
                    reveals_shown += 1;
                }
                ContentItem::Image(_, _) => {
                    found_image = true;
                    if presentation.image.is_none() {
                        before_image.push(Line::from("[Image failed to load]"));
                    }
                }
            }
        }

        // Layout: title/text, then image, then remaining content
        if has_images && presentation.image.is_some() {
            // First, calculate the actual image size we'll render
            if let Some(ref img) = presentation.image {
                // Calculate the space available for the image
                let available_height = chunks[0].height.saturating_sub(before_image.len() as u16);
                let available_width = chunks[0].width;

                // Determine the actual image dimensions based on size specification
                let (img_width, img_height) = match &presentation.image_size {
                    ImageSize::Auto => {
                        // Calculate size that fits the image proportionally
                        let img_w = img.width() as f32;
                        let img_h = img.height() as f32;
                        let aspect_ratio = img_h / img_w;
                        let term_adjusted_height = (available_width as f32 * aspect_ratio * 0.5).round() as u16;
                        let h = term_adjusted_height.min(available_height);
                        (available_width, h)
                    }
                    ImageSize::Both(width, height) => {
                        ((*width).min(available_width), (*height).min(available_height))
                    }
                    ImageSize::WidthOnly(width) => {
                        let w = (*width).min(available_width);
                        let img_w = img.width() as f32;
                        let img_h = img.height() as f32;
                        let aspect_ratio = img_h / img_w;
                        let h = ((w as f32) * aspect_ratio * 0.5).round() as u16;
                        (w, h.min(available_height))
                    }
                    ImageSize::HeightOnly(height) => {
                        let h = (*height).min(available_height);
                        let img_w = img.width() as f32;
                        let img_h = img.height() as f32;
                        let aspect_ratio = img_w / img_h;
                        let w = ((h as f32) * aspect_ratio * 2.0).round() as u16;
                        (w.min(available_width), h)
                    }
                };

                // Now create layout with exact image size
                let content_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(before_image.len() as u16),
                        Constraint::Length(img_height),
                        Constraint::Min(0),
                    ])
                    .split(chunks[0]);

                // Render title/content before image
                let title_text = Text::from(before_image);
                let title_paragraph = Paragraph::new(title_text)
                    .wrap(Wrap { trim: false })
                    .alignment(Alignment::Left);
                f.render_widget(title_paragraph, content_chunks[0]);

                // Render image
                let render_area = Rect {
                    x: content_chunks[1].x,
                    y: content_chunks[1].y,
                    width: img_width,
                    height: img_height,
                };

                if let Ok(protocol) = presentation.picker.new_protocol(img.clone(), render_area, Resize::Fit(None)) {
                    let image_widget = Image::new(&protocol);
                    f.render_widget(image_widget, render_area);
                }

                // Render content after image
                if !after_image.is_empty() {
                    let after_text = Text::from(after_image);
                    let after_paragraph = Paragraph::new(after_text)
                        .wrap(Wrap { trim: false })
                        .alignment(Alignment::Left);
                    f.render_widget(after_paragraph, content_chunks[2]);
                }
            }
        } else {
            // No image - render all content normally
            let all_content: Vec<_> = before_image.into_iter().chain(after_image.into_iter()).collect();
            let text = Text::from(all_content);
            let paragraph = Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .alignment(Alignment::Left);
            f.render_widget(paragraph, chunks[0]);
        }

        // Render URL references if present
        if !current_slide.urls.is_empty() {
            let mut url_lines = vec![Line::from("")]; // Separator line
            for (i, url) in current_slide.urls.iter().enumerate() {
                url_lines.push(Line::from(vec![
                    Span::styled(
                        format!("[{}] ", i + 1),
                        Style::default().fg(Color::DarkGray)
                    ),
                    Span::styled(
                        url.clone(),
                        Style::default().fg(Color::Cyan)
                    ),
                ]));
            }
            let url_text = Text::from(url_lines);
            let url_paragraph = Paragraph::new(url_text)
                .alignment(Alignment::Left);
            f.render_widget(url_paragraph, chunks[1]);
        }

        let reveal_info = if current_slide.total_reveals > 0 {
            format!(
                " [{}/{}]",
                presentation.current_reveal,
                current_slide.total_reveals
            )
        } else {
            String::new()
        };

        let footer = Paragraph::new(format!(
            "Slide {} / {}{}  |  [←/→] Navigate [q] Quit",
            presentation.current_slide + 1,
            presentation.slides.len(),
            reveal_info
        ))
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));

        f.render_widget(footer, chunks[2]);
    })?;

    Ok(())
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let markdown_content = fs::read_to_string(&args.markdown_file)?;
    let markdown_path = args.markdown_file.to_str().unwrap_or("unknown");
    let mut presentation = Presentation::from_markdown(&markdown_content, markdown_path, args.style);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Load the first slide's image if it has one
    presentation.load_current_image();

    loop {
        render_slide(&mut presentation, &mut terminal)?;

        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            match code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => {
                    presentation.next();
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    presentation.previous();
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line_simple_text() {
        let mut counter = 1;
        let (line, urls, is_heading) = parse_line("Simple text", &mut counter, HeadingStyle::Classic);
        assert_eq!(line.spans.len(), 1);
        assert_eq!(urls.len(), 0);
        assert_eq!(counter, 1); // Counter unchanged
        assert_eq!(is_heading, false);
    }

    #[test]
    fn test_parse_line_heading_level1() {
        let mut counter = 1;
        let (line, urls, is_heading) = parse_line("# Main Title", &mut counter, HeadingStyle::Classic);
        assert_eq!(line.spans.len(), 2); // Padding + styled content
        assert_eq!(urls.len(), 0);
        assert_eq!(is_heading, true);
        // Check the content is uppercase
        let content = &line.spans[1].content;
        assert!(content.contains("MAIN TITLE"));
    }

    #[test]
    fn test_parse_line_heading_level2() {
        let mut counter = 1;
        let (line, urls, is_heading) = parse_line("## Subtitle", &mut counter, HeadingStyle::Classic);
        assert_eq!(line.spans.len(), 3); // Padding + arrow + text
        assert_eq!(urls.len(), 0);
        assert_eq!(is_heading, true);
    }

    #[test]
    fn test_parse_line_heading_level3() {
        let mut counter = 1;
        let (line, urls, is_heading) = parse_line("### Subsubtitle", &mut counter, HeadingStyle::Classic);
        assert_eq!(line.spans.len(), 3); // Padding + arrow + text
        assert_eq!(urls.len(), 0);
        assert_eq!(is_heading, true);
    }

    #[test]
    fn test_parse_line_bullet_level0() {
        let mut counter = 1;
        let (line, urls, _is_heading) = parse_line("- Level 0 item", &mut counter, HeadingStyle::Classic);
        assert_eq!(urls.len(), 0);
        // Check bullet is present
        let content = line.to_string();
        assert!(content.contains("•"));
        assert!(content.contains("Level 0 item"));
    }

    #[test]
    fn test_parse_line_bullet_level1_with_tab() {
        let mut counter = 1;
        let (line, urls, _is_heading) = parse_line("\t- Level 1 item", &mut counter, HeadingStyle::Classic);
        assert_eq!(urls.len(), 0);
        // Check hollow bullet is used for level 1
        let content = line.to_string();
        assert!(content.contains("◦"));
        assert!(content.contains("Level 1 item"));
    }

    #[test]
    fn test_parse_line_bullet_level2_with_tabs() {
        let mut counter = 1;
        let (line, urls, _is_heading) = parse_line("\t\t- Level 2 item", &mut counter, HeadingStyle::Classic);
        assert_eq!(urls.len(), 0);
        // Check small square bullet is used for level 2+
        let content = line.to_string();
        assert!(content.contains("▪"));
        assert!(content.contains("Level 2 item"));
    }

    #[test]
    fn test_parse_line_bullet_with_asterisk() {
        let mut counter = 1;
        let (line, urls, _is_heading) = parse_line("* Asterisk bullet", &mut counter, HeadingStyle::Classic);
        assert_eq!(urls.len(), 0);
        let content = line.to_string();
        assert!(content.contains("•"));
        assert!(content.contains("Asterisk bullet"));
    }

    #[test]
    fn test_parse_line_inline_code() {
        let mut counter = 1;
        let (line, urls, _is_heading) = parse_line("Text with `code` inside", &mut counter, HeadingStyle::Classic);
        assert_eq!(urls.len(), 0);
        assert!(line.spans.len() >= 3); // Before code, code, after code
    }

    #[test]
    fn test_parse_line_single_url() {
        let mut counter = 1;
        let (line, urls, _is_heading) = parse_line("Check [this link](https://example.com)", &mut counter, HeadingStyle::Classic);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://example.com");
        assert_eq!(counter, 2); // Counter incremented
        // Check reference number is present
        let content = line.to_string();
        assert!(content.contains("[1]"));
        assert!(content.contains("this link"));
    }

    #[test]
    fn test_parse_line_multiple_urls() {
        let mut counter = 1;
        let (line, urls, _is_heading) = parse_line("See [link1](https://a.com) and [link2](https://b.com)", &mut counter, HeadingStyle::Classic);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://a.com");
        assert_eq!(urls[1], "https://b.com");
        assert_eq!(counter, 3); // Counter incremented twice
        let content = line.to_string();
        assert!(content.contains("[1]"));
        assert!(content.contains("[2]"));
    }

    #[test]
    fn test_parse_line_bullet_with_url() {
        let mut counter = 1;
        let (line, urls, _is_heading) = parse_line("- Item with [link](https://example.com)", &mut counter, HeadingStyle::Classic);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://example.com");
        assert_eq!(counter, 2);
        let content = line.to_string();
        assert!(content.contains("•"));
        assert!(content.contains("[1]"));
    }

    #[test]
    fn test_parse_inline_formatting_plain_text() {
        let mut counter = 1;
        let (_line, urls) = parse_inline_formatting("plain text", &mut counter);
        assert_eq!(urls.len(), 0);
        assert_eq!(counter, 1);
    }

    #[test]
    fn test_parse_inline_formatting_with_code() {
        let mut counter = 1;
        let (line, urls) = parse_inline_formatting("text `code` more", &mut counter);
        assert_eq!(urls.len(), 0);
        assert!(line.spans.len() >= 3);
    }

    #[test]
    fn test_parse_inline_formatting_with_link() {
        let mut counter = 5; // Start at 5 to test counter continuation
        let (line, urls) = parse_inline_formatting("text [link](https://test.com) more", &mut counter);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://test.com");
        assert_eq!(counter, 6);
        let content = line.to_string();
        assert!(content.contains("[5]")); // Should use the starting counter value
    }

    #[test]
    fn test_parse_inline_formatting_code_and_link() {
        let mut counter = 1;
        let (_line, urls) = parse_inline_formatting("run `cargo build` see [docs](https://doc.com)", &mut counter);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://doc.com");
        assert_eq!(counter, 2);
    }

    #[test]
    fn test_parse_inline_formatting_link_inside_code_ignored() {
        let mut counter = 1;
        let (_line, urls) = parse_inline_formatting("`[not a link](url)`", &mut counter);
        // Links inside code blocks should not be parsed
        assert_eq!(urls.len(), 0);
        assert_eq!(counter, 1);
    }

    #[test]
    fn test_indentation_level_no_indent() {
        let text = "- No indent";
        let leading = &text[..text.len() - text.trim_start().len()];
        let level: usize = leading.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum();
        assert_eq!(level, 0);
    }

    #[test]
    fn test_indentation_level_one_tab() {
        let text = "\t- One tab";
        let leading = &text[..text.len() - text.trim_start().len()];
        let level: usize = leading.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum();
        assert_eq!(level, 4);
    }

    #[test]
    fn test_indentation_level_two_tabs() {
        let text = "\t\t- Two tabs";
        let leading = &text[..text.len() - text.trim_start().len()];
        let level: usize = leading.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum();
        assert_eq!(level, 8);
    }

    #[test]
    fn test_indentation_level_four_spaces() {
        let text = "    - Four spaces";
        let leading = &text[..text.len() - text.trim_start().len()];
        let level: usize = leading.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum();
        assert_eq!(level, 4);
    }

    #[test]
    fn test_url_counter_persistence() {
        let mut counter = 1;
        let (_, urls1, _) = parse_line("First [link](https://a.com)", &mut counter, HeadingStyle::Classic);
        assert_eq!(urls1.len(), 1);
        assert_eq!(counter, 2);

        let (_, urls2, _) = parse_line("Second [link](https://b.com)", &mut counter, HeadingStyle::Classic);
        assert_eq!(urls2.len(), 1);
        assert_eq!(counter, 3);
    }
}
