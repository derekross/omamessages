// SPDX-License-Identifier: GPL-3.0-or-later
//! mmsd-tng's carrier settings: the MMSC, the MMS proxy and the MMS APN.
//!
//! mmsd-tng keeps them in `$HOME/.mms/modemmanager/mms`, a GLib key file,
//! group `[Modem Manager]`, keys `CarrierMMSC`, `CarrierMMSProxy` and
//! `MMS_APN` (its README and `plugins/modemmanager.c`). Two values mean
//! "none": `http://mms.invalid` for the MMSC (mmsd-tng then looks the
//! carrier up in mobile-broadband-provider-info) and `NULL` for the proxy
//! (and for an empty APN). Here they are shown and taken as empty fields.
//!
//! mmsd-tng reads the file when it starts and writes its own copy back
//! when it stops, so the file is only edited while it isn't running.
//! While it runs, its `org.ofono.mms.ModemManager` interface
//! (`ViewSettings`, `ChangeSettings`) reads and changes the same settings
//! at once and saves the file itself.
//!
//! The file is edited in place: only the three values change; comments,
//! other groups and keys and their order stay as they were.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use zbus::zvariant::OwnedValue;

pub const GROUP: &str = "Modem Manager";
pub const MMSC: &str = "CarrierMMSC";
pub const PROXY: &str = "CarrierMMSProxy";
pub const APN: &str = "MMS_APN";
/// mmsd-tng's "look the MMSC up" placeholder.
const NO_MMSC: &str = "http://mms.invalid";
const NULL: &str = "NULL";

/// `$HOME/.mms/modemmanager/mms`, where mmsd-tng's ModemManager plugin
/// keeps its settings (GLib's home directory is `$HOME` when set).
pub fn path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    Path::new(&home).join(".mms/modemmanager/mms")
}

/// The three settings as the form shows them: empty means none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MmsSettings {
    pub mmsc: String,
    pub proxy: String,
    pub apn: String,
}

impl MmsSettings {
    /// From mmsd-tng's stored values (file or ViewSettings).
    pub fn from_values(mmsc: &str, proxy: &str, apn: &str) -> MmsSettings {
        let shown = |v: &str, none: &str| {
            let v = v.trim();
            if v.eq_ignore_ascii_case(none) || v.eq_ignore_ascii_case(NULL) {
                String::new()
            } else {
                v.to_string()
            }
        };
        MmsSettings {
            mmsc: shown(mmsc, NO_MMSC),
            proxy: shown(proxy, NULL),
            apn: shown(apn, NULL),
        }
    }

    /// The value mmsd-tng stores for one of the keys.
    pub fn stored(key: &str, shown: &str) -> String {
        let v = shown.trim();
        match (key, v.is_empty()) {
            (MMSC, true) => NO_MMSC.into(),
            (_, true) => NULL.into(),
            _ => v.to_string(),
        }
    }

    pub fn read_file(path: &Path) -> MmsSettings {
        let file = KeyFile::parse(&std::fs::read_to_string(path).unwrap_or_default());
        let get = |k: &str| file.get(GROUP, k).unwrap_or_default();
        MmsSettings::from_values(&get(MMSC), &get(PROXY), &get(APN))
    }

    /// From ViewSettings' answer.
    pub fn from_dbus(values: &HashMap<String, OwnedValue>) -> MmsSettings {
        let get = |k: &str| {
            values
                .get(k)
                .and_then(|v| <&str>::try_from(v).ok())
                .unwrap_or_default()
                .to_string()
        };
        MmsSettings::from_values(&get(MMSC), &get(PROXY), &get(APN))
    }
}

/// Why a value can't be saved, for the form.
pub fn check(key: &str, shown: &str) -> Result<(), &'static str> {
    let v = shown.trim();
    if v.chars().any(char::is_whitespace) {
        return Err("No spaces, please");
    }
    if key == MMSC && !v.is_empty() && !(v.starts_with("http://") || v.starts_with("https://")) {
        return Err("The MMSC is a web address: http://…");
    }
    if key == PROXY && v.contains("://") {
        return Err("The proxy is a host and an optional port: host:port");
    }
    Ok(())
}

