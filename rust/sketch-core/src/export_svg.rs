use std::fmt::Write;

use usvg::roxmltree;

use crate::document::format_number;
use crate::{
    outline_stroke, parse_background, BackgroundError, Color, Document, PathData, VERB_CLOSE,
    VERB_CUBIC, VERB_LINE, VERB_MOVE, VERB_QUAD,
};

/// Root attributes replaced when nesting a background, so the nested `<svg>` fills the document
/// and its default `preserveAspectRatio` performs the aspect-fit.
const REPLACED_BACKGROUND_ATTRIBUTES: [&str; 5] =
    ["x", "y", "width", "height", "preserveAspectRatio"];

/// Exported background markup beyond this is rejected; entity expansion can amplify input.
const MAX_NESTED_BACKGROUND_BYTES: usize = 32 * 1024 * 1024;

/// A standalone SVG of the document: strokes as filled outlines, over the optional background.
pub fn export_svg(doc: &Document, background: Option<&str>) -> Result<String, BackgroundError> {
    let (width, height) = (format_number(doc.width), format_number(doc.height));
    let mut out = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    );
    if let Some(svg) = background {
        out.push_str(&nested_background(svg, &width, &height)?);
    }
    for stroke in &doc.strokes {
        let color = stroke.color;
        let _ = write!(
            out,
            r#"<path d="{}" fill="{}""#,
            path_d(&outline_stroke(stroke)),
            Color {
                a: u8::MAX,
                ..color
            }
            .to_hex()
        );
        if color.a < u8::MAX {
            let _ = write!(
                out,
                r#" fill-opacity="{}""#,
                format_number(f64::from(color.a) / f64::from(u8::MAX))
            );
        }
        out.push_str("/>");
    }
    out.push_str("</svg>");
    Ok(out)
}

/// Re-serialises the background's root element from the parsed tree, so entity references
/// arrive expanded and no DTD, prolog or comment is carried over.
fn nested_background(svg: &str, width: &str, height: &str) -> Result<String, BackgroundError> {
    let invalid = |message: String| BackgroundError::Invalid { message };
    let background = parse_background(svg)?;
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let xml = roxmltree::Document::parse_with_options(svg, options)
        .map_err(|e| invalid(e.to_string()))?;
    let root = xml.root_element();
    let mut fitted = format!(r#" x="0" y="0" width="{width}" height="{height}""#);
    if root.attribute("viewBox").is_none() {
        let _ = write!(
            fitted,
            r#" viewBox="0 0 {} {}""#,
            format_number(background.width),
            format_number(background.height)
        );
    }
    let mut out = String::with_capacity(svg.len());
    write_element(root, None, &fitted, &mut out);
    if out.len() > MAX_NESTED_BACKGROUND_BYTES {
        return Err(invalid(format!(
            "background markup exceeds {MAX_NESTED_BACKGROUND_BYTES} bytes"
        )));
    }
    Ok(out)
}

/// Writes an element; `fitted` attributes replace the root's own position and size.
fn write_element(
    node: roxmltree::Node,
    parent: Option<roxmltree::Node>,
    fitted: &str,
    out: &mut String,
) {
    let is_root = parent.is_none();
    let tag = qualified_name(node, node.tag_name().namespace(), node.tag_name().name());
    let _ = write!(out, "<{tag}");
    for namespace in node.namespaces() {
        let inherited = parent.is_some_and(|p| p.namespaces().any(|ns| ns == namespace));
        if inherited || namespace.uri() == roxmltree::NS_XML_URI {
            continue;
        }
        match namespace.name() {
            Some(prefix) => write_attribute(out, &format!("xmlns:{prefix}"), namespace.uri()),
            None => write_attribute(out, "xmlns", namespace.uri()),
        }
    }
    out.push_str(fitted);
    for attribute in node.attributes() {
        let replaced = is_root
            && attribute.namespace().is_none()
            && REPLACED_BACKGROUND_ATTRIBUTES.contains(&attribute.name());
        if replaced {
            continue;
        }
        let name = match attribute.namespace() {
            None => attribute.name().to_string(),
            Some(uri) => match attribute_prefix(node, uri) {
                Some(prefix) => format!("{prefix}:{}", attribute.name()),
                None => continue,
            },
        };
        write_attribute(out, &name, attribute.value());
    }
    if !node.has_children() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for child in node.children() {
        if child.is_element() {
            write_element(child, Some(node), "", out);
        } else if let Some(text) = child.is_text().then(|| child.text()).flatten() {
            escape_into(out, text, false);
        }
    }
    let _ = write!(out, "</{tag}>");
}

/// A namespaced attribute needs a named prefix; well-formed XML always has one in scope.
fn attribute_prefix<'a>(node: roxmltree::Node<'a, 'a>, uri: &str) -> Option<&'a str> {
    if uri == roxmltree::NS_XML_URI {
        return Some("xml");
    }
    node.namespaces()
        .find(|ns| ns.uri() == uri && ns.name().is_some())
        .and_then(|ns| ns.name())
}

fn qualified_name(node: roxmltree::Node, namespace: Option<&str>, name: &str) -> String {
    match namespace
        .and_then(|uri| node.lookup_prefix(uri))
        .filter(|prefix| !prefix.is_empty())
    {
        Some(prefix) => format!("{prefix}:{name}"),
        None => name.to_string(),
    }
}

fn write_attribute(out: &mut String, name: &str, value: &str) {
    let _ = write!(out, r#" {name}=""#);
    escape_into(out, value, true);
    out.push('"');
}

fn escape_into(out: &mut String, text: &str, in_attribute: bool) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if in_attribute => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

fn path_d(path: &PathData) -> String {
    let mut d = String::with_capacity(path.coords.len() * 7);
    let mut coords = path.coords.iter().map(|&c| format_number(f64::from(c)));
    for &verb in &path.verbs {
        let (command, count) = match verb {
            VERB_MOVE => ('M', 2),
            VERB_LINE => ('L', 2),
            VERB_QUAD => ('Q', 4),
            VERB_CUBIC => ('C', 6),
            VERB_CLOSE => ('Z', 0),
            _ => continue,
        };
        d.push(command);
        for i in 0..count {
            if i > 0 {
                d.push(' ');
            }
            d.push_str(&coords.next().unwrap_or_default());
        }
    }
    d
}
