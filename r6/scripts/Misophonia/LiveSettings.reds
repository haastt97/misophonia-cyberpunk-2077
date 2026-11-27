module Misophonia

import AudioXL.*
import Codeware.*
import ModSettingsModule.*

public class CategoryControl {
  public let option: ref<ModConfigVarBool>;
  public let events: array<CName>;
  public let registered: array<Bool>;
  public let applied: Bool;
  public let appliedValue: Bool;
}

public class SectionControl {
  public let option: ref<ModConfigVarBool>;
  public let categories: array<ref<CategoryControl>>;
  public let previousValue: Bool;
}

public class LiveSettings extends ScriptableService {
  private let changed: array<CName>;
  private let sections: array<ref<SectionControl>>;
  private let master: ref<ModConfigVarBool>;
  private let screens: ref<ModConfigVarInt32>;
  private let previousMaster: Bool;
  private let appliedVolume: Int32;
  private let propagating: Bool;
  private let accepted: Bool;
  private let scheduled: Bool;
  private let generation: Int32;
  private let clock: ref<inkCanvas>;
  private let animation: ref<inkAnimProxy>;
  private let initialized: Bool;
  private let redrawPending: Bool;

  private cb func OnLoad() {
    this.appliedVolume = -1;
    ModSettings.RegisterListenerToModifications(this);
    GameInstance.GetCallbackSystem().RegisterCallback(n"Session/Ready", this, n"OnSessionReady");
    this.Schedule(0.10);
  }

  private cb func OnSessionReady(event: ref<GameSessionEvent>) {
    this.StopClock();
    this.scheduled = false;
    this.appliedVolume = -1;
    for section in this.sections {
      for category in section.categories {
        category.applied = false;
      }
    }
    this.Schedule(0.10);
  }

  public static func Get() -> ref<LiveSettings> {
    return GameInstance.GetScriptableServiceContainer().GetService(n"Misophonia.LiveSettings") as LiveSettings;
  }

  public func MenuReady() -> Void {
    // The loading screen can suspend an animation queued before the menu was shown.
    if !this.initialized || this.scheduled {
      this.generation += 1;
      this.scheduled = true;
      this.Tick(this.generation);
    }
  }

  private func Schedule(delay: Float) -> Void {
    if this.scheduled {
      if !IsDefined(this.animation) {
        this.StartClock(delay);
      }
      return;
    }
    this.scheduled = true;
    this.generation += 1;
    let scheduler: ref<DelaySystem> = GameInstance.GetDelaySystem(GetGameInstance());
    if IsDefined(scheduler) {
      let callback: ref<SettingsTick> = new SettingsTick();
      callback.service = this;
      callback.generation = this.generation;
      scheduler.DelayCallback(callback, delay, false);
    }
    this.StartClock(delay);
  }

  private func StartClock(delay: Float) -> Void {
    let parent = MenuRefresh.Window();
    if !IsDefined(parent) || IsDefined(this.clock) {
      return;
    }
    let pulse = new inkAnimTransparency();
    pulse.SetStartTransparency(0.0);
    pulse.SetEndTransparency(0.01);
    pulse.SetDuration(delay);
    let animation = new inkAnimDef();
    animation.AddInterpolator(pulse);

    this.clock = new inkCanvas();
    this.clock.SetName(n"MisophoniaUpdateClock");
    this.clock.SetSize(1.0, 1.0);
    this.clock.SetInteractive(false);
    this.clock.Reparent(parent);
    this.animation = this.clock.PlayAnimation(animation);
    this.animation.RegisterToCallback(inkanimEventType.OnFinish, this, n"OnClockFinished");
  }

  private func StopClock() -> Void {
    if IsDefined(this.animation) {
      this.animation.UnregisterFromCallback(inkanimEventType.OnFinish, this, n"OnClockFinished");
      this.animation.Stop();
      this.animation = null;
    }
    if IsDefined(this.clock) {
      let parent = this.clock.GetParentWidget() as inkCompoundWidget;
      if IsDefined(parent) {
        parent.RemoveChild(this.clock);
      }
      this.clock = null;
    }
  }

