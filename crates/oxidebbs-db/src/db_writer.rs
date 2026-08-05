use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};

use decentdb::{Db, DbError};

pub type DbWriterResult<T> = Result<T, DbWriterError>;

type DbWriterDynResult = Result<DbWriterDynValue, DbWriterError>;
type DbWriterDynValue = Box<dyn Any + Send + 'static>;

type DbWriteFn = Box<dyn FnOnce(&Db) -> DbWriterDynResult + Send + 'static>;

enum QueuedWork {
    Execute {
        work: DbWriteFn,
        response_tx: mpsc::Sender<DbWriterDynResult>,
    },
    Shutdown,
}

pub struct DbWriter {
    command_tx: SyncSender<QueuedWork>,
    worker: Option<JoinHandle<()>>,
}

impl DbWriter {
    pub const DEFAULT_CAPACITY: usize = 32;

    pub fn new(db: Db) -> Self {
        Self::with_capacity(db, Self::DEFAULT_CAPACITY)
    }

    pub fn with_capacity(db: Db, capacity: usize) -> Self {
        let (command_tx, command_rx) = mpsc::sync_channel::<QueuedWork>(capacity);
        let worker = thread::spawn(move || {
            process_writes(db, command_rx);
        });
        Self {
            command_tx,
            worker: Some(worker),
        }
    }

    pub fn submit<T, F>(&self, work: F) -> DbWriterResult<DbWriteTicket<T>>
    where
        T: Send + 'static,
        F: FnOnce(&Db) -> Result<T, DbError> + Send + 'static,
    {
        let (response_tx, response_rx) = mpsc::channel();
        let command = QueuedWork::Execute {
            work: box_transactional_work(work),
            response_tx,
        };

        match self.command_tx.try_send(command) {
            Ok(()) => Ok(DbWriteTicket {
                response_rx: Some(response_rx),
                _marker: PhantomData,
            }),
            Err(TrySendError::Full(_)) => Err(DbWriterError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(DbWriterError::Shutdown),
        }
    }

    pub fn shutdown(mut self) -> DbWriterResult<()> {
        if self.command_tx.send(QueuedWork::Shutdown).is_err() {
            return Err(DbWriterError::Shutdown);
        }

        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }

        Ok(())
    }
}

impl Drop for DbWriter {
    fn drop(&mut self) {
        let _ = self.command_tx.send(QueuedWork::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn box_transactional_work<T, F>(work: F) -> DbWriteFn
where
    T: Send + 'static,
    F: FnOnce(&Db) -> Result<T, DbError> + Send + 'static,
{
    Box::new(move |db| match run_in_transaction(db, work) {
        Ok(value) => Ok(Box::new(value) as DbWriterDynValue),
        Err(error) => Err(error),
    })
}

fn run_in_transaction<T>(
    db: &Db,
    work: impl FnOnce(&Db) -> Result<T, DbError>,
) -> Result<T, DbWriterError>
where
    T: Send + 'static,
{
    db.begin_transaction()?;
    let result = work(db);
    match result {
        Ok(value) => match db.commit_transaction() {
            Ok(_) => Ok(value),
            Err(error) => {
                let _ = db.rollback_transaction();
                Err(DbWriterError::from(error))
            }
        },
        Err(error) => {
            let _ = db.rollback_transaction();
            Err(error.into())
        }
    }
}

fn process_writes(db: Db, command_rx: mpsc::Receiver<QueuedWork>) {
    for command in command_rx {
        match command {
            QueuedWork::Execute { work, response_tx } => {
                let _ = response_tx.send(work(&db));
            }
            QueuedWork::Shutdown => break,
        }
    }
}

pub struct DbWriteTicket<T> {
    response_rx: Option<mpsc::Receiver<DbWriterDynResult>>,
    _marker: PhantomData<T>,
}

impl<T> DbWriteTicket<T>
where
    T: Send + 'static,
{
    pub fn wait(self) -> DbWriterResult<T> {
        let response = match self.response_rx {
            Some(receiver) => receiver.recv(),
            None => return Err(DbWriterError::Shutdown),
        };
        match response {
            Ok(Ok(result)) => match result.downcast::<T>() {
                Ok(value) => Ok(*value),
                Err(_) => Err(DbWriterError::Internal(
                    "type mismatch in writer response".to_string(),
                )),
            },
            Ok(Err(error)) => Err(error),
            Err(_) => Err(DbWriterError::Shutdown),
        }
    }
}

#[derive(Debug)]
pub enum DbWriterError {
    Database(DbError),
    QueueFull,
    Internal(String),
    Shutdown,
}

impl fmt::Display for DbWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "{error}"),
            Self::QueueFull => write!(formatter, "DbWriter write queue is full"),
            Self::Internal(details) => write!(formatter, "{details}"),
            Self::Shutdown => write!(formatter, "DbWriter is not running"),
        }
    }
}

