// SPDX-License-Identifier: Apache-2.0

use crate::types::ModelSelection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    #[serde(default)]
    pub transcripts_enabled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeySettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionSettings {
    #[serde(default)]
    pub general: GeneralSettings,
    #[serde(default)]
    pub hotkeys: HotkeySettings,
    #[serde(default)]
    pub coordinator: CoordinatorSettings,
    #[serde(default)]
    pub surveyor: SubagentSettings,
    #[serde(default)]
    pub planner: SubagentSettings,
    #[serde(default)]
    pub constructor: SubagentSettings,
    #[serde(default)]
    pub shaver: SubagentSettings,
    #[serde(default)]
    pub reorder: SubagentSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordinatorSettings {
    #[serde(default)]
    pub model: ModelSelection,
    #[serde(default)]
    pub human_in_the_loop: HumanInTheLoop,
    #[serde(default = "default_iteration_limit")]
    pub max_iterations: IterationLimit,
    #[serde(default)]
    pub surveyor_enabled: bool,
    #[serde(default = "default_true")]
    pub timeline_enabled: bool,
    #[serde(default = "default_true")]
    pub reorder_enabled: bool,
}

impl Default for CoordinatorSettings {
    fn default() -> Self {
        Self {
            model: ModelSelection::default(),
            human_in_the_loop: HumanInTheLoop::default(),
            max_iterations: default_iteration_limit(),
            surveyor_enabled: false,
            timeline_enabled: true,
            reorder_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HumanInTheLoop {
    #[serde(default)]
    pub after_survey: bool,
    #[serde(default)]
    pub after_planning: bool,
    #[serde(default)]
    pub after_construct: bool,
    #[serde(default)]
    pub after_indivisible: bool,
}

impl Default for HumanInTheLoop {
    fn default() -> Self {
        Self {
            after_survey: false,
            after_planning: false,
            after_construct: false,
            after_indivisible: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum IterationLimit {
    Count {
        #[serde(default = "default_count")]
        count: u32,
    },
    Auto,
}

fn default_count() -> u32 {
    1
}

fn default_iteration_limit() -> IterationLimit {
    IterationLimit::Auto
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentSettings {
    #[serde(default)]
    pub override_model: bool,
    #[serde(default)]
    pub model: ModelSelection,
}

impl Default for SubagentSettings {
    fn default() -> Self {
        Self {
            override_model: false,
            model: ModelSelection::default(),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PartitionSettingsPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub general: Option<GeneralSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotkeys: Option<HotkeySettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinator: Option<CoordinatorSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surveyor: Option<SubagentSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner: Option<SubagentSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constructor: Option<SubagentSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaver: Option<SubagentSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reorder: Option<SubagentSettings>,
}

impl PartitionSettings {
    pub fn apply_patch(&mut self, patch: PartitionSettingsPatch) {
        if let Some(v) = patch.general {
            self.general = v;
        }
        if let Some(v) = patch.hotkeys {
            self.hotkeys = v;
        }
        if let Some(v) = patch.coordinator {
            self.coordinator = v;
        }
        if let Some(v) = patch.surveyor {
            self.surveyor = v;
        }
        if let Some(v) = patch.planner {
            self.planner = v;
        }
        if let Some(v) = patch.constructor {
            self.constructor = v;
        }
        if let Some(v) = patch.shaver {
            self.shaver = v;
        }
        if let Some(v) = patch.reorder {
            self.reorder = v;
        }
    }
}