  private cb func OnClockFinished(proxy: ref<inkAnimProxy>) -> Bool {
    if Equals(proxy, this.animation) {
      this.Tick(this.generation);
    }
    return true;
  }

  public func OnModVariableChangeRequested(groupPath: CName, varName: CName) -> Void {
    if !this.propagating && Equals(groupPath, n"/mods/Misophonia/Misophonia.Config") {
      ArrayPush(this.changed, varName);
      if this.initialized {
        // Propagate values in the same settings transaction. Audio and redraw run on the tick.
        this.redrawPending = this.SyncControls(false) || this.redrawPending;
      }
      this.Schedule(0.03);
    }
  }

  public func OnModVariableChangeAccepted(groupPath: CName, varName: CName) -> Void {
    if Equals(groupPath, n"/mods/Misophonia/Misophonia.Config") {
      this.accepted = true;
    }
  }

  public func OnModSettingsChange() -> Void {
    if !this.propagating {
      if !this.accepted && !ModSettings.GetInstance().changesRequested {
        ArrayClear(this.changed);
        this.redrawPending = true;
      }
      this.accepted = false;
      this.Schedule(0.03);
    }
  }

  private func Discover() -> Bool {
    let catalog = Catalog.Create();
    let sections: array<ref<SectionControl>>;
    let resolved: Int32 = 0;
    for name in ModSettings.GetCategories(n"Misophonia") {
      let rows = ModSettings.GetVars(n"Misophonia", name);
      if Equals(name, n"General") {
        for row in rows {
          switch row.GetName() {
            case n"muteAll":
              this.master = row as ModConfigVarBool;
              break;
            case n"screenPercent":
              this.screens = row as ModConfigVarInt32;
              break;
          }
        }
      } else {
        let section: ref<SectionControl> = new SectionControl();
        for row in rows {
          let setting = row as ModConfigVarBool;
          for entry in catalog {
            if Equals(row.GetName(), entry.section) {
              section.option = setting;
            }
            if Equals(row.GetName(), entry.setting) && IsDefined(setting) {
              let category: ref<CategoryControl> = new CategoryControl();
              category.option = setting;
              category.events = entry.events;
              for event in entry.events {
                ArrayPush(category.registered, false);
              }
              ArrayPush(section.categories, category);
              resolved += 1;
              break;
            }
          }
        }
        if IsDefined(section.option) && ArraySize(section.categories) > 0 {
          ArrayPush(sections, section);
        }
      }
    }
    if !IsDefined(this.master) || !IsDefined(this.screens)
      || ArraySize(sections) != 6 || resolved != ArraySize(catalog) {
      return false;
    }
    this.sections = sections;
    return true;
  }

  // The last request wins when several switches are edited before the next tick.
  private func Order(name: CName) -> Int32 {
    let index = ArraySize(this.changed) - 1;
    while index >= 0 {
      if Equals(this.changed[index], name) {
        return index + 1;
      }
      index -= 1;
    }
    return 0;
  }

  private func Write(option: ref<ModConfigVarBool>, value: Bool) -> Bool {
    if Equals(option.GetValue(), value) {
      return false;
    }
    option.SetValue(value);
    return true;
  }

