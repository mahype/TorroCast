//! Two devices and a folder that syncs like Dropbox does: whole files, late, sometimes twice.

use std::fs;
use std::path::{Path, PathBuf};

use torrocast_library::{Change, Folder, OpenError, StoredItem, UP_NEXT};

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("torrocast-library-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}

fn subscribed(title: &str) -> Change {
    Change::Subscribed {
        podcast: title.to_lowercase(),
        feed_url: format!("https://{title}.example"),
        title: title.into(),
    }
}

fn titles(folder: &Folder) -> Vec<String> {
    folder.state().subscriptions().into_iter().map(|subscription| subscription.title).collect()
}

/// Copies every file of one device's directory to the other library — what a sync service does.
fn sync(from: &Path, to: &Path, device: &str) {
    let target = to.join("devices").join(device);
    fs::create_dir_all(&target).expect("writable");
    for entry in fs::read_dir(from.join("devices").join(device)).expect("the device has written") {
        let entry = entry.expect("readable");
        fs::copy(entry.path(), target.join(entry.file_name())).expect("copyable");
    }
}

#[test]
fn what_is_recorded_is_there_after_a_restart() {
    let root = scratch("restart");
    {
        let mut folder = Folder::open(&root, "laptop", "Laptop", 1_000).expect("an empty folder becomes a library");
        folder.record(subscribed("Alpha"), 2_000).expect("writable");
        folder
            .record(
                Change::PlaybackUpdated {
                    episode: "e1".into(),
                    position_ms: 90_000,
                    duration_ms: Some(600_000),
                    played: false,
                },
                3_000,
            )
            .expect("writable");
    }
    let folder = Folder::open(&root, "laptop", "Laptop", 4_000).expect("reopens");
    assert_eq!(titles(&folder), vec!["Alpha"]);
    assert_eq!(folder.state().progress("e1").map(|progress| progress.position_ms), Some(90_000));
    let journal = fs::read_to_string(root.join("devices/laptop/journal-000001.jsonl")).expect("the journal exists");
    assert_eq!(journal.lines().count(), 3, "registration, subscription, position: one line each");
    assert!(
        journal.lines().all(|line| line.starts_with('{') && line.contains("\"hlc\":\"1970-01-01T00:00:0")),
        "plain, readable JSON"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn two_devices_never_write_the_same_file_and_still_agree() {
    let (here, there) = (scratch("here"), scratch("there"));
    let mut laptop = Folder::open(&here, "laptop", "Laptop", 1_000).expect("opens");
    let mut desktop = Folder::open(&there, "desktop", "Desktop", 1_000).expect("opens");

    // Both offline, both busy.
    laptop.record(subscribed("Alpha"), 2_000).expect("writable");
    desktop.record(subscribed("Beta"), 2_500).expect("writable");
    desktop.record(Change::Unsubscribed { podcast: "alpha".into() }, 1_500).expect("writable");
    let item = StoredItem {
        title: "Episode".into(),
        podcast: "Beta".into(),
        feed_url: None,
        guid: None,
        audio_url: "https://b.example/1.mp3".into(),
        duration_ms: None,
        chapters_url: None,
        is_mp3: true,
        artwork_url: None,
    };
    desktop
        .record(Change::QueueItemSet { playlist: UP_NEXT.into(), episode: "e9".into(), sort: 0.0, item }, 3_000)
        .expect("writable");

    // The sync service catches up, in both directions.
    sync(&here, &there, "laptop");
    sync(&there, &here, "desktop");
    assert!(laptop.rescan(5_000).expect("readable"));
    assert!(desktop.rescan(5_000).expect("readable"));
    assert!(!laptop.rescan(6_000).expect("readable"), "nothing new the second time");

    assert_eq!(titles(&laptop), titles(&desktop));
    assert_eq!(
        titles(&laptop),
        vec!["Beta"],
        "the desktop's clock was behind, yet its later change still counts as later"
    );
    assert_eq!(laptop.state().playlist(UP_NEXT).len(), 1);
    for root in [here, there] {
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn a_sync_services_leftovers_do_no_harm() {
    let root = scratch("leftovers");
    let mut folder = Folder::open(&root, "laptop", "Laptop", 1_000).expect("opens");
    folder.record(subscribed("Alpha"), 2_000).expect("writable");
    let own = root.join("devices/laptop");
    let journal = own.join("journal-000001.jsonl");

    // A conflicted copy, a temp file, a half-synced line, a damaged line, a stray file.
    fs::copy(&journal, own.join("journal-000001 (conflicted copy 2026-09-19).jsonl")).expect("copyable");
    fs::write(own.join(".syncthing.journal-000001.jsonl.tmp"), "garbage").expect("writable");
    fs::create_dir_all(root.join("devices/phone")).expect("writable");
    let beta = r#"{"v":1,"id":"phone:1","hlc":"2026-09-19T10:00:00.000Z-0000-phone","t":"Subscribed","podcast":"beta","feed_url":"https://beta.example","title":"Beta"}"#;
    let future = r#"{"v":7,"id":"phone:2","hlc":"2026-09-19T10:00:01.000Z-0000-phone","t":"SomethingNew","field":1}"#;
    fs::write(
        root.join("devices/phone/journal-000001.jsonl"),
        format!("{beta}\nnot json at all\n{future}\n{{\"v\":1,\"id\":\"phone:3\",\"hl"),
    )
    .expect("writable");
    fs::write(root.join("devices/.DS_Store"), "x").expect("writable");

    drop(folder);
    let folder = Folder::open(&root, "laptop", "Laptop", 9_000).expect("opens among the leftovers");
    assert_eq!(titles(&folder), vec!["Alpha", "Beta"]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_library_from_the_future_is_left_alone() {
    let root = scratch("future");
    fs::create_dir_all(&root).expect("writable");
    fs::write(
        root.join("format.json"),
        r#"{"format":"torrocast-library","format_version":3,"min_reader_version":2,"min_writer_version":3}"#,
    )
    .expect("writable");
    assert!(matches!(Folder::open(&root, "laptop", "Laptop", 1_000), Err(OpenError::TooNew)));
    fs::write(root.join("format.json"), r#"{"format":"something-else"}"#).expect("writable");
    assert!(matches!(Folder::open(&root, "laptop", "Laptop", 1_000), Err(OpenError::NotALibrary)));
    let _ = fs::remove_dir_all(root);
}
