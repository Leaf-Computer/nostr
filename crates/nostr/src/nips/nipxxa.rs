// Copyright (c) 2022-2023 Yuki Kishimoto
// Copyright (c) 2023-2025 Rust Nostr Developers
// Distributed under the MIT software license

//! NIPXXA: Tasks
//!
//! <https://github.com/nostr-protocol/nips/blob/19f650b38a4ca08aa01ef2c66d708cc8c14ffd9a/XXA.md>

#![allow(clippy::wrong_self_convention)]

use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::convert::TryFrom;
use core::fmt;
use core::str::FromStr;

use crate::event::builder::{Error, EventBuilder};
use crate::nips::nip01;
use crate::types::url::Url;
use crate::{Alphabet, Event, Tags};
use crate::{Kind, PublicKey, Tag, TagKind, Timestamp};

/// Task
///
/// A representation of a task/to-do item/reminder. (`kind:35001` event)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Task {
    /// Unique identifier for the task
    pub id: String,
    /// Task description (in Markdown format)
    pub description: String,
    /// Task metadata from the tags
    pub metadata: TaskMetadata,
}

/// TaskMetadata
///
/// A representation of the tags metadata around a task or task-like object
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskMetadata {
    /// Task title (optional)
    pub title: Option<String>,
    /// URL pointing to an image to be shown along with the title (optional)
    pub image: Option<Url>,
    /// Timestamp when the task was first created (optional)
    pub published_at: Option<Timestamp>,
    /// Due date of the task (optional)
    pub due_at: Option<Timestamp>,
    /// Whether the task is archived (optional)
    pub archived: Option<bool>,
    /// Tags for categorizing the task
    pub tags: Vec<String>,
    /// References to users and their roles
    pub users: Vec<(PublicKey, TaskUserRole)>,
}

/// User roles in a Task
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskUserRole {
    /// No specific role (mentioned or CC'd)
    None,
    /// User is assigned to the task
    Assignee,
    /// User is the client or requester of the task
    Client,
    /// Custom role
    Custom(String),
}

impl TaskUserRole {
    /// Converts the TaskUserRole to an Option<String>
    /// Returns None for the None variant, and Some(String) for all other variants
    pub fn to_string(&self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Assignee => Some("assignee".to_string()),
            Self::Client => Some("client".to_string()),
            Self::Custom(role) => Some(role.clone()),
        }
    }
}

impl fmt::Display for TaskUserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, ""),
            Self::Assignee => write!(f, "assignee"),
            Self::Client => write!(f, "client"),
            Self::Custom(role) => write!(f, "{}", role),
        }
    }
}

impl From<Option<String>> for TaskUserRole {
    fn from(role: Option<String>) -> Self {
        match role {
            None => Self::None,
            Some(role) => match role.as_str() {
                "assignee" => Self::Assignee,
                "client" => Self::Client,
                "" => Self::None,
                _ => Self::Custom(role),
            },
        }
    }
}

impl TaskMetadata {
    /// Create a new empty object
    pub fn new() -> Self {
        Self {
            title: None,
            image: None,
            published_at: None,
            due_at: None,
            archived: None,
            tags: Vec::new(),
            users: Vec::new(),
        }
    }

    /// Set task title
    pub fn title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    /// Set task image URL
    pub fn image(mut self, image: Url) -> Self {
        self.image = Some(image);
        self
    }

    /// Set task published timestamp
    pub fn published_at(mut self, timestamp: Timestamp) -> Self {
        self.published_at = Some(timestamp);
        self
    }

    /// Set task due timestamp
    pub fn due_at(mut self, timestamp: Timestamp) -> Self {
        self.due_at = Some(timestamp);
        self
    }

    /// Mark task as archived
    pub fn archived(mut self, archived: bool) -> Self {
        self.archived = Some(archived);
        self
    }

    /// Add a tag for categorizing the task
    pub fn add_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    /// Add multiple tags for categorizing the task
    pub fn add_tags(mut self, tags: Vec<String>) -> Self {
        self.tags.extend(tags);
        self
    }

