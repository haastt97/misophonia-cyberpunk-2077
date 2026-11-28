use anyhow::{Context, Result, ensure};

pub(super) fn render(source: &str) -> Result<String> {
    let mut script = source.replace("\r\n", "\n");
    if script.contains("|| ArraySize(sections) != Catalog.SectionCount()") {
        return Ok(script);
    }

    replace(
        &mut script,
        "  public let events: array<CName>;\n",
        "  public let events: array<CName>;\n  public let replacements: array<String>;\n",
    )?;
    replace(
        &mut script,
        "  public let categories: array<ref<CategoryControl>>;\n",
        "  public let categories: array<ref<CategoryControl>>;\n  public let hasScreens: Bool;\n",
    )?;

    let discovery_start = script
        .find("  private func Discover() -> Bool {\n")
        .context("The selected LiveSettings.reds has no settings discovery function")?;
    let discovery_end = script[discovery_start..]
        .find("  // The last request wins")
        .map(|offset| discovery_start + offset)
        .context("Could not locate the end of settings discovery")?;
    script.replace_range(discovery_start..discovery_end, DISCOVER);

    if script.contains("let screensBefore = this.screens.GetValue();") {
        replace(
            &mut script,
            "let screensBefore = this.screens.GetValue();",
            "let screensBefore = IsDefined(this.screens) ? this.screens.GetValue() : 100;",
        )?;
        replace(
            &mut script,
            "if this.screens.GetValue() != screensBefore {",
            "if IsDefined(this.screens) && this.screens.GetValue() != screensBefore {",
        )?;
    }

    replace(
        &mut script,
        "    if rootOrder > this.Order(n\"screenPercent\") {",
        "    let sliderOrder = this.Order(n\"screenPercent\");\n    if IsDefined(this.screens) && rootOrder > sliderOrder {",
    )?;
    replace(
        &mut script,
        "    let allMuted = this.screens.GetValue() == 0;\n",
        "    let allMuted = true;\n",
    )?;
    replace(
        &mut script,
        "      updated = this.Write(section.option, sectionMuted) || updated;",
        r#"      if section.hasScreens && IsDefined(this.screens) {
        if parentOrder > sliderOrder {
          let sectionVolume = parentValue ? 0 : 100;
          if this.screens.GetValue() != sectionVolume {
            this.screens.SetValue(sectionVolume);
            updated = true;
          }
        }
        sectionMuted = sectionMuted && this.screens.GetValue() == 0;
      }
      updated = this.Write(section.option, sectionMuted) || updated;"#,
    )?;
    replace(
        &mut script,
        "    this.previousMaster = all && allMuted;",
        "    allMuted = allMuted && (!IsDefined(this.screens) || this.screens.GetValue() == 0);\n    this.previousMaster = all && allMuted;",
    )?;
    replace(
        &mut script,
        r#"            if mute && !category.registered[index] {
              category.registered[index] = AudioXLAPI.Mute("Misophonia", event);
            }"#,
        r#"            if mute && !category.registered[index] {
              let replacement = category.replacements[index];
              if StrLen(replacement) > 0 {
                category.registered[index] = AudioXLAPI.RegisterSound("Misophonia", event, n"mod_sfx_occlusion", replacement);
              } else {
                category.registered[index] = AudioXLAPI.Mute("Misophonia", event);
              }
              if !category.registered[index] {
                category.applied = false;
                complete = false;
              }
            }"#,
    )?;
    replace(
        &mut script,
        "            if mute || category.registered[index] {",
        "            if category.registered[index] {",
    )?;
    replace(
        &mut script,
        "    let percent = this.screens.GetValue();",
        "    if !IsDefined(this.screens) {\n      return complete;\n    }\n    let percent = this.screens.GetValue();",
    )?;
    Ok(script)
}

fn replace(script: &mut String, original: &str, replacement: &str) -> Result<()> {
    let mut anchors = script.match_indices(original);
    let (start, _) = anchors.next().with_context(|| {
        format!(
            "The selected LiveSettings.reds lacks an expected statement: {}",
            original.trim()
        )
    })?;
    ensure!(
        anchors.next().is_none(),
        "The selected LiveSettings.reds repeats an expected statement: {}",
        original.trim()
    );
    script.replace_range(start..start + original.len(), replacement);
    Ok(())
}

const DISCOVER: &str = r#"  private func Discover() -> Bool {
    let catalog = Catalog.Create();
    let sections: array<ref<SectionControl>>;
    let resolved: Int32 = 0;
    for name in ModSettings.GetCategories(n"Misophonia") {
      let section: ref<SectionControl> = new SectionControl();
      section.hasScreens = Catalog.HasScreens() && Equals(name, Catalog.ScreensSection());
      for row in ModSettings.GetVars(n"Misophonia", name) {
        let rowName = row.GetName();
        if Equals(rowName, n"muteAll") {
          this.master = row as ModConfigVarBool;
        } else {
          if Catalog.HasScreens() && Equals(rowName, n"screenPercent") {
            this.screens = row as ModConfigVarInt32;
          } else {
            let setting = row as ModConfigVarBool;
            if IsDefined(setting) {
              for entry in catalog {
                if Equals(rowName, entry.section) {
                  section.option = setting;
                  break;
                }
                if Equals(rowName, entry.setting) {
                  let category: ref<CategoryControl> = new CategoryControl();
                  category.option = setting;
                  category.events = entry.events;
                  category.replacements = entry.replacements;
                  for event in entry.events {
                    ArrayPush(category.registered, false);
                  }
                  ArrayPush(section.categories, category);
                  resolved += 1;
                  break;
                }
              }
            }
          }
        }
      }
      if IsDefined(section.option) && ArraySize(section.categories) > 0 {
        ArrayPush(sections, section);
      }
    }
    if !IsDefined(this.master) || (Catalog.HasScreens() && !IsDefined(this.screens))
      || ArraySize(sections) != Catalog.SectionCount() || resolved != ArraySize(catalog) {
      return false;
    }
    this.sections = sections;
    return true;
  }

"#;
