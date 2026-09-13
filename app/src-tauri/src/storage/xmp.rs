// storage/xmp.rs — read Lightroom / Darktable / macOS XMP sidecars.
//
// XMP files live next to the photo as `<name>.xmp` or `<name>.jpg.xmp`.
// The relevant fields SnapIT surfaces in R·01 are:
//   • xmp:Rating          → 0..5 stars
//   • xmp:Label           → free text ("Red", "Blue", …)
//   • dc:description → x-default → caption
//   • dc:subject     → li* → hierarchical keywords
//
// R·01 uses a very tolerant string-extract instead of pulling in a full
// XML parser — real-world XMP written by Lightroom, Darktable and Adobe
// Bridge fits the same well-known shapes. We can swap in `quick-xml`
// later without changing the caller.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Serialize)]
pub struct XmpFields {
    pub rating: Option<i32>,
    pub label: Option<String>,
    pub caption: Option<String>,
    pub keywords: Option<String>, // CSV
}

/// Look for `<photo>.xmp` (Adobe convention) or `<photo.stem>.xmp` (Darktable).
pub fn find_sidecar(photo_path: &Path) -> Option<PathBuf> {
    // Adobe: `IMG_1234.jpg.xmp`
    let with_ext = {
        let mut p = photo_path.as_os_str().to_owned();
        p.push(".xmp");
        PathBuf::from(p)
    };
    if with_ext.exists() {
        return Some(with_ext);
    }
    // Darktable: `IMG_1234.xmp`
    if let Some(stem) = photo_path.file_stem() {
        let parent = photo_path.parent()?;
        let dt = parent.join(format!("{}.xmp", stem.to_string_lossy()));
        if dt.exists() {
            return Some(dt);
        }
    }
    None
}

pub fn read(photo_path: &Path) -> XmpFields {
    let Some(sidecar) = find_sidecar(photo_path) else {
        return XmpFields::default();
    };
    let text = match fs::read_to_string(&sidecar) {
        Ok(s) => s,
        Err(_) => return XmpFields::default(),
    };
    extract_from_text(&text)
}

fn extract_from_text(x: &str) -> XmpFields {
    let mut out = XmpFields::default();

    // Rating — attribute (`xmp:Rating="4"`) OR element (`<xmp:Rating>4</xmp:Rating>`).
    let rating_str: Option<String> = attr(x, "xmp:Rating")
        .map(|s| s.to_string())
        .or_else(|| element(x, "xmp:Rating"));
    if let Some(v) = rating_str {
        if let Ok(n) = v.trim().parse::<i32>() {
            if (0..=5).contains(&n) {
                out.rating = Some(n);
            }
        }
    }

    // Label
    out.label = attr(x, "xmp:Label")
        .map(|s| s.to_string())
        .or_else(|| element(x, "xmp:Label"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Caption / description
    if let Some(desc_block) = between(x, "<dc:description>", "</dc:description>") {
        // Nested rdf:Alt → li — grab first x-default
        if let Some(li) = between(desc_block, "<rdf:li", "</rdf:li>") {
            let inner = li.split_once('>').map(|(_, rest)| rest).unwrap_or(li).trim();
            if !inner.is_empty() {
                out.caption = Some(inner.to_string());
            }
        }
    }

    // Keywords / dc:subject
    if let Some(subj_block) = between(x, "<dc:subject>", "</dc:subject>") {
        let mut kws: Vec<String> = Vec::new();
        let mut rest = subj_block;
        while let Some((_, tail)) = rest.split_once("<rdf:li") {
            let end = match tail.find("</rdf:li>") { Some(i) => i, None => break };
            let li = &tail[..end];
            let value = li.split_once('>').map(|(_, r)| r).unwrap_or(li).trim();
            if !value.is_empty() {
                kws.push(value.to_string());
            }
            rest = &tail[end..];
        }
        if !kws.is_empty() {
            out.keywords = Some(kws.join(","));
        }
    }

    out
}

fn attr<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(r#"{}=""#, name);
    let (_, rest) = s.split_once(&needle)?;
    let (val, _) = rest.split_once('"')?;
    Some(val)
}

fn element(s: &str, name: &str) -> Option<String> {
    let open = format!("<{}>", name);
    let close = format!("</{}>", name);
    let (_, rest) = s.split_once(&open)?;
    let (val, _) = rest.split_once(close.as_str())?;
    Some(val.trim().to_string())
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let (_, after) = s.split_once(open)?;
    let (inside, _) = after.split_once(close)?;
    Some(inside)
}

/// Write-back path. If a sidecar exists, update xmp:Rating in place.
/// If none exists, create an Adobe-convention `<photo>.xmp` with the
/// minimum tree that Lightroom, Bridge and Darktable all recognise.
///
/// Only the rating is touched — we don't invent metadata we didn't
/// derive from the user.
pub fn write_rating(photo_path: &Path, rating: i32) -> anyhow::Result<PathBuf> {
    let rating = rating.clamp(0, 5);
    let sidecar = find_sidecar(photo_path).unwrap_or_else(|| adobe_sidecar_path(photo_path));

    let existing = fs::read_to_string(&sidecar).ok();
    let new_text = match existing {
        Some(text) => rewrite_rating(&text, rating),
        None => fresh_sidecar_with_rating(rating),
    };
    write_atomic(&sidecar, &new_text)?;
    Ok(sidecar)
}

fn adobe_sidecar_path(photo_path: &Path) -> PathBuf {
    let mut p = photo_path.as_os_str().to_owned();
    p.push(".xmp");
    PathBuf::from(p)
}

fn rewrite_rating(text: &str, rating: i32) -> String {
    // Try to replace an existing xmp:Rating attribute value.
    if let Some(idx) = text.find("xmp:Rating=\"") {
        let before = &text[..idx];
        let rest = &text[idx + "xmp:Rating=\"".len()..];
        if let Some(quote_end) = rest.find('"') {
            let after = &rest[quote_end..];
            return format!("{}xmp:Rating=\"{}{}", before, rating, after);
        }
    }
    // Try to replace an element form.
    if let (Some(open), Some(close)) = (text.find("<xmp:Rating>"), text.find("</xmp:Rating>")) {
        let (h, _) = text.split_at(open);
        let after = &text[close + "</xmp:Rating>".len()..];
        return format!("{}<xmp:Rating>{}</xmp:Rating>{}", h, rating, after);
    }
    // Otherwise inject the attribute onto the first rdf:Description tag.
    if let Some(idx) = text.find("<rdf:Description") {
        let insert_at = idx + "<rdf:Description".len();
        let (h, t) = text.split_at(insert_at);
        return format!("{} xmp:Rating=\"{}\"{}", h, rating, t);
    }
    // Give up — return input unchanged (very unusual case).
    text.to_string()
}

fn fresh_sidecar_with_rating(rating: i32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="SnapIT">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmp:Rating="{rating}"/>
 </rdf:RDF>
</x:xmpmeta>
"#
    )
}

fn write_atomic(target: &Path, contents: &str) -> anyhow::Result<()> {
    let tmp = target.with_extension(match target.extension().and_then(|s| s.to_str()) {
        Some(ext) => format!("{}.snapit-tmp", ext),
        None => "snapit-tmp".to_string(),
    });
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, target)?;
    Ok(())
}
