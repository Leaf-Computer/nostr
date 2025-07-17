// Copyright (c) 2022-2023 Yuki Kishimoto
// Copyright (c) 2023-2025 Rust Nostr Developers
// Distributed under the MIT software license

//! NIP-XXC: Kanban Workflows
//!
//! <https://github.com/nostr-protocol/nips/blob/master/XXC.md>

use crate::{nips::nipxxa::TaskMetadata, Event, Kind, PublicKey, Tag, TagKind, TaskError, Tracker};

pub type KanbanTracker = Tracker<KanbanSpecificTrackerData>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KanbanBoard {
    /// Unique identifier for the tracker
    pub id: String,
    
    /// Board title
    pub title: Option<String>,
    
    /// Board description
    pub description: Option<String>,
    
    /// NIP-31 plaintext summary
    pub alt: Option<String>,
    
    /// The columns for this board
    pub columns: Vec<KanbanColumnDefinition>,
    
    /// A 'maintainers' list who can add/edit cards in this board
    pub pubkey: Vec<PublicKey>,
}

/// A definition for one column of a kanban board
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KanbanColumnDefinition {
    /// Machine-readable string identifier of the column (e.g. `todo`, `in-progress`, etc)
    pub id: String,
    
    /// Human-readable label for the column (e.g. "To do", "In Progress")
    pub label: String,
    
    /// Optional color to associate with the column
    pub color: Option<Color>
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KanbanSpecificTrackerData {
    pub status: KanbanTrackerStatus,
    pub rank: Option<u32>,
    pub task_metadata: TaskMetadata
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KanbanTrackerStatus {
    /// Status is a specific column ID on the linked Kanban board
    Column(String),
    /// Status information is deferred to the object being tracked
    Defer,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Color {
    /// Preset red color
    Red,
    /// Preset orange color
    Orange,
    /// Preset yellow color
    Yellow,
    /// Preset green color
    Green,
    /// Preset cyan color
    Cyan,
    /// Preset blue color
    Blue,
    /// Preset purple color
    Purple,
    /// Preset gray color
    Gray,
    /// Custom RGB hex color (e.g., "#FF0000")
    Hex(String),
}

impl TryFrom<&Tag> for KanbanColumnDefinition {
    type Error = &'static str;

    fn try_from(value: &Tag) -> Result<Self, Self::Error> {
        let tag = value;
        
        // TODO: This is hacky, do it properly later
        if tag.kind().as_str() == "col" {
            let Some(tag_content) = tag.content() else { return Err("missing tag content") };
            let tag_vec = tag.clone().to_vec();
            Ok(KanbanColumnDefinition {
                id: tag_content.to_string(),
                label: tag_vec.get(2).ok_or("No label")?.to_string(),
                color: tag_vec.get(3).and_then(|c| Color::from_str(c))
            })
        }
        else {
            return Err("not color tag")
        }
    }
}

impl TryFrom<&Event> for KanbanBoard {
    type Error = &'static str;

    fn try_from(value: &Event) -> Result<Self, Self::Error> {
        // Verify this is a kanban board event (kind 35002)
        if value.kind != Kind::KanbanBoard {
            return Err("Event is not a kanban board (kind 35002)");
        }
        
        // Extract board ID from the "d" tag
        let id = value.tags.iter()
            .find(|tag| tag.kind() == TagKind::d())
            .and_then(|tag| tag.content())
            .ok_or("Missing required 'd' tag for board identifier")?
            .to_string();
        
        // Extract title from the "title" tag
        let title = value.tags.iter()
            .find(|tag| tag.kind() == TagKind::Title)
            .and_then(|tag| tag.content())
            .map(|s| s.to_string());
        
        // Extract description from the "description" tag
        let description = value.tags.iter()
            .find(|tag| tag.kind() == TagKind::Description)
            .and_then(|tag| tag.content())
            .map(|s| s.to_string());
        
        // Extract alt text from the "alt" tag
        let alt = value.tags.iter()
            .find(|tag| tag.kind() == TagKind::Alt)
            .and_then(|tag| tag.content())
            .map(|s| s.to_string());
        
        // Extract columns from "col" tags
        let columns = value.tags.iter()
            .filter(|tag| tag.kind().as_str() == "col")
            .map(|tag| KanbanColumnDefinition::try_from(tag))
            .collect::<Result<Vec<_>, _>>()?;
        
        if columns.is_empty() {
            return Err("Kanban board must have at least one column");
        }
        
        // Extract maintainers (pubkeys) from "p" tags
        let pubkey = value.tags.iter()
            .filter(|tag| tag.kind() == TagKind::p())
            .filter_map(|tag| tag.content())
            .filter_map(|pk_str| {
                // Try to parse the public key
                match PublicKey::from_hex(pk_str) {
                    Ok(pk) => Some(pk),
                    Err(_) => None,
                }
            })
            .collect::<Vec<_>>();
        
        // If no pubkeys were specified, the board owner is the only maintainer
        let pubkey = if pubkey.is_empty() {
            vec![value.pubkey.clone()]
        } else {
            pubkey
        };
        
        Ok(Self {
            id,
            title,
            description,
            alt,
            columns,
            pubkey,
        })
    }
}

impl TryFrom<Event> for KanbanSpecificTrackerData {
    type Error = TaskError;

    fn try_from(value: Event) -> Result<Self, Self::Error> {
        let event = value;
        
        let status = if event.content.is_empty() {
            KanbanTrackerStatus::Defer
        }
        else {
            KanbanTrackerStatus::Column(event.content.clone())
        };
        
        let rank: Option<u32> = event.tags.find(TagKind::custom("rank")).and_then(|tag| tag.content()).and_then(|tag_content| tag_content.parse::<u32>().ok());
        
        let task_metadata: TaskMetadata = TaskMetadata::try_from(&event.tags)?;
        
        Ok(KanbanSpecificTrackerData {
            status,
            rank,
            task_metadata,
        })
    }
}

impl Color {
    /// Parse a color string into a Color enum
    pub fn from_str(color: &str) -> Option<Self> {
        match color.to_lowercase().as_str() {
            "red" => Some(Color::Red),
            "orange" => Some(Color::Orange),
            "yellow" => Some(Color::Yellow),
            "green" => Some(Color::Green),
            "cyan" => Some(Color::Cyan),
            "blue" => Some(Color::Blue),
            "purple" => Some(Color::Purple),
            "gray" => Some(Color::Gray),
            _ if color.starts_with('#') => Some(Color::Hex(color.to_string())),
            _ => None,
        }
    }

    /// Convert Color to a string representation
    pub fn to_string(&self) -> String {
        match self {
            Color::Red => "red".to_string(),
            Color::Orange => "orange".to_string(),
            Color::Yellow => "yellow".to_string(),
            Color::Green => "green".to_string(),
            Color::Cyan => "cyan".to_string(),
            Color::Blue => "blue".to_string(),
            Color::Purple => "purple".to_string(),
            Color::Gray => "gray".to_string(),
            Color::Hex(hex) => hex.clone(),
        }
    }
}
