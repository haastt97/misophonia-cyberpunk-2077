use super::*;

const CATALOG: &str = include_str!("../../../../r6/scripts/Misophonia/Catalog.reds");
const LIVE: &str = include_str!("../../../../r6/scripts/Misophonia/LiveSettings.reds");
const MENU: &str = include_str!("../../../../r6/scripts/Misophonia/MenuRefresh.reds");

fn category(
    id: &str,
    section: &str,
    name: &str,
    screens: bool,
    events: &[(&str, Option<&str>)],
) -> CategoryPlan {
    CategoryPlan {
        id: id.to_owned(),
        section: section.to_owned(),
        name: name.to_owned(),
        screens,
        events: events
            .iter()
            .map(|(event, donor)| (event.to_string(), donor.map(str::to_owned)))
            .collect(),
    }
}

fn plan(categories: Vec<CategoryPlan>) -> BuildPlan {
    BuildPlan {
        categories,
        stand_ins: Default::default(),
        shared_events: Default::default(),
        already_uncovered: Default::default(),
    }
}

#[test]
fn custom_package_preserves_renamed_slider_and_bundles_switchable_audio() -> Result<()> {
    let work = tempfile::tempdir()?;
    let source = work.path().join("source");
    let scripts = source.join(SCRIPTS);
    let output = work.path().join("output");
    fs::create_dir_all(&scripts)?;
    fs::create_dir_all(source.join("data"))?;
    fs::create_dir_all(&output)?;
    fs::write(scripts.join("Catalog.reds"), CATALOG)?;
    fs::write(scripts.join("LiveSettings.reds"), LIVE)?;
    fs::write(scripts.join("MenuRefresh.reds"), MENU)?;
    fs::write(source.join("data/screen-events.txt"), "screen_tv\n")?;
    let wav = work.path().join("donor.wav");
    let mut wave = [0; 48];
    wave[..4].copy_from_slice(b"RIFF");
    wave[8..12].copy_from_slice(b"WAVE");
    fs::write(&wav, wave)?;
    let plan = plan(vec![
        category(
            "screenPercent",
            "City",
            "TV volume",
            true,
            &[("screen_tv", None)],
        ),
        category(
            "from-ui",
            "City",
            "Curtains",
            false,
            &[("curtains_open", Some("donor/a"))],
        ),
        category("custom_3", "City", "Doors", false, &[]),
    ]);

    assert!(
        build(
            &source,
            &output,
            "My Mix",
            &plan,
            &BTreeMap::from([("donor/a".to_owned(), wav)])
        )?
        .is_empty()
    );
    let mut zip = zip::ZipArchive::new(File::open(
        output.join("Misophonia-My Mix-Mod-Settings-PC.zip"),
    )?)?;
    assert_eq!(zip.len(), 5);
    let config = zipped_text(&mut zip, &format!("{SCRIPTS}/Config.reds"))?;
    let catalog = zipped_text(&mut zip, &format!("{SCRIPTS}/Catalog.reds"))?;
    let live = zipped_text(&mut zip, &format!("{SCRIPTS}/LiveSettings.reds"))?;
    let donor_path = format!("{AUDIO}/{}.wav", generated_key("standin", "donor/a"));
    assert!(config.contains("\"ModSettings.category\", \"City\""));
    assert!(config.contains("\"ModSettings.displayName\", \"TV volume\""));
    assert!(config.contains("public let screenPercent: Int32 = 100;"));
    assert!(config.contains("public let custom_3: Bool = false;"));
    assert!(!config.contains("\"Eating and drinking\""));
    assert!(catalog.contains("return n\"City\";"));
    assert!(catalog.contains("n\"curtains_open\""));
    assert!(catalog.contains(&donor_path));
    assert!(catalog.contains("entry.events = noEvents;"));
    assert!(live.contains(
        "AudioXLAPI.RegisterSound(\"Misophonia\", event, n\"mod_sfx_occlusion\", replacement)"
    ));
    assert!(live.contains("AudioXLAPI.SetEnabled(event, mute);"));
    let mut extracted = Vec::new();
    zip.by_name(&donor_path)?.read_to_end(&mut extracted)?;
    assert_eq!(extracted, wave);
    Ok(())
}

#[test]
fn a_modified_screen_list_uses_event_controls() -> Result<()> {
    let plan = plan(vec![category(
        "screenPercent",
        "General",
        "Screens",
        true,
        &[("surviving_event", None)],
    )]);
    let settings = Settings::new(
        &plan,
        &BTreeSet::from(["old_event".to_string(), "surviving_event".to_string()]),
    )?;
    assert_eq!(settings.notices.len(), 1);
    let config = settings.render_config()?;
    assert!(!config.contains(": Int32 = 100"));
    let catalog = settings.render_catalog(CATALOG, &BTreeMap::new())?;
    assert!(catalog.contains("HasScreens() -> Bool { return false; }"));
    assert!(catalog.contains("entry.events = [n\"surviving_event\"];"));
    assert!(!catalog.contains("old_event"));
    Ok(())
}

#[test]
fn a_removed_screen_option_does_not_leave_unused_runtime_references() -> Result<()> {
    let plan = plan(vec![]);
    let settings = Settings::new(&plan, &BTreeSet::from(["screen_tv".to_string()]))?;
    assert!(!settings.render_config()?.contains("screenPercent"));
    assert!(
        settings
            .render_catalog(CATALOG, &BTreeMap::new())?
            .contains("SectionCount() -> Int32 { return 0; }")
    );
    let live = runtime::render(LIVE)?;
    assert!(live.contains("!IsDefined(this.screens) || this.screens.GetValue() == 0"));
    assert!(live.contains("if !IsDefined(this.screens) {\n      return complete;"));
    assert_eq!(runtime::render(&live)?, live);
    Ok(())
}

fn zipped_text(zip: &mut zip::ZipArchive<File>, name: &str) -> Result<String> {
    let mut text = String::new();
    zip.by_name(name)?.read_to_string(&mut text)?;
    Ok(text)
}
