module Misophonia

public class Config {
  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "General")
  @runtimeProperty("ModSettings.category.order", "1")
  @runtimeProperty("ModSettings.order", "1")
  @runtimeProperty("ModSettings.displayName", "All")
  @runtimeProperty("ModSettings.description", "Switch every category on or off and set Screens to 0% or 100%.")
  public let muteAll: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "General")
  @runtimeProperty("ModSettings.category.order", "1")
  @runtimeProperty("ModSettings.order", "2")
  @runtimeProperty("ModSettings.displayName", "Screens")
  @runtimeProperty("ModSettings.description", "TV and advert volume. 100% uses stock gain. 0% silences both.")
  @runtimeProperty("ModSettings.min", "0")
  @runtimeProperty("ModSettings.max", "100")
  @runtimeProperty("ModSettings.step", "1")
  public let screenPercent: Int32 = 100;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Eating and drinking")
  @runtimeProperty("ModSettings.category.order", "2")
  @runtimeProperty("ModSettings.order", "3")
  @runtimeProperty("ModSettings.displayName", "All")
  @runtimeProperty("ModSettings.description", "Switch every mute in this section on or off.")
  public let allFood: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Eating and drinking")
  @runtimeProperty("ModSettings.category.order", "2")
  @runtimeProperty("ModSettings.order", "4")
  @runtimeProperty("ModSettings.displayName", "Eating")
  @runtimeProperty("ModSettings.description", "Mute chewing, bites and eating animations.")
  public let food: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Eating and drinking")
  @runtimeProperty("ModSettings.category.order", "2")
  @runtimeProperty("ModSettings.order", "5")
  @runtimeProperty("ModSettings.displayName", "Drinking")
  @runtimeProperty("ModSettings.description", "Mute drinking and bottle or cup sips.")
  public let drinking: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Eating and drinking")
  @runtimeProperty("ModSettings.category.order", "2")
  @runtimeProperty("ModSettings.order", "6")
  @runtimeProperty("ModSettings.displayName", "Food Misc")
  @runtimeProperty("ModSettings.description", "Mute food containers, takeout packaging and soft food handling.")
  public let food_misc: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Eating and drinking")
  @runtimeProperty("ModSettings.category.order", "2")
  @runtimeProperty("ModSettings.order", "7")
  @runtimeProperty("ModSettings.displayName", "Slurping")
  @runtimeProperty("ModSettings.description", "Mute the slurping sounds in this category.")
  public let slurping: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Eating and drinking")
  @runtimeProperty("ModSettings.category.order", "2")
  @runtimeProperty("ModSettings.order", "8")
  @runtimeProperty("ModSettings.displayName", "Swallowing")
  @runtimeProperty("ModSettings.description", "Mute the swallowing sounds in this category.")
  public let swallowing: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Eating and drinking")
  @runtimeProperty("ModSettings.category.order", "2")
  @runtimeProperty("ModSettings.order", "9")
  @runtimeProperty("ModSettings.displayName", "Cutlery and Plates")
  @runtimeProperty("ModSettings.description", "Mute utensil and plate sounds.")
  public let cutlery: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "10")
  @runtimeProperty("ModSettings.displayName", "All")
  @runtimeProperty("ModSettings.description", "Switch every mute in this section on or off.")
  public let allBody: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "11")
  @runtimeProperty("ModSettings.displayName", "Coughing")
  @runtimeProperty("ModSettings.description", "Mute the coughing sounds in this category.")
  public let coughing: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "12")
  @runtimeProperty("ModSettings.displayName", "Throat Clearing")
  @runtimeProperty("ModSettings.description", "Mute the throat clearing sounds in this category.")
  public let throat_clearing: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "13")
  @runtimeProperty("ModSettings.displayName", "Sniffing")
  @runtimeProperty("ModSettings.description", "Mute the sniffing sounds in this category.")
  public let sniffing: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "14")
  @runtimeProperty("ModSettings.displayName", "Breathing")
  @runtimeProperty("ModSettings.description", "Mute the breathing sounds in this category.")
  public let breathing: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "15")
  @runtimeProperty("ModSettings.displayName", "Snoring")
  @runtimeProperty("ModSettings.description", "Mute the snoring sounds in this category.")
  public let snoring: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "16")
  @runtimeProperty("ModSettings.displayName", "Yawning")
  @runtimeProperty("ModSettings.description", "Mute the yawning sounds in this category.")
  public let yawning: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "17")
  @runtimeProperty("ModSettings.displayName", "Burping")
  @runtimeProperty("ModSettings.description", "Mute the burping sounds in this category.")
  public let burping: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "18")
  @runtimeProperty("ModSettings.displayName", "Spitting")
  @runtimeProperty("ModSettings.description", "Mute the spitting sounds in this category.")
  public let spitting: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "19")
  @runtimeProperty("ModSettings.displayName", "Kissing")
  @runtimeProperty("ModSettings.description", "Mute kissing sounds in the matched scenes.")
  public let kissing: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "20")
  @runtimeProperty("ModSettings.displayName", "Hiccups")
  @runtimeProperty("ModSettings.description", "Mute the hiccups sounds in this category.")
  public let hiccups: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "21")
  @runtimeProperty("ModSettings.displayName", "Gagging")
  @runtimeProperty("ModSettings.description", "Mute the gagging sounds in this category.")
  public let gagging: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "22")
  @runtimeProperty("ModSettings.displayName", "Retching and Vomiting")
  @runtimeProperty("ModSettings.description", "Mute the retching and vomiting sounds in this category.")
  public let retching: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "23")
  @runtimeProperty("ModSettings.displayName", "Joint Cracking")
  @runtimeProperty("ModSettings.description", "Mute the joint cracking sounds in this category.")
  public let joints: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "24")
  @runtimeProperty("ModSettings.displayName", "Choking")
  @runtimeProperty("ModSettings.description", "Mute the choking sounds in this category.")
  public let choking: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Body sounds")
  @runtimeProperty("ModSettings.category.order", "3")
  @runtimeProperty("ModSettings.order", "46")
  @runtimeProperty("ModSettings.displayName", "Player Breathing and Panting")
  @runtimeProperty("ModSettings.description", "Mute V's locomotion breathing and low-stamina panting.")
  public let player_breathing: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Repetitive sounds")
  @runtimeProperty("ModSettings.category.order", "4")
  @runtimeProperty("ModSettings.order", "25")
  @runtimeProperty("ModSettings.displayName", "All")
  @runtimeProperty("ModSettings.description", "Switch every mute in this section on or off.")
  public let allRepetitive: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Repetitive sounds")
  @runtimeProperty("ModSettings.category.order", "4")
  @runtimeProperty("ModSettings.order", "26")
  @runtimeProperty("ModSettings.displayName", "Hand and Table Tapping")
  @runtimeProperty("ModSettings.description", "Mute the hand and table tapping sounds in this category.")
  public let tapping: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Repetitive sounds")
  @runtimeProperty("ModSettings.category.order", "4")
  @runtimeProperty("ModSettings.order", "27")
  @runtimeProperty("ModSettings.displayName", "Keyboard Typing")
  @runtimeProperty("ModSettings.description", "Mute the keyboard typing sounds in this category.")
  public let typing: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Repetitive sounds")
  @runtimeProperty("ModSettings.category.order", "4")
  @runtimeProperty("ModSettings.order", "28")
  @runtimeProperty("ModSettings.displayName", "Phone and Tablet Tapping")
  @runtimeProperty("ModSettings.description", "Mute the phone and tablet tapping sounds in this category.")
  public let phone_taps: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Repetitive sounds")
  @runtimeProperty("ModSettings.category.order", "4")
  @runtimeProperty("ModSettings.order", "29")
  @runtimeProperty("ModSettings.displayName", "Paper and Plastic Rustling")
  @runtimeProperty("ModSettings.description", "Mute book and newspaper handling, paper movement, and plastic rustling.")
  public let rustling: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Repetitive sounds")
  @runtimeProperty("ModSettings.category.order", "4")
  @runtimeProperty("ModSettings.order", "30")
  @runtimeProperty("ModSettings.displayName", "Foot Shuffling")
  @runtimeProperty("ModSettings.description", "Mute the foot shuffling sounds in this category.")
  public let scuffs: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Environmental sounds")
  @runtimeProperty("ModSettings.category.order", "5")
  @runtimeProperty("ModSettings.order", "31")
  @runtimeProperty("ModSettings.displayName", "All")
  @runtimeProperty("ModSettings.description", "Switch every mute in this section on or off.")
  public let allEnvironmental: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Environmental sounds")
  @runtimeProperty("ModSettings.category.order", "5")
  @runtimeProperty("ModSettings.order", "32")
  @runtimeProperty("ModSettings.displayName", "Clock Ticking")
  @runtimeProperty("ModSettings.description", "Mute the clock ticking sounds in this category.")
  public let clock: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Environmental sounds")
  @runtimeProperty("ModSettings.category.order", "5")
  @runtimeProperty("ModSettings.order", "33")
  @runtimeProperty("ModSettings.displayName", "Water Drips")
  @runtimeProperty("ModSettings.description", "Mute the water drips sounds in this category.")
  public let drips: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Environmental sounds")
  @runtimeProperty("ModSettings.category.order", "5")
  @runtimeProperty("ModSettings.order", "34")
  @runtimeProperty("ModSettings.displayName", "Electrical Hums and Buzzes")
  @runtimeProperty("ModSettings.description", "Mute the electrical hums and buzzes sounds in this category.")
  public let electric: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Environmental sounds")
  @runtimeProperty("ModSettings.category.order", "5")
  @runtimeProperty("ModSettings.order", "35")
  @runtimeProperty("ModSettings.displayName", "Cats")
  @runtimeProperty("ModSettings.description", "Mute the cats sounds in this category.")
  public let cats: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Environmental sounds")
  @runtimeProperty("ModSettings.category.order", "5")
  @runtimeProperty("ModSettings.order", "36")
  @runtimeProperty("ModSettings.displayName", "Bird Calls")
  @runtimeProperty("ModSettings.description", "Mute the bird calls sounds in this category.")
  public let birds: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Environmental sounds")
  @runtimeProperty("ModSettings.category.order", "5")
  @runtimeProperty("ModSettings.order", "47")
  @runtimeProperty("ModSettings.displayName", "Mud and Wet Movement")
  @runtimeProperty("ModSettings.description", "Mute wet crawl, movement, and impact sounds in the landfill sequence.")
  public let mud_movement: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Voice sounds")
  @runtimeProperty("ModSettings.category.order", "6")
  @runtimeProperty("ModSettings.order", "37")
  @runtimeProperty("ModSettings.displayName", "All")
  @runtimeProperty("ModSettings.description", "Switch every mute in this section on or off.")
  public let allVoice: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Voice sounds")
  @runtimeProperty("ModSettings.category.order", "6")
  @runtimeProperty("ModSettings.order", "38")
  @runtimeProperty("ModSettings.displayName", "Whistling")
  @runtimeProperty("ModSettings.description", "Mute the whistling sounds in this category.")
  public let whistles: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Voice sounds")
  @runtimeProperty("ModSettings.category.order", "6")
  @runtimeProperty("ModSettings.order", "39")
  @runtimeProperty("ModSettings.displayName", "Humming")
  @runtimeProperty("ModSettings.description", "Mute the humming sounds in this category.")
  public let humming: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Voice sounds")
  @runtimeProperty("ModSettings.category.order", "6")
  @runtimeProperty("ModSettings.order", "40")
  @runtimeProperty("ModSettings.displayName", "Whispering")
  @runtimeProperty("ModSettings.description", "Mute the whispering sounds in this category.")
  public let whispering: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Voice sounds")
  @runtimeProperty("ModSettings.category.order", "6")
  @runtimeProperty("ModSettings.order", "41")
  @runtimeProperty("ModSettings.displayName", "Crying")
  @runtimeProperty("ModSettings.description", "Mute the crying sounds in this category.")
  public let crying: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Voice sounds")
  @runtimeProperty("ModSettings.category.order", "6")
  @runtimeProperty("ModSettings.order", "48")
  @runtimeProperty("ModSettings.displayName", "Ambient NPC Phone Calls")
  @runtimeProperty("ModSettings.description", "Mute generic NPC phone conversation sounds.")
  public let ambient_phone_calls: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Graphic sound effects")
  @runtimeProperty("ModSettings.category.order", "7")
  @runtimeProperty("ModSettings.order", "42")
  @runtimeProperty("ModSettings.displayName", "All")
  @runtimeProperty("ModSettings.description", "Switch every mute in this section on or off.")
  public let allGraphic: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Graphic sound effects")
  @runtimeProperty("ModSettings.category.order", "7")
  @runtimeProperty("ModSettings.order", "43")
  @runtimeProperty("ModSettings.displayName", "Gore and Squishing")
  @runtimeProperty("ModSettings.description", "Mute wet gore, splatter and squishing effects.")
  public let wet_gore: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Graphic sound effects")
  @runtimeProperty("ModSettings.category.order", "7")
  @runtimeProperty("ModSettings.order", "44")
  @runtimeProperty("ModSettings.displayName", "Bone Breaking")
  @runtimeProperty("ModSettings.description", "Mute the bone breaking sounds in this category.")
  public let bone_breaks: Bool = false;

  @runtimeProperty("ModSettings.mod", "Misophonia")
  @runtimeProperty("ModSettings.category", "Graphic sound effects")
  @runtimeProperty("ModSettings.category.order", "7")
  @runtimeProperty("ModSettings.order", "45")
  @runtimeProperty("ModSettings.displayName", "Flesh Impacts")
  @runtimeProperty("ModSettings.description", "Mute the flesh impacts sounds in this category.")
  public let flesh_impacts: Bool = false;

}