    /// Add a user reference with a role
    pub fn add_user(mut self, pubkey: PublicKey, role: TaskUserRole) -> Self {
        self.users.push((pubkey, role));
        self
    }
}
    
impl Into<Tags> for TaskMetadata {
    fn into(self) -> Tags {
        let mut tags: Vec<Tag> = Vec::with_capacity(1 + self.users.len() + self.tags.len() + 5);

        // Add title
        if let Some(title) = self.title {
            tags.push(Tag::custom(
                TagKind::Custom(Cow::Borrowed("title")),
                vec![title],
            ));
        }

        // Add image
        if let Some(image) = self.image {
            tags.push(Tag::custom(
                TagKind::Custom(Cow::Borrowed("image")),
                vec![image.to_string()],
            ));
        }

        // Add published_at
        if let Some(timestamp) = self.published_at {
            tags.push(Tag::custom(
                TagKind::Custom(Cow::Borrowed("published_at")),
                vec![timestamp.to_string()],
            ));
        }

        // Add due_at
        if let Some(timestamp) = self.due_at {
            tags.push(Tag::custom(
                TagKind::Custom(Cow::Borrowed("due_at")),
                vec![timestamp.to_string()],
            ));
        }

        // Add archived
        if let Some(archived) = self.archived {
            if archived {
                tags.push(Tag::custom(
                    TagKind::Custom(Cow::Borrowed("archived")),
                    vec!["true".to_string()],
                ));
            }
        }

        // Add tags
        for tag in self.tags {
            tags.push(Tag::hashtag(tag));
        }

        // Add user references
        for (pubkey, role) in self.users {
            let role_str = role.to_string().unwrap_or_default();
            if role_str.is_empty() {
                tags.push(Tag::public_key(pubkey));
            } else {
                let mut values = vec![pubkey.to_string()];
                values.push(role_str);
                tags.push(Tag::custom(TagKind::single_letter(Alphabet::P, false), values));
            }
        }
        
        Tags::from_list(tags)
    }
}

impl Task {
    /// Create a new Task with a unique identifier and description
    pub fn new(id: String, description: String) -> Self {
        Self {
            id,
            description,
            metadata: TaskMetadata {
                title: None,
                image: None,
                published_at: None,
                due_at: None,
                archived: None,
                tags: Vec::new(),
                users: Vec::new(),
            }
        }
    }

    /// Set task title
    pub fn title(mut self, title: String) -> Self {
        self.metadata.title = Some(title);
        self
    }

    /// Set task image URL
    pub fn image(mut self, image: Url) -> Self {
        self.metadata.image = Some(image);
        self
    }

    /// Set task published timestamp
    pub fn published_at(mut self, timestamp: Timestamp) -> Self {
        self.metadata.published_at = Some(timestamp);
        self
    }

    /// Set task due timestamp
    pub fn due_at(mut self, timestamp: Timestamp) -> Self {
        self.metadata.due_at = Some(timestamp);
        self
    }

    /// Mark task as archived
    pub fn archived(mut self, archived: bool) -> Self {
        self.metadata.archived = Some(archived);
        self
    }

    /// Add a tag for categorizing the task
    pub fn add_tag(mut self, tag: String) -> Self {
        self.metadata.tags.push(tag);
        self
    }

    /// Add multiple tags for categorizing the task
    pub fn add_tags(mut self, tags: Vec<String>) -> Self {
        self.metadata.tags.extend(tags);
        self
    }

    /// Add a user reference with a role
    pub fn add_user(mut self, pubkey: PublicKey, role: TaskUserRole) -> Self {
        self.metadata.users.push((pubkey, role));
        self
    }

    pub(crate) fn to_event_builder(self) -> Result<EventBuilder, Error> {
        if self.id.is_empty() {
            return Err(Error::NIP01(nip01::Error::InvalidCoordinate));
        }

        let tags: Tags = self.metadata.into();

        // Build
        Ok(EventBuilder::new(Kind::Task, self.description).tags(tags))
    }
}

