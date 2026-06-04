use std::{
    collections::HashSet,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;
use zip::ZipArchive;

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

    #[error("bundle I/O failed while trying to {operation} {path}: {message}")]
    Io {
        path: PathBuf,
        operation: &'static str,
        message: String,
    },

    #[error("ZIP archive could not be read: {path}: {message}")]
    ZipArchive { path: PathBuf, message: String },

    #[error("ZIP archive contains no packet entries: {path}")]
    NoPacketEntries { path: PathBuf },

    #[error("unsupported ZIP entry {entry:?} in {path}: {reason}")]
    UnsupportedZipEntry {
        path: PathBuf,
        entry: String,
        reason: &'static str,
    },

    #[error("duplicate ZIP packet entry {entry:?} in {path}")]
    DuplicateZipPacketEntry { path: PathBuf, entry: String },

    #[error("extracted packet already exists for {path}: {output_path}")]
    ExtractedPacketExists { path: PathBuf, output_path: PathBuf },
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
    /// Return packet paths for a raw `.pkt`, extract packet entries from a ZIP
    /// bundle, or return an explicit unsupported error for ARJ bundles.
    ///
    /// # Errors
    ///
    /// Returns classification errors for unsupported filenames. Returns
    /// archive and I/O errors when ZIP extraction fails. Returns
    /// `BundleError::UnsupportedExtraction` for ARJ bundles.
    pub fn extract_packets(
        input_path: impl AsRef<Path>,
        output_dir: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>, BundleError> {
        let classification = classify_bundle_path(input_path)?;
        match classification.format {
            BundleFormat::RawPacket => Ok(vec![classification.path]),
            BundleFormat::ZipArcmail => {
                extract_zip_packets(&classification.path, output_dir.as_ref())
            }
            BundleFormat::ArjArcmail => Err(BundleError::UnsupportedExtraction {
                path: classification.path,
                format: classification.format,
            }),
        }
    }
}

#[derive(Debug, Clone)]
struct ZipPacketEntry {
    index: usize,
    name: String,
}

fn extract_zip_packets(input_path: &Path, output_dir: &Path) -> Result<Vec<PathBuf>, BundleError> {
    let packet_entries = collect_zip_packet_entries(input_path)?;

    fs::create_dir_all(output_dir)
        .map_err(|error| bundle_io_error(output_dir, "create extraction directory", error))?;

    let file = File::open(input_path)
        .map_err(|error| bundle_io_error(input_path, "open ZIP bundle", error))?;
    let mut archive = ZipArchive::new(file).map_err(|error| zip_error(input_path, error))?;

    let mut extracted_paths = Vec::with_capacity(packet_entries.len());
    for packet_entry in packet_entries {
        let output_path = output_dir.join(&packet_entry.name);
        let extract_result =
            extract_zip_packet_entry(input_path, &mut archive, &packet_entry, &output_path);

        if let Err(error) = extract_result {
            for extracted_path in &extracted_paths {
                let _ = fs::remove_file(extracted_path);
            }
            return Err(error);
        }

        extracted_paths.push(output_path);
    }

    Ok(extracted_paths)
}

fn collect_zip_packet_entries(input_path: &Path) -> Result<Vec<ZipPacketEntry>, BundleError> {
    let file = File::open(input_path)
        .map_err(|error| bundle_io_error(input_path, "open ZIP bundle", error))?;
    let mut archive = ZipArchive::new(file).map_err(|error| zip_error(input_path, error))?;
    let mut packet_entries = Vec::new();
    let mut output_names = HashSet::new();

    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| zip_error(input_path, error))?;
        let name = entry.name().to_string();
        validate_zip_packet_entry(input_path, &name)?;

        let output_key = name.to_ascii_lowercase();
        if !output_names.insert(output_key) {
            return Err(BundleError::DuplicateZipPacketEntry {
                path: input_path.to_path_buf(),
                entry: name,
            });
        }

        packet_entries.push(ZipPacketEntry { index, name });
    }

    if packet_entries.is_empty() {
        return Err(BundleError::NoPacketEntries {
            path: input_path.to_path_buf(),
        });
    }

    packet_entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(packet_entries)
}