/// Set one key in the file, creating the file (mode 0600, its directory
/// 0700) when there is none. Written to a temporary file and renamed.
pub fn write_file(path: &Path, key: &str, shown: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut file = KeyFile::parse(&text);
    file.set(GROUP, key, &MmsSettings::stored(key, shown));
    if let Some(dir) = path.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    let tmp = path.with_extension("omamessages-tmp");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    f.write_all(file.text().as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)
}

/// mmsd-tng's user unit, for the restart fallback.
pub const UNIT: &str = "mmsd-tng.service";

#[zbus::proxy(
    interface = "org.ofono.mms.ModemManager",
    default_service = "org.ofono.mms",
    default_path = "/org/ofono/mms"
)]
trait MmsdModemManager {
    fn view_settings(&self) -> zbus::Result<HashMap<String, OwnedValue>>;
    fn change_settings(&self, setting: &str, value: &zbus::zvariant::Value<'_>)
        -> zbus::Result<()>;
    fn process_message_queue(&self) -> zbus::Result<()>;
}

/// Whether mmsd-tng runs on this session bus.
pub async fn running(conn: &zbus::Connection) -> bool {
    let Ok(dbus) = zbus::fdo::DBusProxy::new(conn).await else {
        return false;
    };
    let Ok(name) = zbus::names::BusName::try_from("org.ofono.mms") else {
        return false;
    };
    dbus.name_has_owner(name).await.unwrap_or(false)
}

/// The settings, and whether mmsd-tng runs: from mmsd-tng when it does
/// (what it uses now), else from the file.
pub async fn load(conn: Option<&zbus::Connection>) -> (MmsSettings, bool) {
    if let Some(c) = conn {
        if running(c).await {
            if let Ok(p) = MmsdModemManagerProxy::new(c).await {
                if let Ok(v) = p.view_settings().await {
                    return (MmsSettings::from_dbus(&v), true);
                }
            }
            return (MmsSettings::read_file(&path()), true);
        }
    }
    (MmsSettings::read_file(&path()), false)
}

/// How a saved setting took effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saved {
    /// mmsd-tng runs and took it at once (and saved its file).
    Now,
    /// Written to the file; mmsd-tng reads it when it starts.
    AtStart,
    /// mmsd-tng couldn't take it while running: stopped, the file
    /// written, started again.
    Restarted,
}

/// The value ChangeSettings takes: an empty proxy or APN is "" there
/// (mmsd-tng stores it as NULL itself), an empty MMSC its placeholder.
fn dbus_value(key: &str, shown: &str) -> String {
    if key == MMSC {
        MmsSettings::stored(key, shown)
    } else {
        shown.trim().to_string()
    }
}

/// Save one setting where mmsd-tng will use it.
pub async fn save(
    conn: Option<&zbus::Connection>,
    key: &str,
    shown: &str,
) -> Result<Saved, String> {
    check(key, shown).map_err(str::to_string)?;
    if let Some(c) = conn {
        if running(c).await {
            match change(c, key, &dbus_value(key, shown)).await {
                Ok(()) => return Ok(Saved::Now),
                Err(e) => {
                    tracing::warn!(error = %e, "mmsd-tng didn't take the setting; restarting it");
                    return restart_with(key, shown).await;
                }
            }
        }
    }
    write_file(&path(), key, shown).map_err(|e| e.to_string())?;
    Ok(Saved::AtStart)
}

async fn change(conn: &zbus::Connection, key: &str, value: &str) -> zbus::Result<()> {
    let p = MmsdModemManagerProxy::new(conn).await?;
    p.change_settings(key, &zbus::zvariant::Value::from(value))
        .await?;
    // Messages queued with the old settings go again ("Modem is not
    // connected" when there is no data: nothing to do then).
    let _ = p.process_message_queue().await;
    Ok(())
}

