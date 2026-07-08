use std::{fs, path::Path};

use rusqlite::{Connection, params};

use crate::error::Result;

const CURRENT_SCHEMA_VERSION: i32 = 1;

pub fn open_and_prepare_db(dbpath: &Path) -> Result<Connection> {
    let db_file = dbpath.join("fonts.db");

    if db_file.exists() {
        match check_schema_version(&db_file) {
            Ok(true) => {
                // Version matches! Connect.
                return Ok(Connection::open(&db_file)?);
            }
            _ => {
                // Outdated, corrupt, or table missing. Rebuild.
                let _ = fs::remove_file(&db_file);
            }
        }
    }

    // Connect and build tables
    let mut conn = Connection::open(&db_file)?;
    create_tables(&mut conn)?;
    Ok(conn)
}

fn check_schema_version(db_file: &Path) -> std::result::Result<bool, rusqlite::Error> {
    let conn = Connection::open(db_file)?;

    // Check if meta table exists and version matches
    let table_exists: i32 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='meta'",
        [],
        |row| row.get(0),
    )?;

    if table_exists == 0 {
        return Ok(false);
    }

    let version_str: String = conn.query_row(
        "SELECT value FROM meta WHERE key = 'schema_version'",
        [],
        |row| row.get(0),
    )?;

    if let Ok(ver) = version_str.parse::<i32>() {
        Ok(ver == CURRENT_SCHEMA_VERSION)
    } else {
        Ok(false)
    }
}

fn create_tables(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    tx.execute(
        "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
        [],
    )?;

    tx.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1);",
        params![CURRENT_SCHEMA_VERSION.to_string()],
    )?;

    tx.execute(
        "CREATE TABLE IF NOT EXISTS fonts (
            path TEXT NOT NULL,
            face_index INTEGER NOT NULL,
            display_name TEXT NOT NULL,
            normalized_name TEXT NOT NULL,
            inferred_weight INTEGER NOT NULL,
            is_italic INTEGER NOT NULL,
            mtime INTEGER NOT NULL,
            file_size INTEGER NOT NULL,
            PRIMARY KEY (path, face_index)
        );",
        [],
    )?;

    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_fonts_normalized_name ON fonts(normalized_name);",
        [],
    )?;

    tx.execute(
        "CREATE TABLE IF NOT EXISTS aliases (
            path TEXT NOT NULL,
            face_index INTEGER NOT NULL,
            alias TEXT NOT NULL,
            normalized_alias TEXT NOT NULL,
            PRIMARY KEY (path, face_index, alias),
            FOREIGN KEY (path, face_index) REFERENCES fonts(path, face_index) ON DELETE CASCADE
        );",
        [],
    )?;

    tx.execute(
        "CREATE INDEX IF NOT EXISTS idx_aliases_normalized ON aliases(normalized_alias);",
        [],
    )?;

    tx.commit()?;
    Ok(())
}
