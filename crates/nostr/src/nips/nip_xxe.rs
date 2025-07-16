// Copyright (c) 2022-2023 Yuki Kishimoto
// Copyright (c) 2023-2025 Rust Nostr Developers
// Distributed under the MIT software license

//! NIP-XXE: Workflows
//!
//! <https://github.com/nostr-protocol/nips/blob/master/XXE.md>

use alloc::string::{String, ToString};
use core::convert::TryFrom;
use core::fmt;
use core::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::prelude::Coordinate;
use crate::{Event, Kind, RelayUrl, Tag, TagKind, TagStandard, Timestamp};

/// A Tracker for productive workflows as defined in NIP-XXE.
///
/// Trackers are the glue that specifies:
/// 1. What Item is being tracked
/// 2. In which Workflow it is being tracked
/// 3. Any other metadata attached to the Item that is relevant to the Workflow
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tracker<WorkflowSpecificData>
where
    WorkflowSpecificData: TryFrom<Event>
{
    /// Unique identifier for the tracker
    pub id: String,

    /// Reference to the tracked item
    pub tracked_item: Coordinate,

    /// Reference to the workflow
    pub workflow: Coordinate,

    /// Additional workflow-specific tags
    pub workflow_specific_data: WorkflowSpecificData,
}

pub struct LabelledCoordinate {
    coordinate: Coordinate,
    label: CoordinateLabel
}

/// A label 
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinateLabel {
    None,
    TrackedItem,
    Workflow,
    Custom(String)
}

pub enum CoordinateLabelError {
    WrongLabel
}

impl FromStr for CoordinateLabel {
    type Err = CoordinateLabelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tracked_item" => Ok(CoordinateLabel::TrackedItem),
            "workflow" => Ok(CoordinateLabel::Workflow),
            _ => Ok(CoordinateLabel::Custom(s.to_string())),
        }
    }
}

impl ToString for CoordinateLabel {
    fn to_string(&self) -> String {
        match self {
            CoordinateLabel::TrackedItem => "tracked_item".to_string(),
            CoordinateLabel::Workflow => "workflow".to_string(),
            CoordinateLabel::None => "".to_string(),
            CoordinateLabel::Custom(string) => string.clone(),
        }
    }
}

/// Errors that can occur when working with trackers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackerError {
    /// Event has wrong kind
    WrongKind(Kind),
    /// Missing required tag
    MissingTag(&'static str),
    /// Missing identifier (d tag)
    MissingIdentifier,
    /// An invalid `a` tag
    InvalidATag,
    /// Missing tracked item
    MissingTrackedItem,
    /// Missing workflow item
    MissingWorkflow,
    /// Invalid URL
    InvalidUrl(String),
    /// Invalid timestamp
    InvalidTimestamp,
    /// Cannot get workflow specific data
    CannotGetWorkflowSpecificData,
    /// Duplicate tag found
    DuplicateTag(&'static str),
    /// Invalid tag format
    InvalidTagFormat(&'static str),
}

impl fmt::Display for TrackerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrackerError::WrongKind(kind) => write!(f, "wrong event kind. Expected: {}; Got: {}", Kind::Tracker, kind),
            TrackerError::MissingTag(tag) => write!(f, "missing required tag: {}", tag),
            TrackerError::MissingIdentifier => write!(f, "missing identifier"),
            TrackerError::MissingTrackedItem => write!(f, "missing tracked item reference"),
            TrackerError::MissingWorkflow => write!(f, "missing workflow reference"),
            TrackerError::InvalidUrl(url) => write!(f, "invalid URL: {}", url),
            TrackerError::InvalidTimestamp => write!(f, "invalid timestamp"),
            TrackerError::DuplicateTag(tag) => write!(f, "duplicate tag found: {}", tag),
            TrackerError::InvalidTagFormat(tag) => write!(f, "invalid tag format: {}", tag),
            TrackerError::InvalidATag => write!(f, "invalid a-tag"),
            TrackerError::CannotGetWorkflowSpecificData => write!(f, "cannot get workflow specific data"),
        }
    }
}

fn parse_a_tag(tag: Tag) -> Result<LabelledCoordinate, TrackerError>
{
    let tag = tag.to_vec();
    if tag.len() >= 2 {
        Ok(LabelledCoordinate { 
            coordinate: Coordinate::from_str(tag[1].as_ref()).map_err(|_| TrackerError::InvalidATag)?,
            label: match tag.get(2).map(|u| u.as_ref()) {
                Some(label) => CoordinateLabel::from_str(label).map_err(|_| TrackerError::InvalidATag)?,
                _ => CoordinateLabel::None,
            }
        })
    } else {
        Err(TrackerError::InvalidATag)
    }
}

impl<WorkflowSpecificData: TryFrom<Event>> TryFrom<&Event> for Tracker<WorkflowSpecificData> {
    type Error = TrackerError;

    fn try_from(value: &Event) -> Result<Self, Self::Error> {
        let event = value;
        let Kind::Tracker = event.kind else {
            return Err(TrackerError::WrongKind(event.kind))
        };
        let Some(identity_tag) = event.tags.find_standardized(TagKind::d()) else {
            return Err(TrackerError::MissingIdentifier)
        };
        let TagStandard::Identifier(mutable_identifier) = identity_tag else {
            return Err(TrackerError::MissingIdentifier)
        };
        let tracked_item = {
            let mut found_item = None;
            for tag in event.tags.clone() {
                if let Ok(labelled_coordinate) = parse_a_tag(tag.clone()) {
                    if labelled_coordinate.label == CoordinateLabel::TrackedItem {
                        found_item = Some(labelled_coordinate.coordinate);
                        break;
                    }
                }
            }
            found_item.ok_or(TrackerError::MissingTrackedItem)?
        };

        let workflow = {
            let mut found_workflow = None;
            for tag in event.tags.clone() {
                if let Ok(labelled_coordinate) = parse_a_tag(tag.clone()) {
                    if labelled_coordinate.label == CoordinateLabel::Workflow {
                        found_workflow = Some(labelled_coordinate.coordinate);
                        break;
                    }
                }
            }
            found_workflow.ok_or(TrackerError::MissingWorkflow)?
        };
        let workflow_specific_data = WorkflowSpecificData::try_from(value.clone()).map_err(|_| TrackerError::CannotGetWorkflowSpecificData)?;
        return Ok(Tracker {
            id: mutable_identifier.clone(),
            tracked_item: tracked_item,
            workflow: workflow,
            workflow_specific_data,
        })
    }
}
