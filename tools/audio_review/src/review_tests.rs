use super::*;

fn setup(data: &Path) -> Result<Review> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let catalog = Catalog::read(&root)?;
    let screen_event = fs::read_to_string(root.join("data/screen-events.txt"))?
        .lines()
        .next()
        .unwrap()
        .to_owned();
    let asset = |id: &str, name: &str, kind: &str, media_id| Asset {
        id: id.to_owned(),
        name: name.to_owned(),
        kind: kind.to_owned(),
        detail: "Audio archive".to_owned(),
        archive: Some("content/audio.archive".to_owned()),
        path: None,
        media_id,
    };
    let assets = vec![
        asset("donor", "Tone", "sfx", Some(200)),
        asset("trigger", "Chewing", "sfx", Some(42)),
        asset("screen", "TV clip", "sound", Some(10)),
    ];
    let media = MediaEvents::from([
        (42, ["cmn_generic_male_eat_chrunchy".to_owned()].into()),
        (10, [screen_event].into()),
        (200, ["ambient_stand_in_test".to_owned()].into()),
    ]);
    Review::open(catalog, assets, &media, data)
}

fn candidate(category: &str, replacement: Option<&str>) -> Tags {
    Tags {
        candidate: true,
        category: Some(category.to_owned()),
        replacement: replacement.map(str::to_owned),
        ..Tags::default()
    }
}

fn donor(category: &str) -> Tags {
    Tags {
        stand_in: true,
        stand_in_category: Some(category.to_owned()),
        ..Tags::default()
    }
}

#[test]
fn opens_legacy_sqlite_and_exports_the_same_review_schema() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let old = Connection::open(dir.path().join("review.sqlite3"))?;
    old.execute_batch(
        "CREATE TABLE tags (
            id TEXT PRIMARY KEY, candidate INTEGER NOT NULL, category TEXT,
            stand_in INTEGER NOT NULL, stand_in_category TEXT, replacement TEXT
         );
         INSERT INTO tags VALUES ('donor', 0, NULL, 1, 'food', NULL);
         INSERT INTO tags VALUES ('trigger', 1, 'food', 0, NULL, 'donor');
         INSERT INTO tags VALUES ('missing', 1, 'food', 0, NULL, NULL);",
    )?;
    drop(old);
    let review = setup(dir.path())?;
    assert_eq!(review.tags("trigger"), &candidate("food", Some("donor")));
    assert_eq!(review.stand_ins(Some("food"), "trigger")[0].id, "donor");
    assert!(review.stand_ins(Some("food"), "donor").is_empty());
    let path = dir.path().join("review.json");
    review.write_export(&path)?;
    let data: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    assert_eq!(data["schema_version"], 1);
    assert_eq!(data["mod_version"], review.catalog.version);
    assert_eq!(data["sounds"][1]["unavailable"], true);
    assert_eq!(data["sounds"][2]["tags"]["replacement"], "donor");
    assert_eq!(
        data["sounds"][2]["events"][0],
        "cmn_generic_male_eat_chrunchy"
    );
    assert_eq!(data["sounds"][2]["media_id"], 42);
    Ok(())
}

#[test]
fn replacement_is_persistent_and_resets_when_donor_moves_or_is_removed() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut review = setup(dir.path())?;
    review.save("donor", donor("breathing"))?;
    assert!(
        review
            .save("trigger", candidate("food", Some("donor")))
            .unwrap_err()
            .to_string()
            .contains("same category")
    );
    assert!(
        review
            .save("trigger", candidate("food", Some("trigger")))
            .is_err()
    );
    assert!(review.save("trigger", candidate("invalid", None)).is_err());
    review.save("donor", donor("food"))?;
    review.save("trigger", candidate("food", Some("donor")))?;
    drop(review);
    let mut review = setup(dir.path())?;
    assert_eq!(review.tags("trigger").replacement.as_deref(), Some("donor"));
    assert_eq!(review.save("donor", donor("breathing"))?, 1);
    assert_eq!(review.tags("trigger").replacement, None);
    review.save("donor", donor("food"))?;
    review.save("trigger", candidate("food", Some("donor")))?;
    assert_eq!(review.save("donor", Tags::default())?, 1);
    assert_eq!(setup(dir.path())?.tags("trigger").replacement, None);
    Ok(())
}

#[test]
fn outside_coverage_excludes_event_only_rows_and_ignores_review_tags() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut review = setup(dir.path())?;
    let screen = &review.sounds[review.index("screen").unwrap()];
    assert!(
        screen
            .coverage
            .iter()
            .any(|entry| entry.mode == "volume" && entry.edition == "Configurable")
    );
    assert!(
        screen
            .coverage
            .iter()
            .any(|entry| entry.mode == "mute" && entry.edition == "Standalone")
    );
    let event_only = review
        .sounds
        .iter()
        .find(|sound| !sound.playable)
        .unwrap()
        .asset
        .id
        .clone();
    assert!(review.save(&event_only, donor("food")).is_err());
    let outside = review.filter("", View::Outside, None, None);
    assert_eq!(outside, vec![review.index("donor").unwrap()]);
    review.save("donor", candidate("food", None))?;
    assert_eq!(review.filter("", View::Outside, None, None), outside);
    assert_eq!(
        review.filter("eat/chruNchy", View::Covered, Some("food"), Some("sfx")),
        vec![review.index("trigger").unwrap()]
    );
    assert_eq!(
        review.filter("", View::Candidates, Some("food"), None),
        outside
    );
    assert!(
        review
            .filter("", View::Covered, None, Some("event"))
            .contains(&review.index(&event_only).unwrap())
    );
    Ok(())
}

