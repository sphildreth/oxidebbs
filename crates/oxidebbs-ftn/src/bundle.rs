use std::{
    collections::HashSet,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;
use zip::{
    CompressionMethod, ZipArchive, ZipWriter,
    write::{FileOptions, SimpleFileOptions},
};

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

    #[error("ARJ archive could not be read: {path}: {message}")]
    ArjArchive { path: PathBuf, message: String },

    #[error("unsupported ARJ entry {entry:?} in {path}: {reason}")]
    UnsupportedArjEntry {
        path: PathBuf,
        entry: String,
        reason: &'static str,
    },

    #[error("duplicate ARJ packet entry {entry:?} in {path}")]
    DuplicateArjPacketEntry { path: PathBuf, entry: String },

    #[error("ARJ archive contains no packet entries: {path}")]
    NoArjPacketEntries { path: PathBuf },

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

    #[error("bundle creation received no packet inputs for {output_path}")]
    NoPacketInputs { output_path: PathBuf },

    #[error("packet input path has no filename: {path}")]
    PacketMissingFilename { path: PathBuf },

    #[error("unsupported packet input {path}: {reason}")]
    UnsupportedPacketInput { path: PathBuf, reason: &'static str },

    #[error("duplicate packet input filename {entry:?} for {output_path}")]
    DuplicatePacketInputName { output_path: PathBuf, entry: String },

    #[error("bundle output already exists: {output_path}")]
    BundleOutputExists { output_path: PathBuf },
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
            BundleFormat::ArjArcmail => {
                extract_arj_packets(&classification.path, output_dir.as_ref())
            }
        }
    }
}

/// Creation boundary for outbound arcmail bundles.
pub struct BundleCreator;

impl BundleCreator {
    /// Create a ZIP arcmail bundle containing top-level `.pkt` entries.
    ///
    /// Packet paths may live in any directory, but each ZIP entry is written
    /// using only the packet file name. The output file is created with
    /// `create_new` semantics so an existing bundle is never overwritten.
    ///
    /// # Errors
    ///
    /// Returns typed errors for empty input, non-packet filenames, duplicate
    /// output names, existing bundle output, and I/O or ZIP writer failures.
    pub fn create_zip_bundle(
        packet_paths: &[PathBuf],
        output_path: impl AsRef<Path>,
    ) -> Result<PathBuf, BundleError> {
        let output_path = output_path.as_ref();
        if packet_paths.is_empty() {
            return Err(BundleError::NoPacketInputs {
                output_path: output_path.to_path_buf(),
            });
        }

        let packet_entries = collect_packet_inputs_for_zip(packet_paths, output_path)?;
        if let Some(parent) = output_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|error| bundle_io_error(parent, "create bundle directory", error))?;
        }

        let file = File::options()
            .write(true)
            .create_new(true)
            .open(output_path)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    BundleError::BundleOutputExists {
                        output_path: output_path.to_path_buf(),
                    }
                } else {
                    bundle_io_error(output_path, "create ZIP bundle", error)
                }
            })?;

        let mut writer = ZipWriter::new(file);
        let options: FileOptions<'_, ()> =
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        for packet_entry in packet_entries {
            writer
                .start_file(&packet_entry.name, options)
                .map_err(|error| zip_error(output_path, error))?;
            let mut packet = File::open(&packet_entry.path)
                .map_err(|error| bundle_io_error(&packet_entry.path, "open packet input", error))?;
            io::copy(&mut packet, &mut writer).map_err(|error| {
                bundle_io_error(output_path, "write packet into ZIP bundle", error)
            })?;
        }

        writer
            .finish()
            .map_err(|error| zip_error(output_path, error))?;
        Ok(output_path.to_path_buf())
    }
}

#[derive(Debug, Clone)]
struct ZipPacketEntry {
    index: usize,
    name: String,
}

#[derive(Debug, Clone)]
struct PacketInputEntry {
    path: PathBuf,
    name: String,
}

fn collect_packet_inputs_for_zip(
    packet_paths: &[PathBuf],
    output_path: &Path,
) -> Result<Vec<PacketInputEntry>, BundleError> {
    let mut packet_entries = Vec::with_capacity(packet_paths.len());
    let mut output_names = HashSet::new();

    for packet_path in packet_paths {
        let name = validate_packet_input_path(packet_path)?;
        let output_key = name.to_ascii_lowercase();
        if !output_names.insert(output_key) {
            return Err(BundleError::DuplicatePacketInputName {
                output_path: output_path.to_path_buf(),
                entry: name,
            });
        }
        packet_entries.push(PacketInputEntry {
            path: packet_path.clone(),
            name,
        });
    }

    packet_entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(packet_entries)
}

