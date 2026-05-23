// SPDX-License-Identifier: Apache-2.0

use eunomio_core::{PartitionRow, RunKind};

pub(super) struct PhaseScope {
    pub org_id: String,
    pub partition_id: String,
    pub session_id: String,
    pub target_node_id: String,
}

impl PhaseScope {
    pub fn from_partition(org_id: impl Into<String>, row: &PartitionRow) -> Self {
        Self {
            org_id: org_id.into(),
            partition_id: row.id.clone(),
            session_id: row.session_id.clone(),
            target_node_id: row.target_node_id.clone(),
        }
    }
}

pub(super) struct ActiveRun {
    pub scope: PhaseScope,
    pub run_id: String,
    pub kind: RunKind,
}

impl ActiveRun {
    pub fn new(
        org_id: String,
        partition_row: &PartitionRow,
        run_id: String,
        kind: RunKind,
    ) -> Self {
        Self {
            scope: PhaseScope::from_partition(org_id, partition_row),
            run_id,
            kind,
        }
    }
}
