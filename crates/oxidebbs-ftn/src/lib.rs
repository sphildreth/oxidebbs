pub mod bundle;
pub mod duplicate;
pub mod error;
pub mod kludge;
pub mod nodelist;
pub mod packet;

pub use bundle::{
    BundleClassification, BundleError, BundleExtractor, BundleFormat, classify_bundle_path,
};
pub use duplicate::{
    DecentDbDuplicateDetector, DuplicateDetector, NullDuplicateDetector, duplicate_key,
    echomail_duplicate_key, echomail_fallback_duplicate_candidates, netmail_duplicate_key,
    netmail_fallback_duplicate_candidates,
};
pub use error::FtnError;
pub use kludge::{
    EchomailKludge, FtnAddressList, FtnMessageComposer, FtnParsedMessage, compose_kludge,
    parse_kludge, parse_message_body,
};
pub use nodelist::{FtnNodelistEntry, NodelistDiffError, apply_nodelist_diff, parse_nodelist};
pub use packet::{
    FtnPacket, MessageAttribute, PacketHeader, PacketMessage, PacketReader, PacketWriter,
};