fn validate_zip_packet_entry(input_path: &Path, entry_name: &str) -> Result<(), BundleError> {
    if entry_name.is_empty() {
        return Err(unsupported_zip_entry(
            input_path,
            entry_name,
            "entry name is empty",
        ));
    }

    if entry_name.contains('\0') {
        return Err(unsupported_zip_entry(
            input_path,
            entry_name,
            "entry name contains a null byte",
        ));
    }

    if entry_name.contains('/') || entry_name.contains('\\') {
        return Err(unsupported_zip_entry(
            input_path,
            entry_name,
            "only top-level packet entries are supported",
        ));
    }

    let extension = Path::new(entry_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "pkt" {
        return Err(unsupported_zip_entry(
            input_path,
            entry_name,
            "only .pkt entries are supported",
        ));
    }

    Ok(())
}

fn extract_zip_packet_entry(
    input_path: &Path,
    archive: &mut ZipArchive<File>,
    packet_entry: &ZipPacketEntry,
    output_path: &Path,
) -> Result<(), BundleError> {
    let mut entry = archive
        .by_index(packet_entry.index)
        .map_err(|error| zip_error(input_path, error))?;
    let mut output = File::options()
        .write(true)
        .create_new(true)
        .open(output_path)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                BundleError::ExtractedPacketExists {
                    path: input_path.to_path_buf(),
                    output_path: output_path.to_path_buf(),
                }
            } else {
                bundle_io_error(output_path, "create extracted packet", error)
            }
        })?;

    io::copy(&mut entry, &mut output).map_err(|error| {
        let _ = fs::remove_file(output_path);
        bundle_io_error(output_path, "write extracted packet", error)
    })?;

    Ok(())
}

fn unsupported_zip_entry(input_path: &Path, entry: &str, reason: &'static str) -> BundleError {
    BundleError::UnsupportedZipEntry {
        path: input_path.to_path_buf(),
        entry: entry.to_string(),
        reason,
    }
}

fn bundle_io_error(path: &Path, operation: &'static str, error: io::Error) -> BundleError {
    BundleError::Io {
        path: path.to_path_buf(),
        operation,
        message: error.to_string(),
    }
}

