module Misophonia

import Codeware.*

@if(ModuleExists("RedscriptConfigFramework"))
import RedscriptConfigFramework.*

public abstract class MenuRefresh {
  public static func Update() -> Void {
    MenuRefresh.UpdateConfigurator();
    MenuRefresh.UpdateScreens();
  }

  public static func UpdateScreens() -> Void {
    MenuRefresh.RefreshScreens(MenuRefresh.Window());
  }

  public static func Window() -> ref<inkCompoundWidget> {
    let ink: ref<inkSystem> = GameInstance.GetInkSystem();
    if !IsDefined(ink) {
      return null;
    }
    let layer = ink.GetLayer(n"inkMenuLayer");
    return IsDefined(layer) ? layer.GetVirtualWindow() : null;
  }

  private static func RefreshScreens(widget: wref<inkWidget>) -> Void {
    if !IsDefined(widget) {
      return;
    }
    for controller in widget.GetControllers() {
      let row = controller as SettingsSelectorController;
      if IsDefined(row) && Equals(row.GetGroupPath(), n"/mods/Misophonia/Misophonia.Config")
        && Equals(row.GetVarName(), n"screenPercent") {
        Reflection.Call(row, n"OnUpdateValue");
        return;
      }
    }
    let compound = widget as inkCompoundWidget;
    if IsDefined(compound) {
      let index = 0;
      let children = compound.GetNumChildren();
      while index < children {
        MenuRefresh.RefreshScreens(compound.GetWidgetByIndex(index));
        index += 1;
      }
    }
  }

  @if(ModuleExists("RedscriptConfigFramework"))
  private static func UpdateConfigurator() -> Void {
    let hub = DVRCF_Hub.Get(GetGameInstance());
    if !IsDefined(hub) {
      return;
    }
    let cls = Reflection.GetClassOf(hub);
    let menu = cls.GetProperty(n"m_pregamePopup");
    let popup: ref<DVRCF_HubPopup>;
    if IsDefined(menu) {
      popup = FromVariant<wref<DVRCF_HubPopup>>(menu.GetValue(hub));
    }
    if !IsDefined(popup) {
      menu = cls.GetProperty(n"m_popup");
      if IsDefined(menu) {
        popup = FromVariant<ref<DVRCF_HubPopup>>(menu.GetValue(hub));
      }
    }
    if IsDefined(popup) && !popup.IsClosed() && popup.IsShowingConfig("MS:Misophonia") {
      Reflection.Call(popup, n"RefreshActiveSchema");
    }
  }

  @if(!ModuleExists("RedscriptConfigFramework"))
  private static func UpdateConfigurator() -> Void {}
}
