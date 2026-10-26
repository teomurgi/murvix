use super::Content;

/// An image, then text: file managers offer files' names as text.
pub fn read_content(cx: &gpui::App) -> Result<Content, String> {
    let Some(item) = cx.read_from_clipboard() else {
        return Ok(Content::Text);
    };
    for entry in item.entries() {
        if let gpui::ClipboardEntry::Image(image) = entry {
            if image.bytes().len() > 25 * 1024 * 1024 {
                return Err(crate::tr!("Copied image is larger than 25 MiB").to_string());
            }
            return Ok(Content::Image(image.bytes().to_vec()));
        }
    }
    Ok(Content::Text)
}

/// Whether a `Lease` can hand images to a terminal program and restore the clipboard.
pub const IMAGE_SUBMISSION: murvix_core::capability::Capability =
    murvix_core::capability::Capability::Unsupported;

#[derive(Debug)]
pub enum Lease {}

impl Lease {
    pub fn capture() -> Result<Self, String> {
        Err(murvix_core::capability::Feature::ClipboardImages
            .unsupported_reason()
            .into_owned())
    }

    pub fn write_png(&mut self, _png: Vec<u8>) -> Result<(), String> {
        match *self {}
    }
}