fn validate_packet_input_path(packet_path: &Path) -> Result<String, BundleError> {
    let path = packet_path.to_path_buf();
    let file_name = packet_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| BundleError::PacketMissingFilename { path: path.clone() })?;
    if file_name.is_empty() {
        return Err(BundleError::PacketMissingFilename { path });
    }
    if file_name.contains('\0') {
        return Err(BundleError::UnsupportedPacketInput {
            path,
            reason: "filename contains a null byte",
        });
    }

    let extension = Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "pkt" {
        return Err(BundleError::UnsupportedPacketInput {
            path,
            reason: "only .pkt packet inputs are supported",
        });
    }

    Ok(file_name.to_string())
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

fn extract_arj_packets(input_path: &Path, output_dir: &Path) -> Result<Vec<PathBuf>, BundleError> {
    let arj_entries = collect_arj_packet_entries(input_path)?;

    fs::create_dir_all(output_dir)
        .map_err(|error| bundle_io_error(output_dir, "create extraction directory", error))?;

    let mut archive = unarc_rs::unified::ArchiveFormat::open_path(input_path)
        .map_err(|error| arj_error(input_path, error))?;

    let mut extracted_paths = Vec::with_capacity(arj_entries.len());
    for entry in &arj_entries {
        let output_path = output_dir.join(&entry.name);
        let extract_result =
            extract_arj_packet_entry(input_path, &mut archive, entry, &output_path);

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

fn collect_arj_packet_entries(input_path: &Path) -> Result<Vec<ArjPacketEntry>, BundleError> {
    let mut archive = unarc_rs::unified::ArchiveFormat::open_path(input_path)
        .map_err(|error| arj_error(input_path, error))?;

    let mut packet_entries = Vec::new();
    let mut output_names = HashSet::new();

    let entries = archive
        .entries()
        .map_err(|error| arj_error(input_path, error))?;
    for entry in &entries {
        let name = entry.name().to_string();
        validate_arj_packet_entry(input_path, &name)?;

        let output_key = name.to_ascii_lowercase();
        if !output_names.insert(output_key) {
            return Err(BundleError::DuplicateArjPacketEntry {
                path: input_path.to_path_buf(),
                entry: name,
            });
        }

        packet_entries.push(ArjPacketEntry { name });
    }

    if packet_entries.is_empty() {
        return Err(BundleError::NoArjPacketEntries {
            path: input_path.to_path_buf(),
        });
    }

    packet_entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(packet_entries)
}

fn validate_arj_packet_entry(input_path: &Path, entry_name: &str) -> Result<(), BundleError> {
    if entry_name.is_empty() {
        return Err(unsupported_arj_entry(
            input_path,
            entry_name,
            "entry name is empty",
        ));
    }

    if entry_name.contains('\0') {
        return Err(unsupported_arj_entry(
            input_path,
            entry_name,
            "entry name contains a null byte",
        ));
    }

    if entry_name.contains('/') || entry_name.contains('\\') {
        return Err(unsupported_arj_entry(
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
        return Err(unsupported_arj_entry(
            input_path,
            entry_name,
            "only .pkt entries are supported",
        ));
    }

    Ok(())
}

fn extract_arj_packet_entry(
    input_path: &Path,
    archive: &mut unarc_rs::unified::UnifiedArchive<std::io::BufReader<File>>,
    entry: &ArjPacketEntry,
    output_path: &Path,
) -> Result<(), BundleError> {
    let entries = archive
        .entries()
        .map_err(|error| arj_error(input_path, error))?;
    let archive_entry = entries
        .iter()
        .find(|e| e.name() == entry.name)
        .ok_or_else(|| BundleError::UnsupportedArjEntry {
            path: input_path.to_path_buf(),
            entry: entry.name.clone(),
            reason: "entry not found in archive",
        })?;

    let data = archive
        .read(archive_entry)
        .map_err(|error| arj_error(input_path, error))?;

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

    use std::io::Write;
    output.write_all(&data).map_err(|error| {
        let _ = fs::remove_file(output_path);
        bundle_io_error(output_path, "write extracted packet", error)
    })?;

    Ok(())
}

fn unsupported_arj_entry(input_path: &Path, entry: &str, reason: &'static str) -> BundleError {
    BundleError::UnsupportedArjEntry {
        path: input_path.to_path_buf(),
        entry: entry.to_string(),
        reason,
    }
}

fn arj_error(path: &Path, error: impl std::fmt::Display) -> BundleError {
    BundleError::ArjArchive {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

struct ArjPacketEntry {
    name: String,
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
    fn extraction_boundary_attempts_arj_and_reports_missing_file() {
        let error = BundleExtractor::extract_packets("inbound/00112233.arj", "temp")
            .expect_err("arj file does not exist");

        match error {
            BundleError::ArjArchive { path, .. } => {
                assert_eq!(path, PathBuf::from("inbound/00112233.arj"));
            }
            other => panic!("expected ArjArchive error, got: {other:?}"),
        }
    }

    #[test]
    fn bundle_creator_writes_extractable_zip_bundle_deterministically() {
        let temp_dir = unique_temp_dir("zip-create");
        let packet_dir = temp_dir.join("packets");
        let output_dir = temp_dir.join("out");
        fs::create_dir_all(&packet_dir).expect("create packet dir");
        let second_packet = packet_dir.join("00000002.pkt");
        let first_packet = packet_dir.join("00000001.pkt");
        fs::write(&second_packet, b"second").expect("write second packet");
        fs::write(&first_packet, b"first").expect("write first packet");
        let bundle_path = temp_dir.join("00112233.zip");

        let created = BundleCreator::create_zip_bundle(
            &[second_packet.clone(), first_packet.clone()],
            &bundle_path,
        )
        .expect("create zip bundle");

        assert_eq!(created, bundle_path);
        let packets = BundleExtractor::extract_packets(&bundle_path, &output_dir)
            .expect("extract created bundle");
        assert_eq!(
            packets,
            vec![
                output_dir.join("00000001.pkt"),
                output_dir.join("00000002.pkt"),
            ]
        );
        assert_eq!(fs::read(&packets[0]).expect("read first"), b"first");
        assert_eq!(fs::read(&packets[1]).expect("read second"), b"second");
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }

    #[test]
    fn bundle_creator_rejects_empty_packet_list() {
        let temp_dir = unique_temp_dir("zip-create-empty");
        let bundle_path = temp_dir.join("00112233.zip");

        let error = BundleCreator::create_zip_bundle(&[], &bundle_path)
            .expect_err("empty packet list rejected");

        assert_eq!(
            error,
            BundleError::NoPacketInputs {
                output_path: bundle_path,
            }
        );
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }

    #[test]
    fn bundle_creator_rejects_duplicate_packet_filenames() {
        let temp_dir = unique_temp_dir("zip-create-duplicate");
        let left_dir = temp_dir.join("left");
        let right_dir = temp_dir.join("right");
        fs::create_dir_all(&left_dir).expect("create left dir");
        fs::create_dir_all(&right_dir).expect("create right dir");
        let left_packet = left_dir.join("same.pkt");
        let right_packet = right_dir.join("SAME.PKT");
        fs::write(&left_packet, b"left").expect("write left packet");
        fs::write(&right_packet, b"right").expect("write right packet");
        let bundle_path = temp_dir.join("00112233.zip");

        let error = BundleCreator::create_zip_bundle(&[left_packet, right_packet], &bundle_path)
            .expect_err("duplicate names rejected");

        assert_eq!(
            error,
            BundleError::DuplicatePacketInputName {
                output_path: bundle_path,
                entry: "SAME.PKT".to_string(),
            }
        );
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }

    #[test]
    fn bundle_creator_rejects_non_packet_inputs() {
        let temp_dir = unique_temp_dir("zip-create-non-packet");
        let input = temp_dir.join("README.TXT");
        fs::write(&input, b"not a packet").expect("write input");
        let bundle_path = temp_dir.join("00112233.zip");

        let error = BundleCreator::create_zip_bundle(std::slice::from_ref(&input), &bundle_path)
            .expect_err("non-packet input rejected");

        assert_eq!(
            error,
            BundleError::UnsupportedPacketInput {
                path: input,
                reason: "only .pkt packet inputs are supported",
            }
        );
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
    }

    #[test]
    fn bundle_creator_refuses_to_overwrite_existing_bundle() {
        let temp_dir = unique_temp_dir("zip-create-existing");
        let input = temp_dir.join("00000001.pkt");
        let bundle_path = temp_dir.join("00112233.zip");
        fs::write(&input, b"packet").expect("write packet");
        fs::write(&bundle_path, b"existing").expect("write existing bundle");

        let error = BundleCreator::create_zip_bundle(std::slice::from_ref(&input), &bundle_path)
            .expect_err("existing bundle rejected");

        assert_eq!(
            error,
            BundleError::BundleOutputExists {
                output_path: bundle_path.clone(),
            }
        );
        assert_eq!(fs::read(&bundle_path).expect("read existing"), b"existing");
        fs::remove_dir_all(temp_dir).expect("remove temp dir");
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