fn zip_error(path: &Path, error: zip::result::ZipError) -> BundleError {
    BundleError::ZipArchive {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };
    use zip::{
        CompressionMethod, ZipWriter,
        write::{FileOptions, SimpleFileOptions},
    };

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
    fn extraction_boundary_extracts_zip_packets() {
        let temp_dir = unique_temp_dir("zip-single");
        let bundle_path = temp_dir.join("00112233.zip");
        let output_dir = temp_dir.join("out");
        write_zip_file(&bundle_path, &[("00000001.pkt", b"packet bytes")]);

        let packets = BundleExtractor::extract_packets(&bundle_path, &output_dir)
            .expect("extract zip packet");

        assert_eq!(packets, vec![output_dir.join("00000001.pkt")]);
        assert_eq!(
            fs::read(output_dir.join("00000001.pkt")).expect("read extracted packet"),
            b"packet bytes"
        );
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }

    #[test]
    fn extraction_boundary_extracts_zip_packets_deterministically() {
        let temp_dir = unique_temp_dir("zip-multi");
        let bundle_path = temp_dir.join("00112233.zip");
        let output_dir = temp_dir.join("out");
        write_zip_file(
            &bundle_path,
            &[
                ("00000002.PKT", b"second"),
                ("00000001.pkt", b"first"),
                ("00000003.pKt", b"third"),
            ],
        );

        let packets = BundleExtractor::extract_packets(&bundle_path, &output_dir)
            .expect("extract zip packets");

        assert_eq!(
            packets,
            vec![
                output_dir.join("00000001.pkt"),
                output_dir.join("00000002.PKT"),
                output_dir.join("00000003.pKt"),
            ]
        );
        assert_eq!(fs::read(&packets[0]).expect("read first"), b"first");
        assert_eq!(fs::read(&packets[1]).expect("read second"), b"second");
        assert_eq!(fs::read(&packets[2]).expect("read third"), b"third");
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }

    #[test]
    fn extraction_boundary_rejects_empty_zip() {
        let temp_dir = unique_temp_dir("zip-empty");
        let bundle_path = temp_dir.join("00112233.zip");
        let output_dir = temp_dir.join("out");
        write_zip_file(&bundle_path, &[]);

        let error =
            BundleExtractor::extract_packets(&bundle_path, &output_dir).expect_err("empty zip");

        assert_eq!(
            error,
            BundleError::NoPacketEntries {
                path: bundle_path.clone(),
            }
        );
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }

    #[test]
    fn extraction_boundary_rejects_corrupt_zip() {
        let temp_dir = unique_temp_dir("zip-corrupt");
        let bundle_path = temp_dir.join("00112233.zip");
        let output_dir = temp_dir.join("out");
        fs::write(&bundle_path, b"not a zip").expect("write corrupt zip");

        let error =
            BundleExtractor::extract_packets(&bundle_path, &output_dir).expect_err("corrupt zip");

        assert!(matches!(
            error,
            BundleError::ZipArchive { path, .. } if path == bundle_path
        ));
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }

    #[test]
    fn extraction_boundary_rejects_nested_zip_entry() {
        let temp_dir = unique_temp_dir("zip-nested");
        let bundle_path = temp_dir.join("00112233.zip");
        let output_dir = temp_dir.join("out");
        write_zip_file(&bundle_path, &[("nested/00000001.pkt", b"packet")]);

        let error =
            BundleExtractor::extract_packets(&bundle_path, &output_dir).expect_err("nested entry");

        assert_eq!(
            error,
            BundleError::UnsupportedZipEntry {
                path: bundle_path.clone(),
                entry: "nested/00000001.pkt".to_string(),
                reason: "only top-level packet entries are supported",
            }
        );
        assert!(!output_dir.exists());
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }

    #[test]
    fn extraction_boundary_rejects_non_packet_zip_entry() {
        let temp_dir = unique_temp_dir("zip-non-packet");
        let bundle_path = temp_dir.join("00112233.zip");
        let output_dir = temp_dir.join("out");
        write_zip_file(&bundle_path, &[("README.TXT", b"not a packet")]);

        let error = BundleExtractor::extract_packets(&bundle_path, &output_dir)
            .expect_err("non-packet entry");

        assert_eq!(
            error,
            BundleError::UnsupportedZipEntry {
                path: bundle_path.clone(),
                entry: "README.TXT".to_string(),
                reason: "only .pkt entries are supported",
            }
        );
        assert!(!output_dir.exists());
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }

    #[test]
    fn extraction_boundary_rejects_duplicate_zip_packet_output_names() {
        let temp_dir = unique_temp_dir("zip-duplicate");
        let bundle_path = temp_dir.join("00112233.zip");
        let output_dir = temp_dir.join("out");
        write_zip_file(
            &bundle_path,
            &[("packet.PKT", b"first"), ("PACKET.pkt", b"second")],
        );

        let error =
            BundleExtractor::extract_packets(&bundle_path, &output_dir).expect_err("duplicate");

        assert_eq!(
            error,
            BundleError::DuplicateZipPacketEntry {
                path: bundle_path.clone(),
                entry: "PACKET.pkt".to_string(),
            }
        );
        assert!(!output_dir.exists());
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }

    #[test]
    fn extraction_boundary_refuses_to_overwrite_existing_packet() {
        let temp_dir = unique_temp_dir("zip-existing");
        let bundle_path = temp_dir.join("00112233.zip");
        let output_dir = temp_dir.join("out");
        write_zip_file(&bundle_path, &[("00000001.pkt", b"new")]);
        fs::create_dir_all(&output_dir).expect("create output dir");
        fs::write(output_dir.join("00000001.pkt"), b"existing").expect("write existing packet");

        let error = BundleExtractor::extract_packets(&bundle_path, &output_dir)
            .expect_err("existing output");

        assert_eq!(
            error,
            BundleError::ExtractedPacketExists {
                path: bundle_path.clone(),
                output_path: output_dir.join("00000001.pkt"),
            }
        );
        assert_eq!(
            fs::read(output_dir.join("00000001.pkt")).expect("read existing packet"),
            b"existing"
        );
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
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

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "oxidebbs-ftn-bundle-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        temp_dir
    }

    fn write_zip_file(bundle_path: &Path, entries: &[(&str, &[u8])]) {
        let file = File::create(bundle_path).expect("create zip file");
        let mut writer = ZipWriter::new(file);
        let options: FileOptions<'_, ()> =
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        for (name, bytes) in entries {
            writer.start_file(*name, options).expect("start zip entry");
            writer.write_all(bytes).expect("write zip entry");
        }

        writer.finish().expect("finish zip file");
    }
}