  private func SyncControls(saveBatchedChanges: Bool) -> Bool {
    let alreadySaved = !ModSettings.GetInstance().changesRequested;
    let all: Bool = this.master.GetValue();
    let rootOrder = this.Order(n"muteAll");
    if this.initialized {
      if Equals(all, this.previousMaster) {
        rootOrder = 0;
      }
    } else {
      rootOrder = all ? 1 : 0;
    }

    this.propagating = true;
    let updated = false;
    if rootOrder > this.Order(n"screenPercent") {
      let expected = all ? 0 : 100;
      if this.screens.GetValue() != expected {
        this.screens.SetValue(expected);
        updated = true;
      }
    }
    let allMuted = this.screens.GetValue() == 0;
    for section in this.sections {
      let parentOrder = rootOrder;
      let parentValue = all;
      let order = this.Order(section.option.GetName());
      if this.initialized && order > parentOrder
        && !Equals(section.option.GetValue(), section.previousValue) {
        parentOrder = order;
        parentValue = section.option.GetValue();
      }

      let sectionMuted = true;
      for category in section.categories {
        if parentOrder > this.Order(category.option.GetName()) {
          updated = this.Write(category.option, parentValue) || updated;
        }
        sectionMuted = sectionMuted && category.option.GetValue();
      }
      updated = this.Write(section.option, sectionMuted) || updated;
      section.previousValue = sectionMuted;
      allMuted = allMuted && sectionMuted;
    }
    this.previousMaster = all && allMuted;
    updated = this.Write(this.master, this.previousMaster) || updated;
    this.propagating = false;
    this.initialized = true;
    ArrayClear(this.changed);
    if updated && alreadySaved && saveBatchedChanges {
      ModSettings.AcceptChanges();
    }
    return updated;
  }

  private func ApplyAudio() -> Bool {
    if !AudioXLNative.Enabled() {
      return false;
    }
    let complete = true;
    for section in this.sections {
      for category in section.categories {
        let mute = category.option.GetValue();
        if !category.applied || !Equals(category.appliedValue, mute) {
          category.applied = true;
          let index = 0;
          for event in category.events {
            if mute && !category.registered[index] {
              category.registered[index] = AudioXLAPI.Mute("Misophonia", event);
            }
            if mute || category.registered[index] {
              let enabled = AudioXLAPI.IsEnabled(event);
              if !Equals(enabled, mute) {
                AudioXLAPI.SetEnabled(event, mute);
              }
              if !Equals(AudioXLAPI.IsEnabled(event), mute) {
                category.applied = false;
                complete = false;
              }
            }
            index += 1;
          }
          category.appliedValue = mute;
        }
      }
    }

    let percent = this.screens.GetValue();
    if this.appliedVolume != percent {
      let tv = -1;
      let ads = -1;
      let index = 0;
      let buses = AudioXLNative.MixCount();
      while index < buses {
        switch AudioXLNative.MixKey(index) {
          case "tv":
            tv = index;
            break;
          case "ads":
            ads = index;
            break;
        }
        index += 1;
      }
      if tv < 0 || ads < 0 {
        return false;
      }
      let gain = Catalog.ScreenGain(percent);
      AudioXLNative.MixSet(tv, gain);
      AudioXLNative.MixSet(ads, gain);
      if AudioXLNative.MixApply() < 0 {
        return false;
      }
      this.appliedVolume = percent;
    }
    return complete;
  }

  public func Tick(generation: Int32) -> Void {
    if !this.scheduled || generation != this.generation {
      return;
    }
    this.scheduled = false;
    this.StopClock();
    if ArraySize(this.sections) == 0 && !this.Discover() {
      this.Schedule(0.50);
      return;
    }
    let redraw = this.SyncControls(true) || this.redrawPending;
    this.redrawPending = false;
    if !this.ApplyAudio() {
      this.Schedule(0.50);
    }
    if redraw {
      MenuRefresh.Update();
    }
  }
}

public class SettingsTick extends DelayCallback {
  public let service: wref<LiveSettings>;
  public let generation: Int32;

  public func Call() -> Void {
    if IsDefined(this.service) {
      this.service.Tick(this.generation);
    }
  }
}

@wrapMethod(SingleplayerMenuGameController)
private func PopulateMenuItemList() -> Void {
  wrappedMethod();
  let service = LiveSettings.Get();
  if IsDefined(service) {
    service.MenuReady();
  }
}