/// mmsd-tng writes its settings back when it stops, so the file is
/// edited between a stop and a start.
async fn restart_with(key: &str, shown: &str) -> Result<Saved, String> {
    let (key, shown) = (key.to_string(), shown.to_string());
    gtk4::gio::spawn_blocking(move || {
        systemctl("stop")?;
        let written = write_file(&path(), &key, &shown).map_err(|e| e.to_string());
        systemctl("start")?;
        written.map(|()| Saved::Restarted)
    })
    .await
    .map_err(|_| "the restart was interrupted".to_string())?
}

fn systemctl(verb: &str) -> Result<(), String> {
    let status = std::process::Command::new("systemctl")
        .args(["--user", verb, UNIT])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("systemctl --user {verb} {UNIT} failed"))
    }
}

/// A GLib key file kept as its lines, so that an edit changes one line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyFile {
    lines: Vec<String>,
}

fn group_of(line: &str) -> Option<&str> {
    let t = line.trim();
    t.strip_prefix('[')?.strip_suffix(']')
}

fn key_of(line: &str) -> Option<(&str, &str)> {
    let t = line.trim_start();
    if t.starts_with('#') || t.is_empty() {
        return None;
    }
    let (k, v) = t.split_once('=')?;
    Some((k.trim_end(), v.trim_start()))
}

impl KeyFile {
    pub fn parse(text: &str) -> KeyFile {
        KeyFile {
            lines: text.lines().map(str::to_string).collect(),
        }
    }

    pub fn text(&self) -> String {
        let mut s = self.lines.join("\n");
        s.push('\n');
        s
    }

    /// The line range of a group's entries (after its header).
    fn group_range(&self, group: &str) -> Option<(usize, usize)> {
        let start = self.lines.iter().position(|l| group_of(l) == Some(group))? + 1;
        let end = self.lines[start..]
            .iter()
            .position(|l| group_of(l).is_some())
            .map_or(self.lines.len(), |i| start + i);
        Some((start, end))
    }

    pub fn get(&self, group: &str, key: &str) -> Option<String> {
        let (start, end) = self.group_range(group)?;
        self.lines[start..end]
            .iter()
            .filter_map(|l| key_of(l))
            .find(|(k, _)| *k == key)
            .map(|(_, v)| unescape(v))
    }

    pub fn set(&mut self, group: &str, key: &str, value: &str) {
        let line = format!("{key}={}", escape(value));
        let Some((start, end)) = self.group_range(group) else {
            if self.lines.last().is_some_and(|l| !l.trim().is_empty()) {
                self.lines.push(String::new());
            }
            self.lines.push(format!("[{group}]"));
            self.lines.push(line);
            return;
        };
        if let Some(i) =
            (start..end).find(|i| key_of(&self.lines[*i]).is_some_and(|(k, _)| k == key))
        {
            self.lines[i] = line;
            return;
        }
        // After the group's last entry, before the blank lines that
        // separate it from the next group.
        let mut at = end;
        while at > start && self.lines[at - 1].trim().is_empty() {
            at -= 1;
        }
        self.lines.insert(at, line);
    }
}

