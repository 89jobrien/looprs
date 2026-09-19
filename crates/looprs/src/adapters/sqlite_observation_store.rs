//! SQLite implementation of the observation write and query ports.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use looprs_core::observation::Observation;
use looprs_core::ports::{ObservationQuery, ObservationStore};
use looprs_core::types::ToolId;
use rusqlite::{Connection, OpenFlags, ToSql, params};

const CREATE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS observations (
    session_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    input TEXT NOT NULL,
    output TEXT NOT NULL,
    tool_use_id TEXT,
    timestamp INTEGER NOT NULL,
    context TEXT
)";

const REQUIRED_COLUMNS: [(&str, &str); 7] = [
    ("session_id", "TEXT"),
    ("tool_name", "TEXT"),
    ("input", "TEXT"),
    ("output", "TEXT"),
    ("tool_use_id", "TEXT"),
    ("timestamp", "INTEGER"),
    ("context", "TEXT"),
];

/// SQLite-backed observation writer and query adapter.
///
/// Construction does not touch the filesystem. Writes create and validate the
/// schema; reads require an existing compatible database and never create one.
///
/// ```no_run
/// use looprs::{ObservationQuery, ObservationStore, SqliteObservationStore};
///
/// let store = SqliteObservationStore::new("observations.db");
/// store.save(&[])?;
/// let recent = store.recent(10)?;
/// assert!(recent.is_empty());
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct SqliteObservationStore {
    path: PathBuf,
}

impl SqliteObservationStore {
    /// Target an observation database at `path` without opening it.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Return the configured database path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn open_for_write(&self) -> Result<Connection> {
        let connection = Connection::open(&self.path).with_context(|| {
            format!(
                "failed to open observation database for writing: {}",
                self.path.display()
            )
        })?;
        connection
            .execute_batch(CREATE_SCHEMA)
            .context("failed to initialize observations schema")?;
        validate_schema(&connection)?;
        Ok(connection)
    }

    fn open_for_query(&self) -> Result<Connection> {
        if !self.path.exists() {
            bail!(
                "observation database does not exist: {}",
                self.path.display()
            );
        }
        let connection = Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| {
                format!(
                    "failed to open observation database for reading: {}",
                    self.path.display()
                )
            })?;
        validate_schema(&connection)?;
        Ok(connection)
    }

    fn query(&self, sql: &str, parameters: &[&dyn ToSql]) -> Result<Vec<Observation>> {
        let connection = self.open_for_query()?;
        let mut statement = connection
            .prepare(sql)
            .context("failed to prepare observation query")?;
        let rows = statement
            .query_map(parameters, |row| {
                Ok(RawObservation {
                    session_id: row.get(0)?,
                    tool_name: row.get(1)?,
                    input: row.get(2)?,
                    output: row.get(3)?,
                    tool_use_id: row.get(4)?,
                    timestamp: row.get(5)?,
                    context: row.get(6)?,
                })
            })
            .context("failed to execute observation query")?;

        rows.enumerate()
            .map(|(index, row)| {
                let raw =
                    row.with_context(|| format!("failed to decode observation row {index}"))?;
                raw.try_into_observation(index)
            })
            .collect()
    }
}

impl ObservationStore for SqliteObservationStore {
    fn save(&self, observations: &[Observation]) -> Result<()> {
        let mut connection = self.open_for_write()?;
        let transaction = connection
            .transaction()
            .context("failed to start observation transaction")?;
        {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO observations
                     (session_id, tool_name, input, output, tool_use_id, timestamp, context)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .context("failed to prepare observation insert")?;
            for observation in observations {
                let timestamp = i64::try_from(observation.timestamp).with_context(|| {
                    format!(
                        "observation timestamp {} exceeds SQLite INTEGER range",
                        observation.timestamp
                    )
                })?;
                statement
                    .execute(params![
                        &observation.session_id,
                        &observation.tool_name,
                        observation.input.to_string(),
                        &observation.output,
                        observation.tool_use_id.as_ref().map(ToolId::as_str),
                        timestamp,
                        observation.context.as_deref(),
                    ])
                    .context("failed to insert observation")?;
            }
        }
        transaction
            .commit()
            .context("failed to commit observations")?;
        Ok(())
    }
}

impl ObservationQuery for SqliteObservationStore {
    fn for_session(&self, session_id: &str) -> Result<Vec<Observation>> {
        self.query(
            "SELECT session_id, tool_name, input, output, tool_use_id, timestamp, context
             FROM observations WHERE session_id = ?1 ORDER BY timestamp ASC, rowid ASC",
            &[&session_id],
        )
    }

    fn by_tool(&self, tool_name: &str, limit: usize) -> Result<Vec<Observation>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let limit = limit_to_i64(limit);
        self.query(
            "SELECT session_id, tool_name, input, output, tool_use_id, timestamp, context
             FROM observations WHERE tool_name = ?1
             ORDER BY timestamp DESC, rowid DESC LIMIT ?2",
            &[&tool_name, &limit],
        )
    }

    fn recent(&self, limit: usize) -> Result<Vec<Observation>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let limit = limit_to_i64(limit);
        self.query(
            "SELECT session_id, tool_name, input, output, tool_use_id, timestamp, context
             FROM observations ORDER BY timestamp DESC, rowid DESC LIMIT ?1",
            &[&limit],
        )
    }
}

#[derive(Debug)]
struct RawObservation {
    session_id: String,
    tool_name: String,
    input: String,
    output: String,
    tool_use_id: Option<String>,
    timestamp: i64,
    context: Option<String>,
}

impl RawObservation {
    fn try_into_observation(self, row: usize) -> Result<Observation> {
        let input = serde_json::from_str(&self.input)
            .with_context(|| format!("invalid observation input JSON in row {row}"))?;
        let timestamp = u64::try_from(self.timestamp)
            .map_err(|_| anyhow!("invalid negative observation timestamp in row {row}"))?;
        Ok(Observation {
            session_id: self.session_id,
            tool_name: self.tool_name,
            input,
            output: self.output,
            tool_use_id: self.tool_use_id.map(ToolId::new),
            timestamp,
            context: self.context,
        })
    }
}

fn limit_to_i64(limit: usize) -> i64 {
    i64::try_from(limit).unwrap_or(i64::MAX)
}

fn validate_schema(connection: &Connection) -> Result<()> {
    let mut statement = connection
        .prepare("PRAGMA table_info(observations)")
        .context("failed to inspect observations schema")?;
    let columns = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })
        .context("failed to query observations schema")?
        .collect::<std::result::Result<BTreeMap<_, _>, _>>()
        .context("failed to decode observations schema")?;

    for (name, expected_type) in REQUIRED_COLUMNS {
        match columns.get(name) {
            None => bail!("incompatible observations schema: missing column {name}"),
            Some(actual_type) if !actual_type.eq_ignore_ascii_case(expected_type) => bail!(
                "incompatible observations schema: column {name} has type {actual_type}, expected {expected_type}"
            ),
            Some(_) => {}
        }
    }
    Ok(())
}
