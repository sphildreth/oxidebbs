use std::path::{Path, PathBuf};

use thiserror::Error;

/// FTN inbound file category selected from the filename extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleFormat {
    /// An uncompressed FTN packet that can be handed directly to `PacketReader`.
    RawPacket,
    /// A ZIP-format arcmail bundle.
    ZipArcmail,
    /// An ARJ-format arcmail bundle.
    ArjArcmail,
}

impl BundleFormat {
    /// Returns true when the file is an archive and requires decompression
    /// before packet parsing.
    #[must_use]
    pub fn requires_extraction(self) -> bool {
        !matches!(self, Self::RawPacket)
    }
}

/// Classified FTN bundle or raw packet filename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleClassification {
    pub path: PathBuf,
    pub format: BundleFormat,
}

/// Errors from bundle classification and extraction-boundary code.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BundleError {
    #[error("bundle path has no filename: {path}")]
    MissingFilename { path: PathBuf },

    #[error("bundle filename has no extension: {path}")]
    MissingExtension { path: PathBuf },

    #[error("unsupported FTN bundle extension {extension:?} for {path}")]
    UnsupportedExtension { path: PathBuf, extension: String },

    #[error("extraction for {format:?} is not implemented yet: {path}")]
    UnsupportedExtraction { path: PathBuf, format: BundleFormat },
}

/// Classify a raw FTN packet or arcmail bundle by filename extension.
///
/// The current boundary recognizes only unambiguous suffixes:
///
/// - `.pkt` as a raw packet
/// - `.zip` as a ZIP arcmail bundle
/// - `.arj` as an ARJ arcmail bundle
///
/// # Errors
///
/// Returns a typed error for paths without a filename, paths without an
/// extension, and unsupported extensions.
pub fn classify_bundle_path(path: impl AsRef<Path>) -> Result<BundleClassification, BundleError> {
    let path = path.as_ref();
    let path_buf = path.to_path_buf();
    let file_name = path
        .file_name()
        .ok_or_else(|| BundleError::MissingFilename {
            path: path_buf.clone(),
        })?;
    if file_name.is_empty() {
        return Err(BundleError::MissingFilename { path: path_buf });
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .ok_or_else(|| BundleError::MissingExtension {
            path: path_buf.clone(),
        })?;
    let normalized_extension = extension.to_ascii_lowercase();
    let format = match normalized_extension.as_str() {
        "pkt" => BundleFormat::RawPacket,
        "zip" => BundleFormat::ZipArcmail,
        "arj" => BundleFormat::ArjArcmail,
        _ => {
            return Err(BundleError::UnsupportedExtension {
                path: path_buf,
                extension: normalized_extension,
            });
        }
    };

    Ok(BundleClassification {
        path: path_buf,
        format,
    })
}

/// Extraction boundary for inbound FTN packet and arcmail files.
pub struct BundleExtractor;

impl BundleExtractor {
    /// Return packet paths for a raw `.pkt`, or an explicit unsupported error for
    /// compressed bundles until decompression is implemented.
    ///
    /// # Errors
    ///
    /// Returns classification errors for unsupported filenames. Returns
    /// `BundleError::UnsupportedExtraction` for ZIP and ARJ bundles.
    pub fn extract_packets(
        input_path: impl AsRef<Path>,
        _output_dir: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>, BundleError> {
        let classification = classify_bundle_path(input_path)?;
        match classification.format {
            BundleFormat::RawPacket => Ok(vec![classification.path]),
            BundleFormat::ZipArcmail | BundleFormat::ArjArcmail => {
                Err(BundleError::UnsupportedExtraction {
                    path: classification.path,
                    format: classification.format,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_raw_pkt_case_insensitively() {
        let classification =
            classify_bundle_path("inbound/00000001.PKT").expect("classify raw packet");

        assert_eq!(classification.path, PathBuf::from("inbound/00000001.PKT"));
        assert_eq!(classification.format, BundleFormat::RawPacket);
        assert!(!classification.format.requires_extraction());
    }

    #[test]
    fn classifies_zip_arcmail_case_insensitively() {
        let classification =
            classify_bundle_path("inbound/00112233.Zip").expect("classify zip bundle");

        assert_eq!(classification.format, BundleFormat::ZipArcmail);
        assert!(classification.format.requires_extraction());
    }

    #[test]
    fn classifies_arj_arcmail_case_insensitively() {
        let classification =
            classify_bundle_path("inbound/00112233.ARJ").expect("classify arj bundle");

        assert_eq!(classification.format, BundleFormat::ArjArcmail);
        assert!(classification.format.requires_extraction());
    }

    #[test]
    fn rejects_missing_extension() {
        let error = classify_bundle_path("inbound/00112233").expect_err("missing extension");

        assert_eq!(
            error,
            BundleError::MissingExtension {
                path: PathBuf::from("inbound/00112233")
            }
        );
    }

    #[test]
    fn rejects_unsupported_extension() {
        let error = classify_bundle_path("inbound/00112233.rar").expect_err("unsupported format");

        assert_eq!(
            error,
            BundleError::UnsupportedExtension {
                path: PathBuf::from("inbound/00112233.rar"),
                extension: "rar".to_string()
            }
        );
    }

    #[test]
    fn extraction_boundary_passes_through_raw_pkt() {
        let packets = BundleExtractor::extract_packets("inbound/00000001.pkt", "temp")
            .expect("raw packet pass-through");

        assert_eq!(packets, vec![PathBuf::from("inbound/00000001.pkt")]);
    }

    #[test]
    fn extraction_boundary_rejects_zip_with_explicit_error() {
        let error = BundleExtractor::extract_packets("inbound/00112233.zip", "temp")
            .expect_err("zip extraction unsupported");

        assert_eq!(
            error,
            BundleError::UnsupportedExtraction {
                path: PathBuf::from("inbound/00112233.zip"),
                format: BundleFormat::ZipArcmail,
            }
        );
    }

    #[test]
    fn extraction_boundary_rejects_arj_with_explicit_error() {
        let error = BundleExtractor::extract_packets("inbound/00112233.arj", "temp")
            .expect_err("arj extraction unsupported");

        assert_eq!(
            error,
            BundleError::UnsupportedExtraction {
                path: PathBuf::from("inbound/00112233.arj"),
                format: BundleFormat::ArjArcmail,
            }
        );
    }
}
