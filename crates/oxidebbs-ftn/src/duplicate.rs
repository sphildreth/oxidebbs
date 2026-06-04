use oxidebbs_network::DuplicateDetectionKey;

pub trait DuplicateDetector {
    fn is_duplicate(&self, key: &DuplicateDetectionKey) -> bool;
}

pub struct NullDuplicateDetector;

impl DuplicateDetector for NullDuplicateDetector {
    fn is_duplicate(&self, _key: &DuplicateDetectionKey) -> bool {
        false
    }
}