impl std::error::Error for DbWriterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::QueueFull | Self::Internal(_) | Self::Shutdown => None,
        }
    }
}

impl From<DbError> for DbWriterError {
    fn from(error: DbError) -> Self {
        Self::Database(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message_repo::MessageRecord;
    use crate::schema;
    use crate::user_repo::UserRecord;
    use crate::user_repo::insert_user;
    use decentdb::{DbConfig, Value};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    const USER_ID: &str = "00000000-0000-4000-8000-100000000020";
    const USER_AREA: &str = "00000000-0000-4000-8000-100000000021";
    const MESSAGE_ID: &str = "00000000-0000-4000-8000-100000000022";

    fn test_db() -> Db {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DB");
        schema::init_schema(&db).expect("init schema");
        db
    }

    fn ensure_user_and_area(db: &Db) {
        insert_user(
            db,
            &UserRecord {
                id: USER_ID.to_string(),
                alias: "alice".to_string(),
                real_name: "Alice".to_string(),
                email: None,
                password_hash: "hash".to_string(),
                security_level: 10,
                is_sysop: false,
                created_at: "2026-01-01T00:00:00.000000Z".to_string(),
                last_login_at: None,
                total_calls: 0,
                time_bank_minutes: 0,
                status: "active".to_string(),
            },
        )
        .expect("insert user");
        crate::message_repo::insert_message_area(
            db,
            &crate::message_repo::MessageAreaRecord {
                id: USER_AREA.to_string(),
                key: "general".to_string(),
                name: "General".to_string(),
                description: "main".to_string(),
                kind: "local".to_string(),
                network_id: None,
                read_security_level: 0,
                post_security_level: 10,
                moderated: false,
                enabled: true,
            },
        )
        .expect("insert area");
    }

    fn message_record() -> MessageRecord {
        MessageRecord {
            id: MESSAGE_ID.to_string(),
            area_id: USER_AREA.to_string(),
            author_user_id: USER_ID.to_string(),
            author_kind: "local".to_string(),
            author_display_name: String::new(),
            author_network_address: None,
            to_user_id: None,
            subject: "hello".to_string(),
            body: "world".to_string(),
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            reply_to_id: None,
            network_message_id: None,
            visibility: "normal".to_string(),
        }
    }

    #[test]
    fn ordered_execution_preserves_submission_order() {
        let db = test_db();
        ensure_user_and_area(&db);

        let writer = DbWriter::new(db.clone());
        let first = writer
            .submit(|db| {
                crate::message_repo::insert_message(db, &message_record())?;
                db.execute_with_params(
                    "UPDATE messages SET subject = $1 WHERE id = UUID_PARSE($2)",
                    &[
                        Value::Text("first".to_string()),
                        Value::Text(MESSAGE_ID.to_string()),
                    ],
                )?;
                Ok(())
            })
            .expect("submit first");
        let second = writer
            .submit(|db| {
                db.execute_with_params(
                    "UPDATE messages SET body = $1 WHERE id = UUID_PARSE($2)",
                    &[
                        Value::Text("ordered".to_string()),
                        Value::Text(MESSAGE_ID.to_string()),
                    ],
                )?;
                Ok(())
            })
            .expect("submit second");

        first.wait().expect("first done");
        second.wait().expect("second done");
        drop(writer);

        let message = crate::message_repo::find_message_by_id(&db, MESSAGE_ID)
            .expect("find")
            .expect("message");
        assert_eq!(message.subject, "first");
        assert_eq!(message.body, "ordered");
    }

    #[test]
    fn transaction_rollback_keeps_db_consistent_on_failure() {
        let db = test_db();
        let writer = DbWriter::new(db.clone());

        let ticket = writer
            .submit(|db| {
                db.execute(
                    "INSERT INTO message_areas (id, key, name, kind)
                     VALUES (UUID_PARSE('00000000-0000-4000-8000-100000000030'), 'dup', 'Duplicate', 'local')",
                )?;
                db.execute(
                    "INSERT INTO message_areas (id, key, name, kind)
                     VALUES (UUID_PARSE('00000000-0000-4000-8000-100000000031'), 'dup', 'Duplicate2', 'local')",
                )?;
                Ok(())
            })
            .expect("submit tx");

        assert!(ticket.wait().is_err());
        drop(writer);

        let result = db
            .execute("SELECT COUNT(*) FROM message_areas WHERE key = 'dup'")
            .expect("count")
            .rows()
            .first()
            .and_then(|row| row.values().first())
            .and_then(|value| match value {
                Value::Int64(count) => Some(*count),
                _ => None,
            });
        assert_eq!(result, Some(0));
    }

    #[test]
    fn queue_full_is_reported_as_backpressure() {
        let db = test_db();
        let writer = DbWriter::with_capacity(db.clone(), 0);
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let mut first = None;
        let mut attempts = 0;
        while first.is_none() && attempts < 200 {
            let started_tx = started_tx.clone();
            let release_rx = Arc::clone(&release_rx);
            match writer.submit(move |_db| {
                started_tx.send(()).expect("first job started");
                release_rx
                    .lock()
                    .unwrap()
                    .recv()
                    .expect("release first job");
                Ok(())
            }) {
                Ok(ticket) => {
                    first = Some(ticket);
                    break;
                }
                Err(DbWriterError::QueueFull) => {
                    attempts += 1;
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("unexpected writer submit error: {error}"),
            }
        }
        let first = first.expect("first submit eventually succeeds");

        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first started");
        let second = writer.submit::<(), _>(|_db| Ok(()));
        assert!(matches!(second, Err(DbWriterError::QueueFull)));

        let _ = release_tx.send(());
        first.wait().expect("first");
        drop(writer);

        assert_eq!(
            db.execute("SELECT COUNT(*) FROM message_areas WHERE key = 'blocked'")
                .expect("query")
                .rows()
                .first()
                .and_then(|row| row.values().first())
                .and_then(|value| match value {
                    Value::Int64(value) => Some(*value),
                    _ => None,
                }),
            Some(0),
        );
    }

    #[test]
    fn shutdown_drains_queued_writes_before_exit() {
        let db = test_db();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        {
            let writer = DbWriter::new(db.clone());
            let _ticket = writer
                .submit(move |db| {
                    started_tx.send(()).expect("started");
                    let result = crate::message_repo::insert_message_area(
                        db,
                        &crate::message_repo::MessageAreaRecord {
                            id: "00000000-0000-4000-8000-100000000050".to_string(),
                            key: "shutdown".to_string(),
                            name: "Shutdown".to_string(),
                            description: "Drain test".to_string(),
                            kind: "local".to_string(),
                            network_id: None,
                            read_security_level: 0,
                            post_security_level: 10,
                            moderated: false,
                            enabled: true,
                        },
                    );
                    done_tx.send(()).expect("done");
                    result
                })
                .expect("submit");
            // Writer dropped at end of scope here.
        }

        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("started");
        done_rx.recv_timeout(Duration::from_secs(1)).expect("done");

        let found = crate::message_repo::find_message_area_by_key(&db, "shutdown")
            .expect("find area")
            .expect("area exists");
        assert_eq!(found.key, "shutdown");
    }
}
