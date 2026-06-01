use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AreaKind {
    Local,
    EchoMail,
    NetMail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageArea {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: String,
    pub kind: AreaKind,
    pub network_id: Option<String>,
    pub read_security_level: i32,
    pub post_security_level: i32,
    pub moderated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageVisibility {
    Normal,
    Deleted,
    PendingModeration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub area_id: String,
    pub author_user_id: String,
    pub to_user_id: Option<String>,
    pub subject: String,
    pub body: String,
    pub created_at: String,
    pub reply_to_id: Option<String>,
    pub network_message_id: Option<String>,
    pub visibility: MessageVisibility,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostMessageCommand {
    pub id: String,
    pub author_user_id: String,
    pub author_security_level: i32,
    pub to_user_id: Option<String>,
    pub subject: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyMessageCommand {
    pub id: String,
    pub author_user_id: String,
    pub author_security_level: i32,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModerationDecision {
    Approve,
    Delete,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MessageCommandError {
    #[error("subject is required")]
    MissingSubject,

    #[error("message body is required")]
    MissingBody,

    #[error("permission denied: requires security level {required}, user has {actual}")]
    PermissionDenied { required: i32, actual: i32 },

    #[error("cannot reply to deleted message")]
    DeletedMessage,
}

pub fn post_message(
    area: &MessageArea,
    command: PostMessageCommand,
) -> Result<Message, MessageCommandError> {
    ensure_can_post(area, command.author_security_level)?;
    validate_subject_body(&command.subject, &command.body)?;

    Ok(Message {
        id: command.id,
        area_id: area.id.clone(),
        author_user_id: command.author_user_id,
        to_user_id: command.to_user_id,
        subject: command.subject.trim().to_string(),
        body: command.body,
        created_at: command.created_at,
        reply_to_id: None,
        network_message_id: None,
        visibility: if area.moderated {
            MessageVisibility::PendingModeration
        } else {
            MessageVisibility::Normal
        },
    })
}

pub fn reply_message(
    area: &MessageArea,
    original: &Message,
    command: ReplyMessageCommand,
) -> Result<Message, MessageCommandError> {
    if original.visibility == MessageVisibility::Deleted {
        return Err(MessageCommandError::DeletedMessage);
    }
    ensure_can_post(area, command.author_security_level)?;
    validate_subject_body(&original.subject, &command.body)?;

    Ok(Message {
        id: command.id,
        area_id: area.id.clone(),
        author_user_id: command.author_user_id,
        to_user_id: original.to_user_id.clone(),
        subject: reply_subject(&original.subject),
        body: command.body,
        created_at: command.created_at,
        reply_to_id: Some(original.id.clone()),
        network_message_id: None,
        visibility: if area.moderated {
            MessageVisibility::PendingModeration
        } else {
            MessageVisibility::Normal
        },
    })
}

pub fn readable_messages<'a>(
    area: &MessageArea,
    messages: &'a [Message],
    security_level: i32,
) -> Result<Vec<&'a Message>, MessageCommandError> {
    ensure_can_read(area, security_level)?;
    Ok(messages
        .iter()
        .filter(|message| {
            message.area_id == area.id && matches!(message.visibility, MessageVisibility::Normal)
        })
        .collect())
}

pub fn apply_moderation(message: &mut Message, decision: ModerationDecision) {
    message.visibility = match decision {
        ModerationDecision::Approve => MessageVisibility::Normal,
        ModerationDecision::Delete => MessageVisibility::Deleted,
    };
}

pub fn private_mail_area() -> MessageArea {
    MessageArea {
        id: "private-mail".to_string(),
        key: "mail".to_string(),
        name: "Private Mail".to_string(),
        description: "Local private mail".to_string(),
        kind: AreaKind::Local,
        network_id: None,
        read_security_level: 10,
        post_security_level: 10,
        moderated: false,
    }
}

fn ensure_can_read(area: &MessageArea, actual: i32) -> Result<(), MessageCommandError> {
    if actual < area.read_security_level {
        Err(MessageCommandError::PermissionDenied {
            required: area.read_security_level,
            actual,
        })
    } else {
        Ok(())
    }
}

fn ensure_can_post(area: &MessageArea, actual: i32) -> Result<(), MessageCommandError> {
    if actual < area.post_security_level {
        Err(MessageCommandError::PermissionDenied {
            required: area.post_security_level,
            actual,
        })
    } else {
        Ok(())
    }
}

fn validate_subject_body(subject: &str, body: &str) -> Result<(), MessageCommandError> {
    if subject.trim().is_empty() {
        return Err(MessageCommandError::MissingSubject);
    }
    if body.trim().is_empty() {
        return Err(MessageCommandError::MissingBody);
    }
    Ok(())
}

fn reply_subject(subject: &str) -> String {
    if subject.to_ascii_lowercase().starts_with("re:") {
        subject.trim().to_string()
    } else {
        format!("Re: {}", subject.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_area(moderated: bool) -> MessageArea {
        MessageArea {
            id: "general".to_string(),
            key: "general".to_string(),
            name: "General".to_string(),
            description: "General discussion".to_string(),
            kind: AreaKind::Local,
            network_id: None,
            read_security_level: 0,
            post_security_level: 10,
            moderated,
        }
    }

    fn post_command(subject: &str) -> PostMessageCommand {
        PostMessageCommand {
            id: "msg-1".to_string(),
            author_user_id: "uid-1".to_string(),
            author_security_level: 10,
            to_user_id: None,
            subject: subject.to_string(),
            body: "Hello".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn post_message_command_creates_visible_local_message() {
        let message = post_message(&local_area(false), post_command(" News ")).expect("post");

        assert_eq!(message.subject, "News");
        assert_eq!(message.reply_to_id, None);
        assert_eq!(message.visibility, MessageVisibility::Normal);
    }

    #[test]
    fn moderated_area_marks_new_posts_pending() {
        let message = post_message(&local_area(true), post_command("News")).expect("post");

        assert_eq!(message.visibility, MessageVisibility::PendingModeration);
    }

    #[test]
    fn post_message_rejects_low_security_user() {
        let mut command = post_command("News");
        command.author_security_level = 5;

        let error = post_message(&local_area(false), command).expect_err("low security");

        assert_eq!(
            error,
            MessageCommandError::PermissionDenied {
                required: 10,
                actual: 5
            }
        );
    }

    #[test]
    fn reply_command_links_to_original_message() {
        let area = local_area(false);
        let original = post_message(&area, post_command("News")).expect("post original");
        let reply = reply_message(
            &area,
            &original,
            ReplyMessageCommand {
                id: "msg-2".to_string(),
                author_user_id: "uid-2".to_string(),
                author_security_level: 10,
                body: "Reply body".to_string(),
                created_at: "2026-01-02T00:00:00Z".to_string(),
            },
        )
        .expect("reply");

        assert_eq!(reply.subject, "Re: News");
        assert_eq!(reply.reply_to_id.as_deref(), Some("msg-1"));
    }

    #[test]
    fn private_mail_foundation_uses_recipient_field() {
        let area = private_mail_area();
        let mut command = post_command("Private");
        command.to_user_id = Some("uid-2".to_string());

        let message = post_message(&area, command).expect("private post");

        assert_eq!(message.to_user_id.as_deref(), Some("uid-2"));
        assert_eq!(area.key, "mail");
    }

    #[test]
    fn local_moderation_changes_visibility() {
        let area = local_area(true);
        let mut message = post_message(&area, post_command("News")).expect("post");

        apply_moderation(&mut message, ModerationDecision::Approve);
        assert_eq!(message.visibility, MessageVisibility::Normal);

        apply_moderation(&mut message, ModerationDecision::Delete);
        assert_eq!(message.visibility, MessageVisibility::Deleted);
    }

    #[test]
    fn read_command_filters_non_visible_messages() {
        let area = local_area(false);
        let visible = post_message(&area, post_command("Visible")).expect("post");
        let mut deleted = visible.clone();
        deleted.id = "msg-2".to_string();
        deleted.visibility = MessageVisibility::Deleted;

        let messages = vec![visible, deleted];
        let readable = readable_messages(&area, &messages, 10).expect("read");

        assert_eq!(readable.len(), 1);
        assert_eq!(readable[0].id, "msg-1");
    }
}
