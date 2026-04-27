use std::collections::HashMap;

use tanren_domain::methodology::finding::{
    Finding, FindingLifecycleEvidence, FindingStatus, FindingView,
};

pub(super) fn open_finding_view(finding: Finding) -> FindingView {
    FindingView {
        finding,
        status: FindingStatus::Open,
        lifecycle_evidence: None,
        superseded_by: Vec::new(),
        updated_at: None,
    }
}

pub(super) fn apply_finding_view_status(
    views: &mut HashMap<tanren_domain::FindingId, FindingView>,
    finding_id: tanren_domain::FindingId,
    status: FindingStatus,
    evidence: FindingLifecycleEvidence,
    superseded_by: Vec<tanren_domain::FindingId>,
) {
    if let Some(view) = views.get_mut(&finding_id) {
        view.status = status;
        view.lifecycle_evidence = Some(evidence);
        view.superseded_by = superseded_by;
    }
}
