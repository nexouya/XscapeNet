use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{Duration, Local, TimeZone, Timelike};
use parking_lot::Mutex;
use rusqlite::{params, Connection};

use crate::models::{
    AnalyticsResponse, AppSettings, CollectorMode, EndpointUsageRow, ProcessTrafficDelta, ProcessUsageRow,
    TimeRange, TimeSeriesPoint,
};

pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    pub fn open() -> Result<Self, rusqlite::Error> {
        let base_dir: PathBuf = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("XscapeNet");
        let _ = std::fs::create_dir_all(&base_dir);
        let db_path = base_dir.join("xscapenet.db");
        Self::open_path(db_path)
    }

    pub fn open_path(db_path: PathBuf) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = -8000;

            CREATE TABLE IF NOT EXISTS traffic_minutely (
                bucket_ts     INTEGER NOT NULL,
                interface_id  TEXT    NOT NULL,
                rx_bytes      INTEGER NOT NULL DEFAULT 0,
                tx_bytes      INTEGER NOT NULL DEFAULT 0,
                peak_bps      INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (bucket_ts, interface_id)
            );

            CREATE TABLE IF NOT EXISTS traffic_hourly (
                bucket_ts     INTEGER NOT NULL,
                interface_id  TEXT    NOT NULL,
                rx_bytes      INTEGER NOT NULL DEFAULT 0,
                tx_bytes      INTEGER NOT NULL DEFAULT 0,
                peak_bps      INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (bucket_ts, interface_id)
            );

            CREATE TABLE IF NOT EXISTS traffic_daily (
                day_key       TEXT    NOT NULL,
                interface_id  TEXT    NOT NULL,
                rx_bytes      INTEGER NOT NULL DEFAULT 0,
                tx_bytes      INTEGER NOT NULL DEFAULT 0,
                peak_bps      INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (day_key, interface_id)
            );

            CREATE TABLE IF NOT EXISTS process_endpoint_hourly (
                bucket_ts     INTEGER NOT NULL,
                interface_id  TEXT    NOT NULL,
                process_name  TEXT    NOT NULL,
                process_path  TEXT    NOT NULL DEFAULT '',
                remote_host   TEXT    NOT NULL,
                protocol      TEXT    NOT NULL,
                rx_bytes      INTEGER NOT NULL DEFAULT 0,
                tx_bytes      INTEGER NOT NULL DEFAULT 0,
                connections   INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (bucket_ts, interface_id, process_name, remote_host, protocol)
            );

            CREATE INDEX IF NOT EXISTS idx_minutely_ts ON traffic_minutely (bucket_ts);
            CREATE INDEX IF NOT EXISTS idx_hourly_ts ON traffic_hourly (bucket_ts);
            CREATE TABLE IF NOT EXISTS blocked_apps (
                process_name TEXT PRIMARY KEY,
                process_path TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_proc_ts ON process_endpoint_hourly (bucket_ts);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Records a 1-second interface delta into minutely, hourly, and daily rollup tables.
    pub fn record_interface_deltas(
        &self,
        now_ts: i64,
        deltas: &[(String, u64, u64)], // (interface_id, rx_delta, tx_delta)
    ) -> Result<(), rusqlite::Error> {
        if deltas.is_empty() {
            return Ok(());
        }
        let minute_bucket = now_ts - (now_ts % 60);
        let hour_bucket = now_ts - (now_ts % 3600);
        let day_key = Local
            .timestamp_opt(now_ts, 0)
            .single()
            .unwrap_or_else(Local::now)
            .format("%Y-%m-%d")
            .to_string();

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        {
            let mut stmt_min = tx.prepare_cached(
                "INSERT INTO traffic_minutely (bucket_ts, interface_id, rx_bytes, tx_bytes, peak_bps)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(bucket_ts, interface_id) DO UPDATE SET
                   rx_bytes = rx_bytes + excluded.rx_bytes,
                   tx_bytes = tx_bytes + excluded.tx_bytes,
                   peak_bps = MAX(peak_bps, excluded.peak_bps)",
            )?;

            let mut stmt_hr = tx.prepare_cached(
                "INSERT INTO traffic_hourly (bucket_ts, interface_id, rx_bytes, tx_bytes, peak_bps)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(bucket_ts, interface_id) DO UPDATE SET
                   rx_bytes = rx_bytes + excluded.rx_bytes,
                   tx_bytes = tx_bytes + excluded.tx_bytes,
                   peak_bps = MAX(peak_bps, excluded.peak_bps)",
            )?;

            let mut stmt_day = tx.prepare_cached(
                "INSERT INTO traffic_daily (day_key, interface_id, rx_bytes, tx_bytes, peak_bps)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(day_key, interface_id) DO UPDATE SET
                   rx_bytes = rx_bytes + excluded.rx_bytes,
                   tx_bytes = tx_bytes + excluded.tx_bytes,
                   peak_bps = MAX(peak_bps, excluded.peak_bps)",
            )?;

            for (iface_id, rx, tx_b) in deltas {
                if *rx == 0 && *tx_b == 0 {
                    continue;
                }
                let rate_bps = rx.saturating_add(*tx_b);
                stmt_min.execute(params![minute_bucket, iface_id, *rx as i64, *tx_b as i64, rate_bps as i64])?;
                stmt_hr.execute(params![hour_bucket, iface_id, *rx as i64, *tx_b as i64, rate_bps as i64])?;
                stmt_day.execute(params![&day_key, iface_id, *rx as i64, *tx_b as i64, rate_bps as i64])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn record_process_deltas(
        &self,
        now_ts: i64,
        interface_id: &str,
        deltas: &[ProcessTrafficDelta],
    ) -> Result<(), rusqlite::Error> {
        if deltas.is_empty() {
            return Ok(());
        }
        let hour_bucket = now_ts - (now_ts % 3600);
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO process_endpoint_hourly
                 (bucket_ts, interface_id, process_name, process_path, remote_host, protocol, rx_bytes, tx_bytes, connections)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(bucket_ts, interface_id, process_name, remote_host, protocol) DO UPDATE SET
                   process_path = CASE WHEN excluded.process_path != '' THEN excluded.process_path ELSE process_path END,
                   rx_bytes = rx_bytes + excluded.rx_bytes,
                   tx_bytes = tx_bytes + excluded.tx_bytes,
                   connections = MAX(connections, excluded.connections)",
            )?;

            for d in deltas {
                if d.rx_bytes == 0 && d.tx_bytes == 0 && d.connections == 0 {
                    continue;
                }
                stmt.execute(params![
                    hour_bucket,
                    interface_id,
                    &d.process_name,
                    &d.process_path,
                    &d.remote_host,
                    &d.protocol,
                    d.rx_bytes as i64,
                    d.tx_bytes as i64,
                    d.connections as i64
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn prune_old_buckets(&self, now_ts: i64) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        let min_cutoff = now_ts - 48 * 3600;
        let hour_cutoff = now_ts - 90 * 86400;
        conn.execute("DELETE FROM traffic_minutely WHERE bucket_ts < ?1", params![min_cutoff])?;
        conn.execute("DELETE FROM traffic_hourly WHERE bucket_ts < ?1", params![hour_cutoff])?;
        conn.execute("DELETE FROM process_endpoint_hourly WHERE bucket_ts < ?1", params![hour_cutoff])?;
        Ok(())
    }

    pub fn get_today_interface_totals(&self) -> Result<HashMap<String, (u64, u64)>, rusqlite::Error> {
        let day_key = Local::now().format("%Y-%m-%d").to_string();
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT interface_id, rx_bytes, tx_bytes FROM traffic_daily WHERE day_key = ?1",
        )?;
        let rows = stmt.query_map(params![day_key], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?.max(0) as u64,
                row.get::<_, i64>(2)?.max(0) as u64,
            ))
        })?;
        let mut out = HashMap::new();
        for r in rows.flatten() {
            out.insert(r.0, (r.1, r.2));
        }
        Ok(out)
    }

    pub fn query_analytics(
        &self,
        range: TimeRange,
        interface_id: &str,
        collector_mode: CollectorMode,
    ) -> Result<AnalyticsResponse, rusqlite::Error> {
        let now = Local::now();
        let now_ts = now.timestamp();
        let today_start = now
            .with_hour(0)
            .and_then(|d| d.with_minute(0))
            .and_then(|d| d.with_second(0))
            .map(|d| d.timestamp())
            .unwrap_or(now_ts - 86400);

        let (start_ts, series) = match range {
            TimeRange::Hourly => {
                let start = now_ts - 3600;
                (start, self.query_minutely_series(start, now_ts, interface_id, 60)?)
            }
            TimeRange::Today => {
                let start = today_start;
                (start, self.query_hourly_series(start, now_ts, interface_id, "%H:00")?)
            }
            TimeRange::TwoDays => {
                let start = (now - Duration::hours(48)).timestamp();
                (start, self.query_hourly_series(start, now_ts, interface_id, "%a %H:00")?)
            }
            TimeRange::Weekly => {
                let start = (now - Duration::days(7)).timestamp();
                (start, self.query_daily_series(7, interface_id)?)
            }
            TimeRange::Monthly => {
                let start = (now - Duration::days(30)).timestamp();
                (start, self.query_daily_series(30, interface_id)?)
            }
        };

        let mut total_rx: u64 = 0;
        let mut total_tx: u64 = 0;
        let mut peak_bps: u64 = 0;
        for pt in &series {
            total_rx = total_rx.saturating_add(pt.rx_bytes);
            total_tx = total_tx.saturating_add(pt.tx_bytes);
            peak_bps = peak_bps.max(pt.peak_bps);
        }

        let processes = self.query_process_breakdown(start_ts, now_ts, interface_id, total_rx + total_tx)?;

        Ok(AnalyticsResponse {
            range,
            interface_id: interface_id.to_string(),
            collector_mode,
            total_rx_bytes: total_rx,
            total_tx_bytes: total_tx,
            total_bytes: total_rx.saturating_add(total_tx),
            peak_bps,
            series,
            processes,
        })
    }

    fn query_minutely_series(
        &self,
        start_ts: i64,
        end_ts: i64,
        interface_id: &str,
        step_secs: i64,
    ) -> Result<Vec<TimeSeriesPoint>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut map: HashMap<i64, (u64, u64, u64)> = HashMap::new();

        if interface_id == "all" {
            let mut stmt = conn.prepare(
                "SELECT bucket_ts, SUM(rx_bytes), SUM(tx_bytes), MAX(peak_bps)
                 FROM traffic_minutely
                 WHERE bucket_ts >= ?1 AND bucket_ts <= ?2
                 GROUP BY bucket_ts ORDER BY bucket_ts ASC",
            )?;
            let rows = stmt.query_map(params![start_ts, end_ts], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?.max(0) as u64,
                    r.get::<_, i64>(2)?.max(0) as u64,
                    r.get::<_, i64>(3)?.max(0) as u64,
                ))
            })?;
            for row in rows.flatten() {
                map.insert(row.0, (row.1, row.2, row.3));
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT bucket_ts, rx_bytes, tx_bytes, peak_bps
                 FROM traffic_minutely
                 WHERE bucket_ts >= ?1 AND bucket_ts <= ?2 AND interface_id = ?3
                 ORDER BY bucket_ts ASC",
            )?;
            let rows = stmt.query_map(params![start_ts, end_ts, interface_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?.max(0) as u64,
                    r.get::<_, i64>(2)?.max(0) as u64,
                    r.get::<_, i64>(3)?.max(0) as u64,
                ))
            })?;
            for row in rows.flatten() {
                map.insert(row.0, (row.1, row.2, row.3));
            }
        }

        let mut points = Vec::new();
        let mut ts = start_ts - (start_ts % step_secs);
        let end_aligned = end_ts - (end_ts % step_secs);
        while ts <= end_aligned {
            let (rx, tx, peak) = map.get(&ts).copied().unwrap_or((0, 0, 0));
            let label = Local
                .timestamp_opt(ts, 0)
                .single()
                .map(|dt| dt.format("%H:%M").to_string())
                .unwrap_or_default();
            points.push(TimeSeriesPoint {
                timestamp: ts,
                label,
                rx_bytes: rx,
                tx_bytes: tx,
                peak_bps: peak,
            });
            ts += step_secs;
        }
        Ok(points)
    }

    fn query_hourly_series(
        &self,
        start_ts: i64,
        end_ts: i64,
        interface_id: &str,
        fmt: &str,
    ) -> Result<Vec<TimeSeriesPoint>, rusqlite::Error> {
        let conn = self.conn.lock();
        let mut map: HashMap<i64, (u64, u64, u64)> = HashMap::new();

        if interface_id == "all" {
            let mut stmt = conn.prepare(
                "SELECT bucket_ts, SUM(rx_bytes), SUM(tx_bytes), MAX(peak_bps)
                 FROM traffic_hourly
                 WHERE bucket_ts >= ?1 AND bucket_ts <= ?2
                 GROUP BY bucket_ts ORDER BY bucket_ts ASC",
            )?;
            let rows = stmt.query_map(params![start_ts - 3600, end_ts], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?.max(0) as u64,
                    r.get::<_, i64>(2)?.max(0) as u64,
                    r.get::<_, i64>(3)?.max(0) as u64,
                ))
            })?;
            for row in rows.flatten() {
                map.insert(row.0, (row.1, row.2, row.3));
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT bucket_ts, rx_bytes, tx_bytes, peak_bps
                 FROM traffic_hourly
                 WHERE bucket_ts >= ?1 AND bucket_ts <= ?2 AND interface_id = ?3
                 ORDER BY bucket_ts ASC",
            )?;
            let rows = stmt.query_map(params![start_ts - 3600, end_ts, interface_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?.max(0) as u64,
                    r.get::<_, i64>(2)?.max(0) as u64,
                    r.get::<_, i64>(3)?.max(0) as u64,
                ))
            })?;
            for row in rows.flatten() {
                map.insert(row.0, (row.1, row.2, row.3));
            }
        }

        let mut points = Vec::new();
        let mut ts = start_ts - (start_ts % 3600);
        let end_aligned = end_ts - (end_ts % 3600);
        while ts <= end_aligned {
            let (rx, tx, peak) = map.get(&ts).copied().unwrap_or((0, 0, 0));
            let label = Local
                .timestamp_opt(ts, 0)
                .single()
                .map(|dt| dt.format(fmt).to_string())
                .unwrap_or_default();
            points.push(TimeSeriesPoint {
                timestamp: ts,
                label,
                rx_bytes: rx,
                tx_bytes: tx,
                peak_bps: peak,
            });
            ts += 3600;
        }
        Ok(points)
    }

    fn query_daily_series(
        &self,
        days: i64,
        interface_id: &str,
    ) -> Result<Vec<TimeSeriesPoint>, rusqlite::Error> {
        let now = Local::now();
        let start_day = (now - Duration::days(days - 1)).format("%Y-%m-%d").to_string();
        let conn = self.conn.lock();
        let mut map: HashMap<String, (u64, u64, u64)> = HashMap::new();

        if interface_id == "all" {
            let mut stmt = conn.prepare(
                "SELECT day_key, SUM(rx_bytes), SUM(tx_bytes), MAX(peak_bps)
                 FROM traffic_daily
                 WHERE day_key >= ?1
                 GROUP BY day_key ORDER BY day_key ASC",
            )?;
            let rows = stmt.query_map(params![&start_day], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?.max(0) as u64,
                    r.get::<_, i64>(2)?.max(0) as u64,
                    r.get::<_, i64>(3)?.max(0) as u64,
                ))
            })?;
            for row in rows.flatten() {
                map.insert(row.0, (row.1, row.2, row.3));
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT day_key, rx_bytes, tx_bytes, peak_bps
                 FROM traffic_daily
                 WHERE day_key >= ?1 AND interface_id = ?2
                 ORDER BY day_key ASC",
            )?;
            let rows = stmt.query_map(params![&start_day, interface_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?.max(0) as u64,
                    r.get::<_, i64>(2)?.max(0) as u64,
                    r.get::<_, i64>(3)?.max(0) as u64,
                ))
            })?;
            for row in rows.flatten() {
                map.insert(row.0, (row.1, row.2, row.3));
            }
        }

        let mut points = Vec::new();
        for offset in (0..days).rev() {
            let dt = now - Duration::days(offset);
            let key = dt.format("%Y-%m-%d").to_string();
            let label = if days <= 7 {
                dt.format("%a %d").to_string()
            } else {
                dt.format("%b %d").to_string()
            };
            let (rx, tx, peak) = map.get(&key).copied().unwrap_or((0, 0, 0));
            points.push(TimeSeriesPoint {
                timestamp: dt.timestamp(),
                label,
                rx_bytes: rx,
                tx_bytes: tx,
                peak_bps: peak,
            });
        }
        Ok(points)
    }

    pub fn query_process_breakdown(
        &self,
        start_ts: i64,
        end_ts: i64,
        interface_id: &str,
        reference_total: u64,
    ) -> Result<Vec<ProcessUsageRow>, rusqlite::Error> {
        let conn = self.conn.lock();
        let start_hr = start_ts - (start_ts % 3600);

        struct RawRow {
            process_name: String,
            process_path: String,
            remote_host: String,
            protocol: String,
            rx: u64,
            tx: u64,
            conns: u32,
        }

        let mut raw_rows = Vec::new();
        if interface_id == "all" {
            let mut stmt = conn.prepare(
                "SELECT process_name, MAX(process_path), remote_host, protocol,
                        SUM(rx_bytes), SUM(tx_bytes), MAX(connections)
                 FROM process_endpoint_hourly
                 WHERE bucket_ts >= ?1 AND bucket_ts <= ?2
                 GROUP BY process_name, remote_host, protocol",
            )?;
            let iter = stmt.query_map(params![start_hr, end_ts], |r| {
                Ok(RawRow {
                    process_name: r.get(0)?,
                    process_path: r.get(1)?,
                    remote_host: r.get(2)?,
                    protocol: r.get(3)?,
                    rx: r.get::<_, i64>(4)?.max(0) as u64,
                    tx: r.get::<_, i64>(5)?.max(0) as u64,
                    conns: r.get::<_, i64>(6)?.max(0) as u32,
                })
            })?;
            for r in iter.flatten() {
                raw_rows.push(r);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT process_name, MAX(process_path), remote_host, protocol,
                        SUM(rx_bytes), SUM(tx_bytes), MAX(connections)
                 FROM process_endpoint_hourly
                 WHERE bucket_ts >= ?1 AND bucket_ts <= ?2 AND interface_id = ?3
                 GROUP BY process_name, remote_host, protocol",
            )?;
            let iter = stmt.query_map(params![start_hr, end_ts, interface_id], |r| {
                Ok(RawRow {
                    process_name: r.get(0)?,
                    process_path: r.get(1)?,
                    remote_host: r.get(2)?,
                    protocol: r.get(3)?,
                    rx: r.get::<_, i64>(4)?.max(0) as u64,
                    tx: r.get::<_, i64>(5)?.max(0) as u64,
                    conns: r.get::<_, i64>(6)?.max(0) as u32,
                })
            })?;
            for r in iter.flatten() {
                raw_rows.push(r);
            }
        }

        let mut grouped: HashMap<String, ProcessUsageRow> = HashMap::new();
        for r in raw_rows {
            let entry = grouped.entry(r.process_name.clone()).or_insert_with(|| ProcessUsageRow {
                process_name: r.process_name.clone(),
                process_path: r.process_path.clone(),
                rx_bytes: 0,
                tx_bytes: 0,
                total_bytes: 0,
                share_percent: 0.0,
                active_connections: 0,
                endpoints: Vec::new(),
            });
            if entry.process_path.is_empty() && !r.process_path.is_empty() {
                entry.process_path = r.process_path;
            }
            entry.rx_bytes = entry.rx_bytes.saturating_add(r.rx);
            entry.tx_bytes = entry.tx_bytes.saturating_add(r.tx);
            entry.total_bytes = entry.rx_bytes.saturating_add(entry.tx_bytes);
            entry.active_connections = entry.active_connections.saturating_add(r.conns);
            entry.endpoints.push(EndpointUsageRow {
                remote_host: r.remote_host,
                protocol: r.protocol,
                rx_bytes: r.rx,
                tx_bytes: r.tx,
                total_bytes: r.rx.saturating_add(r.tx),
            });
        }

        let sum_procs: u64 = grouped.values().map(|p| p.total_bytes).sum();
        let denom = reference_total.max(sum_procs).max(1) as f64;

        let mut list: Vec<ProcessUsageRow> = grouped
            .into_values()
            .map(|mut p| {
                p.share_percent = ((p.total_bytes as f64 / denom) * 100.0).min(100.0);
                p.endpoints.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes));
                p.endpoints.truncate(12);
                p
            })
            .collect();

        list.sort_by(|a, b| {
            b.total_bytes
                .cmp(&a.total_bytes)
                .then_with(|| b.active_connections.cmp(&a.active_connections))
        });
        list.truncate(40);
        Ok(list)
    }

    pub fn clear_all_history(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "DELETE FROM traffic_minutely;
             DELETE FROM traffic_hourly;
             DELETE FROM traffic_daily;
             DELETE FROM process_endpoint_hourly;
             VACUUM;",
        )
    }

    pub fn load_settings(&self) -> AppSettings {
        let conn = self.conn.lock();
        let mut settings = AppSettings::default();
        if let Ok(mut stmt) = conn.prepare("SELECT key, value FROM app_settings") {
            if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))) {
                for row in rows.flatten() {
                    match row.0.as_str() {
                        "theme" => settings.theme = row.1,
                        "hide_ip" => settings.hide_ip = row.1 == "true",
                        "glass_intensity" => settings.glass_intensity = row.1,
                        _ => {}
                    }
                }
            }
        }
        settings
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), rusqlite::Error> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )?;
            stmt.execute(params!["theme", &settings.theme])?;
            stmt.execute(params!["hide_ip", if settings.hide_ip { "true" } else { "false" }])?;
            stmt.execute(params!["glass_intensity", &settings.glass_intensity])?;
        }
        tx.commit()?;
        Ok(())
    }


    pub fn list_blocked_apps(&self) -> Vec<String> {
        let conn = self.conn.lock();
        let mut out = Vec::new();
        if let Ok(mut stmt) = conn.prepare("SELECT process_name FROM blocked_apps") {
            if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
                for r in rows.flatten() {
                    out.push(r);
                }
            }
        }
        out
    }

    pub fn set_blocked_app_record(
        &self,
        process_name: &str,
        process_path: &str,
        blocked: bool,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock();
        if blocked {
            conn.execute(
                "INSERT INTO blocked_apps (process_name, process_path) VALUES (?1, ?2)
                 ON CONFLICT(process_name) DO UPDATE SET process_path = excluded.process_path",
                params![process_name, process_path],
            )?;
        } else {
            conn.execute(
                "DELETE FROM blocked_apps WHERE LOWER(process_name) = LOWER(?1)",
                params![process_name],
            )?;
        }
        Ok(())
    }

    /// Flushes WAL journal cleanly into the main SQLite database file on exit.
    pub fn checkpoint_wal(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollup_and_query_roundtrip() {
        let dir = std::env::temp_dir().join("xscapenet_test_db");
        let _ = std::fs::create_dir_all(&dir);
        let db = dir.join("test.db");
        let _ = std::fs::remove_file(&db);

        let storage = Storage::open_path(db.clone()).expect("open db");
        let now = Local::now().timestamp();
        storage
            .record_interface_deltas(now, &[("wifi0".into(), 1_500_000, 250_000)])
            .expect("record deltas");
        storage
            .record_process_deltas(
                now,
                "wifi0",
                &[ProcessTrafficDelta {
                    process_name: "chrome.exe".into(),
                    process_path: "C:\\Program Files\\Google\\Chrome\\chrome.exe".into(),
                    remote_host: "youtube.com".into(),
                    protocol: "QUIC".into(),
                    rx_bytes: 1_200_000,
                    tx_bytes: 120_000,
                    connections: 4,
                }],
            )
            .expect("record proc");

        let today = storage
            .query_analytics(TimeRange::Today, "all", CollectorMode::KernelEtw)
            .expect("query today");
        assert_eq!(today.total_rx_bytes, 1_500_000);
        assert_eq!(today.total_tx_bytes, 250_000);
        assert_eq!(today.processes.len(), 1);
        assert_eq!(today.processes[0].process_name, "chrome.exe");
        assert_eq!(today.processes[0].endpoints[0].remote_host, "youtube.com");

        let _ = std::fs::remove_file(&db);
    }
}
