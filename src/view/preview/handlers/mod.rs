// Preview handlers module

mod archive;
mod audio;
mod binary;
mod directory;
mod image;
mod markdown;
mod office;
mod pdf;
mod text;

pub use archive::ArchivePreviewHandler;
pub use audio::AudioPreviewHandler;
pub use binary::BinaryPreviewHandler;
pub use directory::DirectoryPreviewHandler;
pub use image::ImagePreviewHandler;
pub use markdown::MarkdownPreviewHandler;
pub use office::OfficePreviewHandler;
pub use pdf::PdfPreviewHandler;
pub use text::TextPreviewHandler;
