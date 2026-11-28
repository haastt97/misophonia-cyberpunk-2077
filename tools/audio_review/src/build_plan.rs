use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::review::Review;

#[derive(Debug, Default, Serialize)]
pub struct BuildPlan {
    pub categories: Vec<CategoryPlan>,
    pub stand_ins: BTreeMap<String, StandInPlan>,
    pub shared_events: BTreeSet<String>,
}

#[derive(Debug, Serialize)]
pub struct CategoryPlan {
    pub id: String,
    pub section: String,
    pub name: String,
    pub screens: bool,
    pub events: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Serialize)]
pub struct StandInPlan {
    pub name: String,
    pub event: Option<String>,
}

impl BuildPlan {
    pub fn from_review(review: &Review) -> Result<Self> {
        let mut categories: Vec<_> = review
            .categories
            .iter()
            .map(|item| CategoryPlan {
                id: item.id.clone(),
                section: item.section.clone(),
                name: item.name.clone(),
                screens: item.mode == "volume",
                events: BTreeMap::new(),
            })
            .collect();
        let by_category: HashMap<_, _> = categories
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.clone(), i))
            .collect();
        let mut by_event = BTreeMap::new();
        for (category, event) in review.catalog.event_assignments() {
            if let Some(&index) = by_category.get(category) {
                ensure!(
                    by_event.insert(event.to_owned(), (index, None)).is_none(),
                    "The base catalog assigns {event} to multiple active categories"
                );
            }
        }

        let tags = review.saved_tags()?;
        let mut media_by_event = HashMap::new();
        for (media, events) in &review.media_events {
            for event in events {
                media_by_event
                    .entry(event.as_str())
                    .and_modify(|id| *id = None)
                    .or_insert(Some(*media));
            }
        }
        let mut stand_ins = BTreeMap::new();
        let mut removals = BTreeSet::new();
        let mut additions = BTreeMap::new();
        for (id, tag) in &tags {
            if !tag.candidate && !tag.remove_coverage {
                continue;
            }
            let sound = review.index(id).map(|index| &review.sounds[index])
                .with_context(|| format!("Reviewed sound {id} is absent from the library. Open the installation where it was tagged"))?;
            let name = &sound.asset.name;
            ensure!(
                !(tag.candidate && tag.remove_coverage),
                "{name} is marked both as a trigger and for removal. Choose one action before building"
            );
            if tag.remove_coverage {
                removals.extend(
                    sound
                        .coverage
                        .iter()
                        .flat_map(|coverage| coverage.events.iter())
                        .cloned(),
                );
                continue;
            }
            let key = tag
                .category
                .as_deref()
                .with_context(|| format!("Choose a subcategory for {name}"))?;
            let &index = by_category
                .get(key)
                .with_context(|| format!("Choose an available subcategory for {name}"))?;
            ensure!(
                !sound.events.is_empty(),
                "{name} has no linked Cyberpunk audio event. Clear its trigger tag to build"
            );
            if let Some(donor_id) = &tag.replacement {
                let donor = review
                    .index(donor_id)
                    .map(|index| &review.sounds[index])
                    .filter(|donor| donor.playable)
                    .with_context(|| format!("Choose an installed stand-in for {name}"))?;
                ensure!(
                    donor_id != id
                        && tags.get(donor_id).is_some_and(|donor_tag| {
                            donor_tag.stand_in
                                && donor_tag.stand_in_category.as_deref() == Some(key)
                        }),
                    "Choose a stand-in in the same subcategory as {name}"
                );
                stand_ins
                    .entry(donor_id.clone())
                    .or_insert_with(|| StandInPlan {
                        name: donor.asset.name.clone(),
                        event: donor.asset.media_id.and_then(|media| {
                            donor
                                .events
                                .iter()
                                .find(|event| {
                                    media_by_event.get(event.as_str()) == Some(&Some(media))
                                })
                                .cloned()
                        }),
                    });
            }
            for event in &sound.events {
                let desired = (index, tag.replacement.clone());
                if let Some(previous) = additions.insert(event.clone(), desired.clone()) {
                    ensure!(
                        previous == desired,
                        "Reviewed clips sharing {event} request different actions. Use one subcategory and replacement for that event"
                    );
                }
            }
        }
        let shared_events = removals
            .iter()
            .chain(additions.keys())
            .filter(|event| media_by_event.get(event.as_str()) == Some(&None))
            .cloned()
            .collect();
        for event in removals {
            ensure!(
                !additions.contains_key(&event),
                "Reviewed clips both add and remove {event}. Choose one action for that event"
            );
            by_event.remove(&event);
        }
        by_event.extend(additions);
        for (event, (index, donor)) in by_event {
            categories[index].events.insert(event, donor);
        }
        Ok(Self {
            categories,
            stand_ins,
            shared_events,
        })
    }
}
