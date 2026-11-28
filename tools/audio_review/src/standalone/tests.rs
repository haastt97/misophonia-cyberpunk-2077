use serde_json::json;

use crate::build_plan::{CategoryPlan, StandInPlan};

use super::*;

fn metadata(rows: &[(&str, u32)]) -> Value {
    let rows: Vec<_> = rows
        .iter()
        .map(|(name, id)| json!({"redId": {"$value": name}, "wwiseId": id}))
        .collect();
    json!({"Data": {"RootChunk": {"root": {"Data": {"events": rows}}}}})
}

fn category(section: &str, screens: bool, actions: &[(&str, Option<&str>)]) -> CategoryPlan {
    CategoryPlan {
        id: section.to_owned(),
        section: section.to_owned(),
        name: section.to_owned(),
        screens,
        events: actions
            .iter()
            .map(|(event, donor)| (event.to_string(), donor.map(str::to_owned)))
            .collect(),
    }
}

#[test]
fn metadata_patch_uses_original_donor_even_if_donor_is_also_muted() -> Result<()> {
    let mut resource = metadata(&[
        ("target", 10),
        ("target", 11),
        ("donor", 42),
        ("untouched", 9),
    ]);
    let actions = Actions::from([
        ("target".to_owned(), Some("donor".to_owned())),
        ("donor".to_owned(), None),
    ]);
    apply_actions(&mut resource, &actions)?;
    assert_eq!(
        resource,
        metadata(&[
            ("target", 42),
            ("target", 42),
            ("donor", 0),
            ("untouched", 9)
        ])
    );
    Ok(())
}

#[test]
fn rejects_ambiguous_or_missing_metadata_without_writing_partial_changes() {
    let mut resource = metadata(&[("target", 10), ("donor", 20), ("donor", 21)]);
    let original = resource.clone();
    let actions = Actions::from([("target".to_owned(), Some("donor".to_owned()))]);
    let error = apply_actions(&mut resource, &actions).unwrap_err();
    assert!(error.downcast_ref::<Unsupported>().is_some());
    assert!(error.to_string().contains("multiple original Wwise IDs"));
    assert_eq!(resource, original);

    let missing = Actions::from([("unknown".to_owned(), None)]);
    let error = apply_actions(&mut resource, &missing).unwrap_err();
    assert!(error.downcast_ref::<Unsupported>().is_some());
    assert!(error.to_string().contains("unknown"));
    assert_eq!(resource, original);
}

#[test]
fn options_use_safe_distinct_filenames_and_report_unsupported_donors() -> Result<()> {
    let plan = BuildPlan {
        categories: vec![
            category("General", true, &[("tv", None)]),
            category("a / b", false, &[("target", Some("non-native-donor"))]),
            category("a / b", false, &[("another", None)]),
            category("A-B", false, &[("different", None)]),
            category("screens", false, &[("unrelated", None)]),
            category("Empty", false, &[]),
        ],
        shared_events: BTreeSet::new(),
        stand_ins: BTreeMap::from([(
            "non-native-donor".to_owned(),
            StandInPlan {
                name: "Unlinked clip".to_owned(),
                event: None,
            },
        )]),
    };
    let variants = options(&plan)?;
    let names: Vec<_> = variants.iter().map(|(label, _)| label.as_str()).collect();
    assert_eq!(
        names,
        ["All-Covered-Sounds", "Screens", "A-B", "a-b-2", "screens-2"]
    );
    assert_eq!(variants[0].1.len(), 5);
    assert_eq!(variants[3].1.len(), 2);
    assert!(
        resolve_donors(&plan, variants[3].1.clone())
            .unwrap_err()
            .to_string()
            .contains("Unlinked clip")
    );
    assert_eq!(resolve_donors(&plan, variants[1].1.clone())?, variants[1].1);

    let skipped = build(
        Path::new("."),
        Path::new("unneeded"),
        Path::new("."),
        "Test",
        &BuildPlan {
            categories: vec![category(
                "a / b",
                false,
                &[("target", Some("non-native-donor"))],
            )],
            shared_events: BTreeSet::new(),
            stand_ins: plan.stand_ins,
        },
        &AtomicBool::default(),
    )?;
    assert_eq!(skipped.len(), 2);
    assert!(
        skipped
            .iter()
            .all(|message| message.contains("no exact vanilla event"))
    );
    Ok(())
}
