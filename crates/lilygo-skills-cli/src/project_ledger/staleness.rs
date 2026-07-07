//! Staleness rules keep project memory from becoming source authority.
use super::time::current_timestamp;
use super::{CapabilityEntry, ContextDigest};

pub(super) fn entry_stale(
    entry: &CapabilityEntry,
    current_code_signature: Option<&str>,
    current_source_signature: Option<&str>,
) -> bool {
    if entry.status == "stale" {
        return true;
    }
    if entry.runtime_version != env!("CARGO_PKG_VERSION") {
        return true;
    }
    if entry.status == "verified" && entry.public_evidence_hash.is_none() {
        return true;
    }
    if let Some(current) = current_source_signature
        && entry.source_signature != current
    {
        return true;
    }
    if let Some(expires_at) = &entry.expires_at
        && expires_at.as_str() < current_timestamp().as_str()
    {
        return true;
    }
    if let (Some(recorded), Some(current)) = (&entry.project_code_signature, current_code_signature)
        && recorded != current
    {
        return true;
    }
    false
}

pub(super) fn digest_stale(digest: &ContextDigest) -> bool {
    digest.runtime_version != env!("CARGO_PKG_VERSION")
        || digest.expires_at.as_str() < current_timestamp().as_str()
}