/// GLib's key file escapes: `\s` (a leading space), `\n`, `\t`, `\r`, `\\`.
fn unescape(v: &str) -> String {
    let mut out = String::new();
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

fn escape(v: &str) -> String {
    let mut out = String::new();
    for (i, c) in v.chars().enumerate() {
        match c {
            ' ' if i == 0 => out.push_str("\\s"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file mmsd-tng writes on its first run (values fictional).
    const FIRST_RUN: &str = "\
[Settings]
UseDeliveryReports=false
TotalMaxAttachmentSize=1100000
MaxAttachments=25
AutoCreateSMIL=true
ForceCAres=false

[Modem Manager]
CarrierMMSC=http://mms.invalid
MMS_APN=apn.example.invalid
CarrierMMSProxy=NULL
DefaultModemNumber=NULL
AutoProcessOnConnection=true
AutoProcessSMSWAP=false
";

    #[test]
    fn reads_placeholders_as_empty() {
        let f = KeyFile::parse(FIRST_RUN);
        assert_eq!(f.get(GROUP, APN).as_deref(), Some("apn.example.invalid"));
        assert_eq!(f.get("Settings", "MaxAttachments").as_deref(), Some("25"));
        assert_eq!(f.get(GROUP, "Missing"), None);
        let s = MmsSettings::from_values(
            &f.get(GROUP, MMSC).unwrap(),
            &f.get(GROUP, PROXY).unwrap(),
            &f.get(GROUP, APN).unwrap(),
        );
        assert_eq!(
            s,
            MmsSettings {
                mmsc: String::new(),
                proxy: String::new(),
                apn: "apn.example.invalid".into()
            }
        );
    }

    #[test]
    fn an_edit_changes_one_line_only() {
        let mut f = KeyFile::parse(FIRST_RUN);
        f.set(GROUP, MMSC, "http://mmsc.example.invalid/mms");
        f.set(GROUP, PROXY, &MmsSettings::stored(PROXY, ""));
        let expected = FIRST_RUN.replace(
            "CarrierMMSC=http://mms.invalid",
            "CarrierMMSC=http://mmsc.example.invalid/mms",
        );
        assert_eq!(f.text(), expected);
    }

    #[test]
    fn missing_keys_and_groups_are_added() {
        let mut f = KeyFile::parse("# kept\n[Settings]\nAutoCreateSMIL=true\n\n[Other]\nX=1\n");
        f.set(GROUP, APN, "apn.example.invalid");
        f.set(GROUP, PROXY, "proxy.example.invalid:8080");
        f.set("Settings", "UseDeliveryReports", "false");
        assert_eq!(
            f.text(),
            "# kept\n[Settings]\nAutoCreateSMIL=true\nUseDeliveryReports=false\n\n[Other]\nX=1\n\n\
             [Modem Manager]\nMMS_APN=apn.example.invalid\nCarrierMMSProxy=proxy.example.invalid:8080\n"
        );
        let mut empty = KeyFile::parse("");
        empty.set(GROUP, MMSC, "http://mmsc.example.invalid");
        assert_eq!(
            empty.text(),
            "[Modem Manager]\nCarrierMMSC=http://mmsc.example.invalid\n"
        );
    }

    #[test]
    fn stored_values_and_escapes() {
        assert_eq!(MmsSettings::stored(MMSC, " "), "http://mms.invalid");
        assert_eq!(MmsSettings::stored(PROXY, ""), "NULL");
        assert_eq!(MmsSettings::stored(APN, ""), "NULL");
        assert_eq!(
            MmsSettings::stored(APN, " apn.example.invalid "),
            "apn.example.invalid"
        );
        assert_eq!(unescape(r"\sa\\b\nc"), " a\\b\nc");
        assert_eq!(escape(" a\\b"), r"\sa\\b");
        assert!(check(MMSC, "mmsc.example.invalid").is_err());
        assert!(check(MMSC, "http://mmsc.example.invalid").is_ok());
        assert!(check(PROXY, "http://proxy.example.invalid").is_err());
        assert!(check(PROXY, "proxy.example.invalid:80").is_ok());
        assert!(check(APN, "two words").is_err());
        assert!(check(APN, "").is_ok());
    }

    #[test]
    fn file_round_trip_keeps_the_rest() {
        let dir = std::env::temp_dir().join(format!("omamessages-mms-{}", std::process::id()));
        let p = dir.join("modemmanager/mms");
        let _ = std::fs::remove_dir_all(&dir);
        // No file yet: created with the group.
        write_file(&p, APN, "apn.example.invalid").unwrap();
        assert_eq!(MmsSettings::read_file(&p).apn, "apn.example.invalid");
        std::fs::write(&p, FIRST_RUN).unwrap();
        write_file(&p, PROXY, "proxy.example.invalid:80").unwrap();
        let s = MmsSettings::read_file(&p);
        assert_eq!(s.proxy, "proxy.example.invalid:80");
        assert_eq!(s.mmsc, "");
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("AutoProcessOnConnection=true\n"));
        assert!(text.contains("[Settings]\nUseDeliveryReports=false\n"));
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
