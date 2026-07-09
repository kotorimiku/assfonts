use std::{fs, path::Path};

use rusqlite::{Connection, params};

use crate::error::Result;

const CURRENT_SCHEMA_VERSION: i32 = 1;

pub(crate) fn open_connection(db_file: &Path) -> std::result::Result<Connection, rusqlite::Error> {
    let conn = Connection::open(db_file)?;
    conn.execute("PRAGMA foreign_keys = ON;", [])?;
    Ok(conn)
}

pub fn open_and_prepare_db(dbpath: &Path) -> Result<Connection> {
    let db_file = dbpath.join("fonts.db");

    if db_file.exists() {
        match check_schema_version(&db_file) {
            Ok(true) => {
                // Version matches! Connect.
                return Ok(open_connection(&db_file)?);
            }
            _ => {
                // Outdated, corrupt, or table missing. Rebuild.
                let _ = fs::remove_file(&db_file);
            }
        }
    }

    // Connect and build tables
    let mut conn = open_connection(&db_file)?;
    create_tables(&mut conn)?;
    Ok(conn)
}

fn check_schema_version(db_file: &Path) -> std::result::Result<bool, rusqlite::Error> {
    let conn = open_connection(db_file)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_foreign_key_cascade() -> std::result::Result<(), Box<dyn std::error::Error>> {
        // 1. Test when foreign_keys is disabled (explicitly set to OFF), deleting a font won't cascade delete its aliases
        let mut conn = Connection::open_in_memory()?;
        conn.execute("PRAGMA foreign_keys = OFF;", [])?;
        create_tables(&mut conn)?;

        // Insert font
        conn.execute(
            "INSERT INTO fonts (path, face_index, display_name, normalized_name, inferred_weight, is_italic, mtime, file_size)
             VALUES ('/path/to/font.ttf', 0, 'Test Font', 'testfont', 400, 0, 100, 100)",
            [],
        )?;

        // Insert alias
        conn.execute(
            "INSERT INTO aliases (path, face_index, alias, normalized_alias)
             VALUES ('/path/to/font.ttf', 0, 'Test Font Alias', 'testfontalias')",
            [],
        )?;

        // Delete font
        conn.execute("DELETE FROM fonts WHERE path = '/path/to/font.ttf'", [])?;

        // Check if the alias still exists (it should, because foreign_keys = OFF)
        let count: i32 = conn.query_row(
            "SELECT count(*) FROM aliases WHERE path = '/path/to/font.ttf'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(count, 1, "Without foreign_keys = ON, cascade delete should not work");

        // 2. Test when foreign_keys is enabled, cascade delete works as expected
        let mut conn = Connection::open_in_memory()?;
        conn.execute("PRAGMA foreign_keys = ON;", [])?;
        create_tables(&mut conn)?;

        // Insert font
        conn.execute(
            "INSERT INTO fonts (path, face_index, display_name, normalized_name, inferred_weight, is_italic, mtime, file_size)
             VALUES ('/path/to/font.ttf', 0, 'Test Font', 'testfont', 400, 0, 100, 100)",
            [],
        )?;

        // Insert alias
        conn.execute(
            "INSERT INTO aliases (path, face_index, alias, normalized_alias)
             VALUES ('/path/to/font.ttf', 0, 'Test Font Alias', 'testfontalias')",
            [],
        )?;

        // Delete font
        conn.execute("DELETE FROM fonts WHERE path = '/path/to/font.ttf'", [])?;

        // Check if the alias still exists (it should be deleted by cascade)
        let count: i32 = conn.query_row(
            "SELECT count(*) FROM aliases WHERE path = '/path/to/font.ttf'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(count, 0, "With foreign_keys = ON, cascade delete should work");

        Ok(())
    }
}