/// Error type for Task parsing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskError {
    /// Wrong kind
    WrongKind(Kind),
    /// Missing identifier
    MissingIdentifier,
    /// Missing content
    MissingContent,
    /// Invalid URL
    InvalidUrl(String),
    /// Invalid timestamp
    InvalidTimestamp(String),
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongKind(kind) => write!(f, "Wrong kind: {}", kind),
            Self::MissingIdentifier => write!(f, "Missing identifier"),
            Self::MissingContent => write!(f, "Missing content"),
            Self::InvalidUrl(url) => write!(f, "Invalid URL: {}", url),
            Self::InvalidTimestamp(timestamp) => write!(f, "Invalid timestamp: {}", timestamp),
        }
    }
}

impl TryFrom<&Tags> for TaskMetadata {
    type Error = TaskError;

    fn try_from(value: &Tags) -> Result<Self, Self::Error> {
        let mut task_metadata = TaskMetadata::new();
        
        for tag in value.iter() {
            match tag.kind() {
                TagKind::Title => {
                    if let Some(title) = tag.content() {
                        task_metadata = task_metadata.title(title.to_string());
                    }
                },
                TagKind::Image => {
                    if let Some(image_url) = tag.content() {
                        match Url::parse(image_url) {
                            Ok(url) => task_metadata = task_metadata.image(url),
                            Err(_) => return Err(TaskError::InvalidUrl(image_url.to_string())),
                        }
                    }
                },
                TagKind::PublishedAt => {
                    if let Some(timestamp_str) = tag.content() {
                        match timestamp_str.parse::<u64>() {
                            Ok(timestamp) => task_metadata = task_metadata.published_at(Timestamp::from_secs(timestamp)),
                            Err(_) => return Err(TaskError::InvalidTimestamp(timestamp_str.to_string())),
                        }
                    }
                },
                TagKind::DueAt => {
                    if let Some(timestamp_str) = tag.content() {
                        match timestamp_str.parse::<u64>() {
                            Ok(timestamp) => task_metadata = task_metadata.due_at(Timestamp::from_secs(timestamp)),
                            Err(_) => return Err(TaskError::InvalidTimestamp(timestamp_str.to_string())),
                        }
                    }
                },
                TagKind::Archived => {
                    task_metadata = task_metadata.archived(true);
                },
                _ => {
                    if tag.kind() == TagKind::t() {
                        if let Some(hashtag) = tag.content() {
                            task_metadata = task_metadata.add_tag(hashtag.to_string());
                        }
                    }
                    else if tag.kind() == TagKind::p() {
                        if let Some(pubkey_str) = tag.content() {
                            if let Ok(pubkey) = PublicKey::from_str(pubkey_str) {
                                let role = match tag.clone().to_vec().get(2) {
                                    Some(role_value) => Some(role_value.to_string()),
                                    None => None,
                                };
                                task_metadata = task_metadata.add_user(pubkey, TaskUserRole::from(role));
                            }
                        }
                    }
                }
            }
        }
        
        Ok(task_metadata)
    }
}

impl TryFrom<&Event> for Task {
    type Error = TaskError;