#[test]
fn concurrent_windows_do_not_overwrite_newer_tags() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut first = setup(dir.path())?;
    let mut second = setup(dir.path())?;
    first.save("donor", donor("food"))?;
    assert!(
        second
            .save("donor", candidate("food", None))
            .unwrap_err()
            .to_string()
            .contains("another window")
    );
    assert_eq!(second.tags("donor"), &donor("food"));
    let mut tags = second.tags("donor").clone();
    tags.candidate = true;
    tags.category = Some("food".to_owned());
    second.save("donor", tags.clone())?;
    assert_eq!(setup(dir.path())?.tags("donor"), &tags);
    Ok(())
}

#[test]
fn library_category_keeps_sounds_outside_the_mod_visible() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut review = setup(dir.path())?;
    let outside = review.index("donor").unwrap();
    let eating = review.index("trigger").unwrap();
    let screen = review.index("screen").unwrap();

    assert_eq!(
        review.filter("", View::default(), None, None).len(),
        review.sounds.len()
    );
    let general = review.filter("", View::All, Some("screenPercent"), None);
    assert!(general.contains(&screen));
    assert!(general.contains(&outside));
    assert!(!general.contains(&eating));
    assert_eq!(
        review.filter("Tone", View::Outside, Some("screenPercent"), Some("sfx")),
        vec![outside]
    );

    review.save("donor", candidate("food", None))?;
    assert!(
        review
            .filter("", View::All, Some("screenPercent"), None)
            .contains(&outside)
    );
    assert_eq!(
        review.filter("", View::Outside, Some("screenPercent"), None),
        vec![outside]
    );
    assert_eq!(
        review.filter("", View::Candidates, Some("food"), None),
        vec![outside]
    );
    assert!(
        review
            .filter("", View::Candidates, Some("screenPercent"), None)
            .is_empty()
    );
    Ok(())
}

#[test]
fn coverage_removal_candidates_persist_separately_from_trigger_tags() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut review = setup(dir.path())?;
    let screen = review.index("screen").unwrap();
    let removal = Tags {
        remove_coverage: true,
        ..Tags::default()
    };

    assert!(review.save("donor", removal.clone()).is_err());
    review.save("screen", removal.clone())?;
    drop(review);
    let mut review = setup(dir.path())?;
    assert_eq!(review.tags("screen"), &removal);
    assert_eq!(
        review.filter("", View::Candidates, Some("screenPercent"), None),
        vec![screen]
    );
    assert!(
        review
            .filter("", View::Covered, None, None)
            .contains(&screen)
    );
    assert!(
        !review
            .filter("", View::Untagged, None, None)
            .contains(&screen)
    );
    let export = review.export()?;
    let queued = &export["sounds"][0];
    assert_eq!(queued["id"], "screen");
    assert_eq!(queued["tags"]["candidate"], false);
    assert_eq!(queued["tags"]["remove_coverage"], true);
    assert!(!queued["coverage"].as_array().unwrap().is_empty());

    let mut both = donor("screenPercent");
    both.remove_coverage = true;
    review.save("screen", both)?;
    review.save("screen", donor("screenPercent"))?;
    let reopened = setup(dir.path())?;
    assert_eq!(reopened.tags("screen"), &donor("screenPercent"));
    assert!(reopened.filter("", View::Candidates, None, None).is_empty());
    assert!(
        reopened.export()?["sounds"][0]["tags"]
            .get("remove_coverage")
            .is_none()
    );
    Ok(())
}

#[test]
fn category_edits_keep_ids_and_update_the_build_plan() -> Result<()> {
    use crate::build_plan::BuildPlan;
    let dir = tempfile::tempdir()?;
    let mut review = setup(dir.path())?;
    let category = review.add_category("Kitchen", "Sounds")?;
    assert!(review.add_category("kitchen", "sounds").is_err());
    review.rename_section("Kitchen", "Personal")?;
    review.edit_category(&category, "Personal", "Swaps")?;
    review.save_many(vec![
        ("donor".into(), donor(&category)),
        ("trigger".into(), candidate(&category, Some("donor"))),
    ])?;
    let mut review = setup(dir.path())?;
    assert_eq!(review.category(&category).unwrap().name, "Swaps");
    assert_eq!(review.category(&category).unwrap().section, "Personal");
    let plan = BuildPlan::from_review(&review)?;
    let custom = plan
        .categories
        .iter()
        .find(|entry| entry.id == category)
        .unwrap();
    assert_eq!(
        custom.events["cmn_generic_male_eat_chrunchy"].as_deref(),
        Some("donor")
    );
    assert_eq!(
        plan.stand_ins["donor"].event.as_deref(),
        Some("ambient_stand_in_test")
    );

    review.remove_section("Personal")?;
    assert!(review.category(&category).is_none());
    assert!(review.tags("trigger").candidate);
    assert_eq!(review.tags("trigger").replacement, None);
    assert_eq!(review.tags("trigger").category, None);
    assert_eq!(review.tags("donor").stand_in_category, None);
    review.save("trigger", Tags::default())?;
    review.remove_category("screenPercent")?;
    assert!(
        !BuildPlan::from_review(&review)?
            .categories
            .iter()
            .any(|entry| entry.screens)
    );
    Ok(())
}

#[test]
fn bulk_conflict_rolls_back_every_edit() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let mut first = setup(dir.path())?;
    let mut second = setup(dir.path())?;
    first.save("screen", donor("screenPercent"))?;
    let edits = vec![
        ("donor".into(), candidate("food", None)),
        ("screen".into(), candidate("screenPercent", None)),
    ];
    assert!(second.save_many(edits).is_err());
    let review = setup(dir.path())?;
    assert_eq!(review.tags("donor"), &Tags::default());
    assert_eq!(review.tags("screen"), &donor("screenPercent"));
    Ok(())
}