    fn try_from(event: &Event) -> Result<Self, Self::Error> {
        // Verify kind
        if event.kind != Kind::Task {
            return Err(TaskError::WrongKind(event.kind));
        }

        // Get identifier
        let id = event
            .tags
            .iter()
            .find_map(|tag| {
                if tag.kind() == TagKind::d() {
                    tag.content().map(|v| v.to_string())
                } else {
                    None
                }
            })
            .ok_or(TaskError::MissingIdentifier)?;

        // Start building the task
        let mut task = Task::new(id, event.content.clone());

        task.metadata = TaskMetadata::try_from(&event.tags)?;

        Ok(task)
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use core::str::FromStr;

    use super::*;
    use crate::{Event, Keys, Tags};

    #[test]
    fn test_task() {
        let task = Task::new(
            "333e500a-7d80-4e7b-beb1-ad1956a6150a".to_string(),
            "This task is a placeholder for the description of the task.".to_string(),
        )
        .title("Example task".to_string())
        .published_at(Timestamp::from_secs(1296962229))
        .due_at(Timestamp::from_secs(1298962229))
        .add_tag("examples".to_string())
        .add_user(
            PublicKey::from_str("b3e392b11f5d4f28321cedd09303a748acfd0487aea5a7450b3481c60b6e4f87")
                .unwrap(),
            TaskUserRole::Assignee,
        );

        let keys = Keys::generate();
        let event: Event = task
            .to_event_builder()
            .unwrap()
            .sign_with_keys(&keys)
            .unwrap();

        assert_eq!(event.kind, Kind::Task);
        assert_eq!(
            event.content,
            "This task is a placeholder for the description of the task."
        );

        let tags = Tags::parse([
            vec!["d", "333e500a-7d80-4e7b-beb1-ad1956a6150a"],
            vec!["title", "Example task"],
            vec!["published_at", "1296962229"],
            vec!["due_at", "1298962229"],
            vec!["t", "examples"],
            vec![
                "p",
                "b3e392b11f5d4f28321cedd09303a748acfd0487aea5a7450b3481c60b6e4f87",
                "assignee",
            ],
        ])
        .unwrap();
        // Compare tags
        assert_eq!(event.tags, tags);
    }

    #[test]
    fn test_task_with_archived() {
        let task = Task::new(
            "333e500a-7d80-4e7b-beb1-ad1956a6150a".to_string(),
            "This is an archived task.".to_string(),
        )
        .archived(true);

        let keys = Keys::generate();
        let event: Event = task
            .to_event_builder()
            .unwrap()
            .sign_with_keys(&keys)
            .unwrap();

        assert_eq!(event.kind, Kind::Task);
        assert_eq!(event.content, "This is an archived task.");

        let tags = Tags::parse([
            vec!["d", "333e500a-7d80-4e7b-beb1-ad1956a6150a"],
            vec!["archived", "true"],
        ])
        .unwrap();
        // Compare tags
        assert_eq!(event.tags, tags);
    }

    #[test]
    fn test_task_with_multiple_users() {
        let task = Task::new(
            "333e500a-7d80-4e7b-beb1-ad1956a6150a".to_string(),
            "Task with multiple users.".to_string(),
        )
        .add_user(
            PublicKey::from_str("b3e392b11f5d4f28321cedd09303a748acfd0487aea5a7450b3481c60b6e4f87")
                .unwrap(),
            TaskUserRole::Assignee,
        )
        .add_user(
            PublicKey::from_str("32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245")
                .unwrap(),
            TaskUserRole::Client,
        )
        .add_user(
            PublicKey::from_str("8f0e957f3d75c7428454a22ea901d2cd589d34fdd3b32f632ce7749dbd8a2ead")
                .unwrap(),
            TaskUserRole::None,
        );

        let keys = Keys::generate();
        let event: Event = task
            .to_event_builder()
            .unwrap()
            .sign_with_keys(&keys)
            .unwrap();

        assert_eq!(event.kind, Kind::Task);
        assert_eq!(event.content, "Task with multiple users.");

        let tags = Tags::parse([
            vec!["d", "333e500a-7d80-4e7b-beb1-ad1956a6150a"],
            vec![
                "p",
                "b3e392b11f5d4f28321cedd09303a748acfd0487aea5a7450b3481c60b6e4f87",
                "assignee",
            ],
            vec![
                "p",
                "32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245",
                "client",
            ],
            vec![
                "p",
                "8f0e957f3d75c7428454a22ea901d2cd589d34fdd3b32f632ce7749dbd8a2ead",
            ],
        ])
        .unwrap();
        // Compare tags
        assert_eq!(event.tags, tags);
    }

    #[test]
    fn test_task_with_image() {
        let task = Task::new(
            "333e500a-7d80-4e7b-beb1-ad1956a6150a".to_string(),
            "Task with an image.".to_string(),
        )
        .image(Url::parse("https://example.com/image.jpg").unwrap());

        let keys = Keys::generate();
        let event: Event = task
            .to_event_builder()
            .unwrap()
            .sign_with_keys(&keys)
            .unwrap();

        assert_eq!(event.kind, Kind::Task);
        assert_eq!(event.content, "Task with an image.");

        let tags = Tags::parse([
            vec!["d", "333e500a-7d80-4e7b-beb1-ad1956a6150a"],
            vec!["image", "https://example.com/image.jpg"],
        ])
        .unwrap();
        // Compare tags
        assert_eq!(event.tags, tags);
    }

    #[test]
    fn test_task_with_multiple_tags() {
        let task = Task::new(
            "333e500a-7d80-4e7b-beb1-ad1956a6150a".to_string(),
            "Task with multiple tags.".to_string(),
        )
        .add_tags(vec![
            "work".to_string(),
            "urgent".to_string(),
            "meeting".to_string(),
        ]);

        let keys = Keys::generate();
        let event: Event = task
            .to_event_builder()
            .unwrap()
            .sign_with_keys(&keys)
            .unwrap();

        assert_eq!(event.kind, Kind::Task);
        assert_eq!(event.content, "Task with multiple tags.");

        let tags = Tags::parse([
            vec!["d", "333e500a-7d80-4e7b-beb1-ad1956a6150a"],
            vec!["t", "work"],
            vec!["t", "urgent"],
            vec!["t", "meeting"],
        ])
        .unwrap();
        // Compare tags
        assert_eq!(event.tags, tags);
    }

    #[test]
    fn test_try_from_event() {
        let keys = Keys::generate();
        
        // Create a task
        let original_task = Task::new(
            "333e500a-7d80-4e7b-beb1-ad1956a6150a".to_string(),
            "This is a test task".to_string(),
        )
        .title("Test Task".to_string())
        .published_at(Timestamp::from_secs(1296962229))
        .due_at(Timestamp::from_secs(1298962229))
        .add_tag("test".to_string())
        .add_user(
            PublicKey::from_str("b3e392b11f5d4f28321cedd09303a748acfd0487aea5a7450b3481c60b6e4f87")
                .unwrap(),
            TaskUserRole::Assignee,
        );

        // Convert to event
        let event = original_task
            .to_event_builder()
            .unwrap()
            .sign_with_keys(&keys)
            .unwrap();

        // Convert back to task
        let parsed_task = Task::try_from(&event).unwrap();

        // Check values
        assert_eq!(parsed_task.id, "333e500a-7d80-4e7b-beb1-ad1956a6150a");
        assert_eq!(parsed_task.description, "This is a test task");
        assert_eq!(parsed_task.metadata.title, Some("Test Task".to_string()));
        assert_eq!(parsed_task.metadata.published_at, Some(Timestamp::from_secs(1296962229)));
        assert_eq!(parsed_task.metadata.due_at, Some(Timestamp::from_secs(1298962229)));
        assert_eq!(parsed_task.metadata.tags, vec!["test".to_string()]);
        assert_eq!(parsed_task.metadata.users.len(), 1);
        assert_eq!(parsed_task.metadata.users[0].0.to_string(), "b3e392b11f5d4f28321cedd09303a748acfd0487aea5a7450b3481c60b6e4f87");
        assert_eq!(parsed_task.metadata.users[0].1, TaskUserRole::Assignee);
    }

    #[test]
    fn test_try_from_event_wrong_kind() {
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::TextNote, "This is not a task")
            .tag(Tag::identifier("test-id"))
            .sign_with_keys(&keys)
            .unwrap();

        let result = Task::try_from(&event);
        assert!(matches!(result, Err(TaskError::WrongKind(_))));
    }

    #[test]
    fn test_try_from_event_missing_identifier() {
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::Task, "This is a task without identifier")
            .sign_with_keys(&keys)
            .unwrap();

        let result = Task::try_from(&event);
        assert!(matches!(result, Err(TaskError::MissingIdentifier)));
    }
}
